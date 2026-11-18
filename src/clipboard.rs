//! Write-only desktop clipboard adapter. Text is never logged or persisted.
use std::{ffi::OsString, path::PathBuf, process::Stdio, sync::Arc, time::Duration};
use tokio::{io::AsyncWriteExt, process::Command, sync::Semaphore};
use tokio_util::sync::CancellationToken;

pub const MAX_TEXT_BYTES: usize = 2 * 1024 * 1024;
pub const MAX_REQUEST_BYTES: usize = 3 * 1024 * 1024;
const TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, thiserror::Error)]
#[error("{message}")]
pub struct Error {
    pub code: &'static str,
    pub message: &'static str,
}
fn error(code: &'static str, message: &'static str) -> Error {
    Error { code, message }
}

struct CommandSpec {
    executable: PathBuf,
    args: Vec<OsString>,
}

pub struct Service {
    command: Option<CommandSpec>,
    timeout: Duration,
    gate: Arc<Semaphore>,
}

impl Default for Service {
    fn default() -> Self {
        Self {
            command: if cfg!(target_os = "macos") {
                Some(CommandSpec {
                    executable: PathBuf::from("/usr/bin/pbcopy"),
                    args: Vec::new(),
                })
            } else {
                None
            },
            timeout: TIMEOUT,
            gate: Arc::new(Semaphore::new(1)),
        }
    }
}

impl Service {
    pub async fn copy(&self, text: &str, cancel: &CancellationToken) -> Result<(), Error> {
        if text.len() > MAX_TEXT_BYTES || text.contains('\0') {
            return Err(error(
                "invalid_clipboard_text",
                "복사할 텍스트는 NUL 문자가 없는 2 MiB 이내의 문자열이어야 합니다.",
            ));
        }
        if cancel.is_cancelled() {
            return Err(error(
                "clipboard_cancelled",
                "클립보드 복사가 취소됐습니다.",
            ));
        }
        let _permit = self.gate.try_acquire().map_err(|_| {
            error(
                "clipboard_busy",
                "이전 클립보드 복사가 끝난 뒤 다시 시도하세요.",
            )
        })?;
        let spec = self.command.as_ref().ok_or_else(|| {
            error(
                "clipboard_unsupported",
                "이 운영체제에서는 로컬 자동 복사를 지원하지 않습니다. 화면에서 직접 복사하세요.",
            )
        })?;
        // No shell, caller-controlled executable/arguments, inherited output, or disk input.
        let mut child = Command::new(&spec.executable)
            .args(&spec.args)
            // pbcopy determines stdin encoding from locale. Preserve Korean and
            // emoji even if ArchiTalk was started from a non-UTF-8 environment.
            .env("LC_ALL", "en_US.UTF-8")
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .map_err(|_| {
                error(
                    "clipboard_unavailable",
                    "로컬 클립보드 도구를 실행할 수 없습니다.",
                )
            })?;
        let outcome = {
            let write = async {
                let mut stdin = child.stdin.take().ok_or(())?;
                stdin.write_all(text.as_bytes()).await.map_err(|_| ())?;
                stdin.shutdown().await.map_err(|_| ())?;
                drop(stdin);
                child
                    .wait()
                    .await
                    .map_err(|_| ())
                    .and_then(|status| if status.success() { Ok(()) } else { Err(()) })
            };
            tokio::pin!(write);
            tokio::select! {
                biased;
                _ = cancel.cancelled() => Err(error("clipboard_cancelled", "클립보드 복사가 취소됐습니다.")),
                _ = tokio::time::sleep(self.timeout) => Err(error("clipboard_timeout", "로컬 클립보드 복사가 5초 제한을 초과했습니다.")),
                result = &mut write => result.map_err(|_| error("clipboard_failed", "로컬 클립보드에 복사하지 못했습니다. 화면에서 직접 복사하세요.")),
            }
        };
        if outcome.is_err() {
            // kill() waits for the direct helper to exit. kill_on_drop also covers
            // the HTTP caller disappearing while this future is being polled.
            let _ = child.kill().await;
        }
        outcome
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        app::App,
        config::Settings,
        http::{self, ServerState},
    };
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use http_body_util::BodyExt;
    use serde_json::{Value, json};
    use std::{fs, os::unix::fs::PermissionsExt, path::Path};
    use tower::ServiceExt;

    fn helper(dir: &Path, script: &str, output: &Path, timeout: Duration) -> Service {
        let executable = dir.join("fake-copy");
        fs::write(&executable, script).unwrap();
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
        Service {
            command: Some(CommandSpec {
                executable,
                args: vec![output.as_os_str().to_owned()],
            }),
            timeout,
            gate: Arc::new(Semaphore::new(1)),
        }
    }
    fn fake_app(project: &Path, helper_dir: &Path, output: &Path, script: &str) -> Arc<App> {
        let mut app = App::open(project, Settings::default()).unwrap();
        Arc::get_mut(&mut app).unwrap().clipboard =
            helper(helper_dir, script, output, Duration::from_secs(2));
        app
    }
    fn router(app: Arc<App>) -> axum::Router {
        http::router(ServerState {
            app,
            token: "test-capability".into(),
            authority: "127.0.0.1:8087".into(),
        })
    }
    fn request(body: Body) -> Request<Body> {
        Request::builder()
            .method("POST")
            .uri("/api/v1/clipboard")
            .header("host", "127.0.0.1:8087")
            .header("origin", "http://127.0.0.1:8087")
            .header("authorization", "Bearer test-capability")
            .header("content-type", "application/json")
            .body(body)
            .unwrap()
    }
    async fn json_response(response: axum::response::Response) -> Value {
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap()
    }
    async fn wait_pid(path: &Path) -> i32 {
        tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                if let Ok(text) = fs::read_to_string(path) {
                    if let Ok(pid) = text.trim().parse() {
                        return pid;
                    }
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .unwrap()
    }
    fn assert_exited(pid: i32) {
        // Signal zero only probes this test's known direct helper PID.
        assert_eq!(
            unsafe { libc::kill(pid, 0) },
            -1,
            "helper {pid} still exists"
        );
        assert_eq!(
            std::io::Error::last_os_error().raw_os_error(),
            Some(libc::ESRCH)
        );
    }

    #[tokio::test]
    async fn native_writer_preserves_utf8_and_treats_shell_looking_text_as_stdin() {
        let dir = tempfile::tempdir().unwrap();
        let output = dir.path().join("received");
        let sentinel = dir.path().join("must-not-exist");
        let text = format!(
            "설계 🦀\nsecond line\n$(touch '{}') `touch '{}'`\n",
            sentinel.display(),
            sentinel.display()
        );
        let service = helper(
            dir.path(),
            "#!/bin/sh\nprintf '%s' \"$#\" > \"$1.args\"\nprintf '%s' \"$LC_ALL\" > \"$1.locale\"\nexec /bin/cat > \"$1\"\n",
            &output,
            Duration::from_secs(1),
        );
        service
            .copy(&text, &CancellationToken::new())
            .await
            .unwrap();
        assert_eq!(fs::read(&output).unwrap(), text.as_bytes());
        assert_eq!(
            fs::read_to_string(dir.path().join("received.args")).unwrap(),
            "1"
        );
        assert_eq!(
            fs::read_to_string(dir.path().join("received.locale")).unwrap(),
            "en_US.UTF-8"
        );
        assert!(!sentinel.exists());
        // Empty text is an intentional clipboard clear; the upper decoded byte boundary is accepted.
        service.copy("", &CancellationToken::new()).await.unwrap();
        assert_eq!(fs::read(&output).unwrap(), b"");
        service
            .copy(&"a".repeat(MAX_TEXT_BYTES), &CancellationToken::new())
            .await
            .unwrap();
        assert_eq!(fs::metadata(&output).unwrap().len(), MAX_TEXT_BYTES as u64);
    }

    #[tokio::test]
    async fn native_writer_rejects_invalid_text_and_reports_fixed_tool_errors() {
        let dir = tempfile::tempdir().unwrap();
        let output = dir.path().join("received");
        let mut service = helper(
            dir.path(),
            "#!/bin/sh\nexit 7\n",
            &output,
            Duration::from_secs(1),
        );
        for text in [
            "text\0suffix".to_owned(),
            "한".repeat(MAX_TEXT_BYTES / 3 + 1),
        ] {
            assert_eq!(
                service
                    .copy(&text, &CancellationToken::new())
                    .await
                    .unwrap_err()
                    .code,
                "invalid_clipboard_text"
            );
        }
        assert_eq!(
            service
                .copy("private synthetic text", &CancellationToken::new())
                .await
                .unwrap_err()
                .code,
            "clipboard_failed"
        );
        service.command.as_mut().unwrap().executable = dir.path().join("missing");
        let unavailable = service
            .copy("private synthetic text", &CancellationToken::new())
            .await
            .unwrap_err();
        assert_eq!(unavailable.code, "clipboard_unavailable");
        assert!(!unavailable.message.contains("private"));
        service.command = None;
        assert_eq!(
            service
                .copy("text", &CancellationToken::new())
                .await
                .unwrap_err()
                .code,
            "clipboard_unsupported"
        );
        assert!(!output.exists());
    }

    #[tokio::test]
    async fn native_writer_kills_and_reaps_helpers_on_timeout_and_cancellation() {
        for cancelled in [false, true] {
            let dir = tempfile::tempdir().unwrap();
            let pid_path = dir.path().join("pid");
            let service = helper(
                dir.path(),
                "#!/bin/sh\nprintf '%s' \"$$\" > \"$1\"\nexec /bin/sleep 30\n",
                &pid_path,
                if cancelled {
                    Duration::from_secs(2)
                } else {
                    Duration::from_millis(100)
                },
            );
            let cancel = CancellationToken::new();
            if !cancelled {
                assert_eq!(
                    service.copy("text", &cancel).await.unwrap_err().code,
                    "clipboard_timeout"
                );
                // Inspect after the bounded writer returns: a timeout and the
                // readiness probe can become ready together on a loaded runner.
                assert_exited(wait_pid(&pid_path).await);
                continue;
            }
            let work = service.copy("text", &cancel);
            tokio::pin!(work);
            let pid = tokio::select! {
                pid = wait_pid(&pid_path) => pid,
                result = &mut work => panic!("helper finished before pid: {result:?}"),
            };
            assert_eq!(
                service
                    .copy("other", &CancellationToken::new())
                    .await
                    .unwrap_err()
                    .code,
                "clipboard_busy"
            );
            cancel.cancel();
            assert_eq!(work.await.unwrap_err().code, "clipboard_cancelled");
            assert_exited(pid);
        }
    }

    #[tokio::test]
    async fn clipboard_http_copies_only_text_without_workspace_history_or_receipts() {
        let project = tempfile::tempdir().unwrap();
        let helper_dir = tempfile::tempdir().unwrap();
        let output = helper_dir.path().join("received");
        let app = fake_app(
            project.path(),
            helper_dir.path(),
            &output,
            "#!/bin/sh\nexec /bin/cat > \"$1\"\n",
        );
        let before = app.workspace().unwrap();
        let text = "synthetic-only-clipboard-문서 🦀\n[00:01.250] select · docs/design.md\n";
        let response = router(app.clone())
            .oneshot(request(Body::from(json!({"text":text}).to_string())))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(json_response(response).await, json!({"copied":true}));
        assert_eq!(fs::read_to_string(output).unwrap(), text);
        assert_eq!(app.workspace().unwrap(), before);
        for entry in fs::read_dir(project.path().join(".architalk")).unwrap() {
            let entry = entry.unwrap();
            if entry.file_type().unwrap().is_file() {
                let bytes = fs::read(entry.path()).unwrap();
                assert!(!bytes.windows(text.len()).any(|w| w == text.as_bytes()));
            }
        }
    }

    #[tokio::test]
    async fn clipboard_http_rejects_unauthorized_origin_and_invalid_requests_before_helper() {
        let project = tempfile::tempdir().unwrap();
        let helper_dir = tempfile::tempdir().unwrap();
        let output = helper_dir.path().join("received");
        let app = fake_app(
            project.path(),
            helper_dir.path(),
            &output,
            "#!/bin/sh\nexec /bin/cat > \"$1\"\n",
        );
        let api = router(app.clone());
        for (header, replacement, status) in [
            (
                "authorization",
                Some("Bearer wrong"),
                StatusCode::UNAUTHORIZED,
            ),
            (
                "origin",
                Some("https://untrusted.example"),
                StatusCode::FORBIDDEN,
            ),
            ("origin", None, StatusCode::FORBIDDEN),
            ("host", Some("localhost:8087"), StatusCode::FORBIDDEN),
        ] {
            let mut req = request(Body::from("{\"text\":\"sensitive\"}"));
            if let Some(replacement) = replacement {
                req.headers_mut()
                    .insert(header, replacement.parse().unwrap());
            } else {
                req.headers_mut().remove(header);
            }
            assert_eq!(api.clone().oneshot(req).await.unwrap().status(), status);
        }
        for body in [
            "{}".to_owned(),
            "{\"text\":7}".to_owned(),
            "{\"text\":\"x\",\"command\":\"elsewhere\"}".to_owned(),
            "broken".to_owned(),
            json!({"text":"a\0b"}).to_string(),
            json!({"text":"a".repeat(MAX_TEXT_BYTES + 1)}).to_string(),
            " ".repeat(MAX_REQUEST_BYTES + 1),
        ] {
            let response = api
                .clone()
                .oneshot(request(Body::from(body)))
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::BAD_REQUEST);
            assert_eq!(
                json_response(response).await["error"]["code"],
                "invalid_clipboard_text"
            );
        }
        app.shutdown.cancel();
        let response = api
            .clone()
            .oneshot(request(Body::from("{\"text\":\"after shutdown\"}")))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        fs::remove_dir_all(project.path().join(".architalk")).unwrap();
        let response = api
            .oneshot(request(Body::from("{\"text\":\"stale\"}")))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::GONE);
        assert!(!output.exists());
        assert!(!project.path().join(".architalk").exists());
    }

    #[tokio::test]
    async fn clipboard_http_reset_and_shutdown_cancel_pending_helpers() {
        for reset in [true, false] {
            let project = tempfile::tempdir().unwrap();
            let helper_dir = tempfile::tempdir().unwrap();
            let pid_path = helper_dir.path().join("pid");
            let app = fake_app(
                project.path(),
                helper_dir.path(),
                &pid_path,
                "#!/bin/sh\nprintf '%s' \"$$\" > \"$1\"\nexec /bin/sleep 30\n",
            );
            let work = tokio::spawn(
                router(app.clone()).oneshot(request(Body::from("{\"text\":\"pending\"}"))),
            );
            let pid = wait_pid(&pid_path).await;
            if reset {
                fs::remove_dir_all(project.path().join(".architalk")).unwrap();
            } else {
                app.shutdown.cancel();
            }
            let response = tokio::time::timeout(Duration::from_secs(1), work)
                .await
                .unwrap()
                .unwrap()
                .unwrap();
            assert_eq!(
                response.status(),
                if reset {
                    StatusCode::GONE
                } else {
                    StatusCode::SERVICE_UNAVAILABLE
                }
            );
            assert_eq!(
                json_response(response).await["error"]["code"],
                if reset {
                    "workspace_reset_required"
                } else {
                    "shutting_down"
                }
            );
            assert_exited(pid);
            if reset {
                assert!(!project.path().join(".architalk").exists());
            }
        }
    }
}
