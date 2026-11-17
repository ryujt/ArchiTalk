//! Opt-in, paid/subscription CLI checks. Never run as part of normal `cargo test`.
#![cfg(target_os = "macos")]
use architalk::{ai::Backend, config::Settings};
use serde_json::{Value, json};
use std::{
    io::{BufRead, BufReader},
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

struct Server {
    child: Child,
    base: String,
    client: reqwest::Client,
}
impl Server {
    async fn start(project: &std::path::Path, config: &std::path::Path) -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_architalk"))
            .arg("--project")
            .arg(project)
            .arg("--config")
            .arg(config)
            .arg("--no-open")
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap();
        let stdout = child.stdout.take().unwrap();
        let url = tokio::task::spawn_blocking(move || {
            let reader = BufReader::new(stdout);
            reader
                .lines()
                .map_while(Result::ok)
                .find(|l| l.starts_with("http://127.0.0.1:"))
                .unwrap()
        })
        .await
        .unwrap();
        let (base, fragment) = url.split_once("/#token=").unwrap();
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            "authorization",
            format!("Bearer {fragment}").parse().unwrap(),
        );
        headers.insert("origin", base.parse().unwrap());
        let client = reqwest::Client::builder()
            .default_headers(headers)
            .timeout(Duration::from_secs(20))
            .build()
            .unwrap();
        Self {
            child,
            base: base.into(),
            client,
        }
    }
    async fn get(&self, path: &str) -> Value {
        let r = self
            .client
            .get(format!("{}/api/v1/{}", self.base, path))
            .send()
            .await
            .unwrap();
        let status = r.status();
        let body: Value = r.json().await.unwrap();
        assert!(status.is_success(), "GET {path}: {status} {body}");
        body
    }
    async fn post(&self, path: &str, revision: u64, payload: Value) -> Value {
        let r=self.client.post(format!("{}/api/v1/{}",self.base,path)).json(&json!({"command_id":uuid::Uuid::new_v4().to_string(),"expected_revision":revision,"payload":payload})).send().await.unwrap();
        let status = r.status();
        let body: Value = r.json().await.unwrap();
        assert!(status.is_success(), "POST {path}: {status} {body}");
        body
    }
    async fn settle(&self, sid: &str) -> Value {
        let deadline = Instant::now() + Duration::from_secs(600);
        loop {
            let w = self.get("workspace").await;
            if w["active_run"]["process_termination"] == "unknown" {
                panic!(
                    "process termination requires manual confirmation: {}",
                    w["active_run"]
                );
            }
            if w["active_run"].is_null() {
                let s = self.get(&format!("sessions/{sid}")).await;
                for r in w["runs"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter(|r| r["session_id"] == sid)
                {
                    assert_eq!(r["status"], "completed", "real CLI run failed: {}", r);
                }
                return s;
            }
            assert!(Instant::now() < deadline, "real workflow timed out");
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        unsafe {
            libc::kill(self.child.id() as i32, libc::SIGTERM);
        }
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            if self.child.try_wait().ok().flatten().is_some() {
                break;
            }
            if Instant::now() > deadline {
                let _ = self.child.kill();
                let _ = self.child.wait();
                break;
            }
            std::thread::sleep(Duration::from_millis(30));
        }
    }
}

async fn workflow(backend: Backend) {
    let project = tempfile::tempdir().unwrap();
    let conf = tempfile::tempdir().unwrap();
    let config = conf.path().join("architalk.toml");
    let mut settings = Settings::default();
    settings.ai.default_backend = backend;
    settings.ai.timeout_seconds = 180;
    match backend {
        Backend::Claude => settings.ai.backends.claude.effort = Some("low".into()),
        Backend::Codex => settings.ai.backends.codex.effort = Some("low".into()),
    };
    std::fs::write(&config, toml::to_string(&settings).unwrap()).unwrap();
    std::fs::write(project.path().join("README.md"),"# 개인 메모 CLI\n개인용 메모를 JSON 파일로 저장하는 로컬 CLI. 다중 사용자·외부 연동·과금은 제외합니다.\n").unwrap();
    let server = Server::start(project.path(), &config).await;
    assert_eq!(server.get("workspace").await["sessions"], json!([]));
    let intro = "혼자 macOS에서 쓸 개인 메모 CLI입니다. 메모 추가·목록·검색만 첫 버전에 넣고 삭제·동기화·로그인·외부 API는 제외합니다. 메모는 프로젝트의 JSON 파일 하나로 저장하고 원본 덮어쓰기 전 백업합니다. 일반 텍스트만 처리하고 민감 정보는 넣지 않습니다. 로컬 1만 건 이하, 검색은 1초 이내를 목표로 측정합니다. 프로세스 한 개만 쓰고 파일 손상은 백업에서 수동 복구합니다. 설치와 복구 담당자는 본인입니다. Rust로 구현합니다. 현재 범위에 맞는 작은 설계를 원합니다. 더 확인할 내용은 미정으로 남기고 첫 초안을 작성해 주세요.";
    let created = server
        .post(
            "sessions",
            0,
            json!({"title":"개인 메모 CLI","intro":intro,"context_paths":["README.md"]}),
        )
        .await;
    let sid = created["session"]["id"].as_str().unwrap();
    let session = server.settle(sid).await;
    println!("{backend}: conversation settled");
    assert!(
        session["messages"]
            .as_array()
            .unwrap()
            .iter()
            .any(|m| m["role"] == "assistant")
    );
    let mut stream = server
        .client
        .get(format!(
            "{}/api/v1/sessions/{sid}/events?after=0",
            server.base
        ))
        .send()
        .await
        .unwrap();
    assert!(
        stream.headers()["content-type"]
            .to_str()
            .unwrap()
            .contains("text/event-stream")
    );
    let chunk = stream.chunk().await.unwrap().unwrap();
    assert!(String::from_utf8_lossy(&chunk).contains("state"));
    drop(stream);
    server
        .post(
            &format!("sessions/{sid}/draft"),
            session["revision"].as_u64().unwrap(),
            json!({}),
        )
        .await;
    let session = server.settle(sid).await;
    let draft = &session["drafts"][0];
    assert_eq!(draft["reviewed"], true);
    let preview = server
        .post(
            &format!("sessions/{sid}/exports"),
            session["revision"].as_u64().unwrap(),
            json!({"mode":"preview","draft_id":draft["id"]}),
        )
        .await;
    let saved = server
        .post(
            &format!("sessions/{sid}/exports"),
            session["revision"].as_u64().unwrap(),
            json!({"mode":"apply","plan_id":preview["export"]["id"]}),
        )
        .await;
    assert_eq!(saved["export"]["status"], "completed");
    let path = saved["export"]["result"]["paths"][0]
        .as_str()
        .unwrap()
        .to_owned();
    let markdown = std::fs::read_to_string(project.path().join(&path)).unwrap();
    assert!(markdown.contains("ArchiTalk 근거"));
    assert!(markdown.contains("검토 및 저장 기준"));
    let w = server.get("workspace").await;
    let runs = w["runs"].as_array().unwrap();
    println!(
        "real {backend} workflow: {} completed runs, {} byte document, {} review findings; all outputs in isolated temporary project",
        runs.len(),
        markdown.len(),
        session["findings"].as_array().unwrap().len()
    );
    for r in runs {
        println!(
            "  role={} CLI={} requested model={} effort={} resolved={}",
            r["kind"],
            r["cli_version"],
            r["effective_settings"]["model"],
            r["effective_settings"]["effort"],
            r["resolved_model"]
        );
    }
    drop(server);
    let server = Server::start(project.path(), &config).await;
    let w = server.get("workspace").await;
    assert_eq!(w["sessions"].as_array().unwrap().len(), 1);
    assert_eq!(w["exports"][0]["status"], "completed");
    assert_eq!(
        std::fs::read_to_string(project.path().join(path)).unwrap(),
        markdown
    );
    assert!(w["active_run"].is_null());
}
#[tokio::test]
#[ignore = "uses authenticated local Claude CLI and may incur model usage"]
async fn real_claude_project_to_document_and_restart() {
    workflow(Backend::Claude).await;
}
#[tokio::test]
#[ignore = "uses authenticated local Codex CLI and may incur model usage"]
async fn real_codex_project_to_document_and_restart() {
    workflow(Backend::Codex).await;
}
