//! Test CLI stand-ins are used solely for protocol/termination fault injection.
//! Product defaults always resolve the installed Claude/Codex executable.
use architalk::ai::{self, Backend, RunSpec};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

#[tokio::test]
async fn missing_cli_diagnostic_does_not_attempt_a_model() {
    let result = ai::diagnose(Backend::Claude, "/nonexistent/architalk-test-cli").await;
    assert!(!result.available);
    assert!(!result.compatible);
}

#[cfg(target_os = "macos")]
mod macos {
    use super::*;
    use std::{os::unix::fs::PermissionsExt, path::Path, time::Duration};
    fn fake_cli(dir: &Path, body: &str) -> String {
        let path = dir.join("fake-ai.py");
        // Required help tokens mirror the externally verified 2.1.326 profile;
        // this verifies product behavior, never real CLI compatibility.
        let script = format!(
            r##"#!/usr/bin/python3
import json,sys,os,time,subprocess,signal
if '--version' in sys.argv:
    print('2.1.326 (Claude Code)'); sys.exit(0)
if '--help' in sys.argv:
    print('--print --output-format --include-partial-messages --safe-mode --restricted --strict-mcp-config --mcp-config --setting-sources --settings --tools --permission-mode --disable-slash-commands --no-session-persistence --effort'); sys.exit(0)
prompt=sys.stdin.read()
{body}
"##
        );
        std::fs::write(&path, script).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
        path.to_str().unwrap().to_owned()
    }
    fn spec(executable: String, root: &Path) -> RunSpec {
        std::fs::create_dir_all(root.join(".architalk")).unwrap();
        RunSpec {
            backend: Backend::Claude,
            executable,
            model: None,
            effort: None,
            profile: None,
            timeout_seconds: 15,
            prompt: "test -- $(touch ignored)".into(),
            project_root: root.to_owned(),
        }
    }
    fn success() -> &'static str {
        "print(json.dumps({'type':'system','subtype':'init','tools':[],'mcp_servers':[],'permissionMode':'dontAsk','model':'fake-model'}),flush=True)\nprint(json.dumps({'type':'assistant','message':{'content':[{'type':'text','text':'OK'}]}}),flush=True)\nprint(json.dumps({'type':'result','subtype':'success','is_error':False,'result':'OK'}),flush=True)"
    }
    async fn wait_runtime_record(root: &Path, name: &str) -> String {
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                if let Ok(entries) = std::fs::read_dir(root.join(".architalk/runtime")) {
                    for entry in entries.flatten() {
                        if let Ok(text) = std::fs::read_to_string(entry.path().join(name)) {
                            if serde_json::from_str::<serde_json::Value>(&text).is_ok() {
                                return text;
                            }
                        }
                    }
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .unwrap()
    }
    #[tokio::test]
    async fn project_diagnostics_do_not_resurrect_missing_state_or_follow_runtime_symlinks() {
        use std::os::unix::fs::symlink;
        let root = tempfile::tempdir().unwrap();
        let fixture = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let executable = fake_cli(fixture.path(), "raise RuntimeError('model must not run')");
        let diagnostic = ai::diagnose_in_project(Backend::Claude, &executable, root.path()).await;
        assert!(!diagnostic.compatible);
        assert!(
            !root.path().join(".architalk").exists(),
            "missing state must not be recreated"
        );
        symlink(outside.path(), root.path().join(".architalk")).unwrap();
        let diagnostic = ai::diagnose_in_project(Backend::Claude, &executable, root.path()).await;
        assert!(!diagnostic.compatible);
        assert_eq!(std::fs::read_dir(outside.path()).unwrap().count(), 0);
        std::fs::remove_file(root.path().join(".architalk")).unwrap();
        std::fs::create_dir(root.path().join(".architalk")).unwrap();
        symlink(outside.path(), root.path().join(".architalk/runtime")).unwrap();
        let diagnostic = ai::diagnose_in_project(Backend::Claude, &executable, root.path()).await;
        assert!(!diagnostic.compatible);
        assert_eq!(std::fs::read_dir(outside.path()).unwrap().count(), 0);
    }

    #[tokio::test]
    async fn project_preparation_restricts_version_and_help_writes_and_cleans_scratch() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir(root.path().join(".architalk")).unwrap();
        let fixture = tempfile::tempdir().unwrap();
        let outside = fixture.path().join("outside-diagnostic-write");
        let executable = fake_cli(fixture.path(), "raise RuntimeError('model must not run')");
        let script = std::fs::read_to_string(&executable).unwrap();
        let probe = format!(
            "try:\n    open({},'w').write('forbidden')\n    raise RuntimeError('outside write escaped')\nexcept PermissionError:\n    pass\nwith open('diagnostic-private-write','w') as f:\n    f.write('allowed')\n",
            serde_json::to_string(outside.to_str().unwrap()).unwrap()
        );
        let script = script.replace(
            "if '--version' in sys.argv:",
            &(probe + "if '--version' in sys.argv:"),
        );
        std::fs::write(&executable, script).unwrap();
        let diagnostic = ai::diagnose_in_project(Backend::Claude, &executable, root.path()).await;
        assert!(diagnostic.compatible, "{}", diagnostic.message);
        assert!(!outside.exists());
        assert_eq!(
            std::fs::read_dir(root.path().join(".architalk/runtime"))
                .unwrap()
                .count(),
            0
        );
    }

    #[tokio::test]
    async fn explicitly_supported_claude_patch_profiles_only_use_version_and_help() {
        for version in ["2.1.326", "2.1.327"] {
            let fixture = tempfile::tempdir().unwrap();
            let marker = fixture.path().join("unexpected-model-call");
            let body = format!(
                "open({},'w').write('called')",
                serde_json::to_string(marker.to_str().unwrap()).unwrap()
            );
            let executable = fake_cli(fixture.path(), &body);
            let script = std::fs::read_to_string(&executable)
                .unwrap()
                .replace("2.1.326 (Claude Code)", &format!("{version} (Claude Code)"));
            std::fs::write(&executable, script).unwrap();
            let diagnostic = ai::diagnose(Backend::Claude, &executable).await;
            assert!(diagnostic.compatible, "{}", diagnostic.message);
            assert_eq!(
                diagnostic.version.as_deref(),
                Some(format!("{version} (Claude Code)").as_str())
            );
            assert!(diagnostic.message.contains(version));
            assert!(!marker.exists(), "diagnosis must never invoke a model");
        }
    }

    #[tokio::test]
    async fn unverified_patch_reports_observed_version_path_and_supported_profiles() {
        let fixture = tempfile::tempdir().unwrap();
        let executable = fake_cli(fixture.path(), "raise RuntimeError('model must not run')");
        let script = std::fs::read_to_string(&executable)
            .unwrap()
            .replace("2.1.326 (Claude Code)", "2.1.999 (Claude Code)");
        std::fs::write(&executable, script).unwrap();
        let diagnostic = ai::diagnose(Backend::Claude, &executable).await;
        assert!(!diagnostic.compatible);
        assert!(diagnostic.message.contains("2.1.999"));
        assert!(diagnostic.message.contains("2.1.326, 2.1.327"));
        assert!(diagnostic.message.contains(&executable));
        assert!(diagnostic.message.contains("No model was called"));
    }

    #[tokio::test]
    async fn supported_patch_still_requires_each_restriction_option() {
        let fixture = tempfile::tempdir().unwrap();
        let executable = fake_cli(fixture.path(), "raise RuntimeError('model must not run')");
        let script = std::fs::read_to_string(&executable)
            .unwrap()
            .replace("2.1.326 (Claude Code)", "2.1.327 (Claude Code)")
            .replace("--restricted ", "");
        std::fs::write(&executable, script).unwrap();
        let diagnostic = ai::diagnose(Backend::Claude, &executable).await;
        assert!(!diagnostic.compatible);
        assert!(
            diagnostic
                .message
                .contains("Required restriction option missing: --restricted")
        );
    }

    #[tokio::test]
    async fn product_only_writes_private_project_runtime_and_keeps_prompt_on_stdin() {
        let root = tempfile::tempdir().unwrap();
        let fixture = tempfile::tempdir().unwrap();
        let denied = root.path().join("forbidden.txt");
        let shared = fixture.path().join("forbidden-shared-history.txt");
        let body = format!(
            "blocked=[]\nfor target in [{},{}]:\n    try:\n        open(target,'w').write('forbidden')\n    except PermissionError:\n        blocked.append(target)\nwith open('runtime-proof','w') as out:\n    out.write('allowed')\nanswer=json.dumps({{'cwd':os.getcwd(),'argv':sys.argv,'prompt':prompt,'blocked':blocked,'tmp':os.environ.get('TMPDIR'),'traffic':os.environ.get('CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC'),'safe':os.environ.get('CLAUDE_CODE_SAFE_MODE')}})\n{}",
            serde_json::to_string(denied.to_str().unwrap()).unwrap(),
            serde_json::to_string(shared.to_str().unwrap()).unwrap(),
            success().replace("'OK'", "answer")
        );
        let executable = fake_cli(fixture.path(), &body);
        let (tx, _rx) = mpsc::channel(1);
        let mut request = spec(executable, root.path());
        request.model = Some("vendor/future-model:2[1m]".into());
        request.effort = Some("future_2-high".into());
        let output = ai::generate(request, CancellationToken::new(), tx)
            .await
            .unwrap();
        assert_eq!(output.resolved_model.as_deref(), Some("fake-model"));
        let record: serde_json::Value = serde_json::from_str(&output.text).unwrap();
        assert_eq!(record["blocked"].as_array().unwrap().len(), 2);
        assert!(!denied.exists());
        assert!(!shared.exists());
        let cwd = Path::new(record["cwd"].as_str().unwrap());
        assert!(
            cwd.starts_with(
                root.path()
                    .canonicalize()
                    .unwrap()
                    .join(".architalk/runtime")
            )
        );
        assert!(
            !cwd.exists(),
            "private scratch data is cleaned after the process terminates"
        );
        assert!(Path::new(record["tmp"].as_str().unwrap()).starts_with(cwd));
        assert_eq!(record["traffic"], "1");
        assert_eq!(record["safe"], "1");
        assert_eq!(record["prompt"], "test -- $(touch ignored)");
        let argv = record["argv"].as_array().unwrap();
        assert!(
            argv.windows(2)
                .any(|a| a[0] == "--model" && a[1] == "vendor/future-model:2[1m]")
        );
        assert!(
            argv.windows(2)
                .any(|a| a[0] == "--effort" && a[1] == "future_2-high")
        );
        assert!(
            !argv
                .iter()
                .any(|x| x.as_str() == Some("test -- $(touch ignored)"))
        );
    }
    #[tokio::test]
    async fn codex_requested_model_is_not_reported_as_resolved_without_cli_evidence() {
        let root = tempfile::tempdir().unwrap();
        let fixture = tempfile::tempdir().unwrap();
        let body = "print(json.dumps({'type':'thread.started'}),flush=True)\nprint(json.dumps({'type':'turn.started'}),flush=True)\nprint(json.dumps({'type':'item.completed','item':{'type':'agent_message','text':json.dumps(sys.argv)}}),flush=True)\nprint(json.dumps({'type':'turn.completed'}),flush=True)".to_owned();
        let executable = fake_cli(fixture.path(), &body);
        let mut script = std::fs::read_to_string(&executable).unwrap();
        script=script.replace("2.1.326 (Claude Code)","codex-cli 0.169.0").replace("--print --output-format --include-partial-messages --safe-mode --restricted --strict-mcp-config --mcp-config --setting-sources --settings --tools --permission-mode --disable-slash-commands --no-session-persistence --effort", "--json --ignore-user-config --ignore-rules --ephemeral --sandbox --skip-git-repo-check --disable");
        std::fs::write(&executable, script).unwrap();
        for effort in ["max", "ultra", "future_2-high"] {
            let mut request = spec(executable.clone(), root.path());
            request.backend = Backend::Codex;
            request.model = Some("gpt-6.2-sol".into());
            request.effort = Some(effort.into());
            let (tx, _rx) = mpsc::channel(1);
            let output = ai::generate(request, CancellationToken::new(), tx)
                .await
                .unwrap();
            assert_eq!(output.resolved_model, None);
            let argv: Vec<String> = serde_json::from_str(&output.text).unwrap();
            assert_eq!(argv.last().map(String::as_str), Some("-"));
            for key in ["sqlite_home", "log_dir", "history.persistence"] {
                let assignment = &argv
                    .windows(2)
                    .find(|a| a[0] == "-c" && a[1].starts_with(&format!("{key}=")))
                    .unwrap()[1];
                let parsed: toml::Value = toml::from_str(assignment).unwrap();
                if key == "history.persistence" {
                    assert_eq!(parsed["history"]["persistence"].as_str(), Some("none"));
                } else {
                    assert!(
                        Path::new(parsed[key].as_str().unwrap()).starts_with(
                            root.path()
                                .canonicalize()
                                .unwrap()
                                .join(".architalk/runtime")
                        )
                    );
                }
            }
            assert!(argv.windows(2).any(|a| a == ["--model", "gpt-6.2-sol"]));
            let assignment = &argv
                .windows(2)
                .find(|a| a[0] == "-c" && a[1].starts_with("model_reasoning_effort="))
                .unwrap()[1];
            let parsed: toml::Value = toml::from_str(assignment).unwrap();
            assert_eq!(parsed.as_table().unwrap().len(), 1);
            assert_eq!(parsed["model_reasoning_effort"].as_str(), Some(effort));
        }
    }
    #[tokio::test]
    async fn cancellation_stops_cli_and_its_process_group() {
        let root = tempfile::tempdir().unwrap();
        let fixture = tempfile::tempdir().unwrap();

        let body = "child=subprocess.Popen(['/bin/sleep','120'])\nwith open('pids.json','w') as out:\n    json.dump([os.getpid(),child.pid],out)\nchild.wait()".to_owned();
        let executable = fake_cli(fixture.path(), &body);
        let cancel = CancellationToken::new();
        let (tx, _rx) = mpsc::channel(1);
        let work = tokio::spawn(ai::generate(
            spec(executable, root.path()),
            cancel.clone(),
            tx,
        ));
        let pids: Vec<i32> =
            serde_json::from_str(&wait_runtime_record(root.path(), "pids.json").await).unwrap();
        cancel.cancel();
        let error = tokio::time::timeout(Duration::from_secs(12), work)
            .await
            .unwrap()
            .unwrap()
            .unwrap_err();
        assert_eq!(error.code, "cancelled");
        if error.termination_unknown {
            let output = std::process::Command::new("/bin/ps")
                .args([
                    "-o",
                    "pid,ppid,pgid,stat,comm",
                    "-p",
                    &pids
                        .iter()
                        .map(i32::to_string)
                        .collect::<Vec<_>>()
                        .join(","),
                ])
                .output()
                .unwrap();
            eprintln!(
                "termination evidence: {}",
                String::from_utf8_lossy(&output.stdout)
            );
        }
        assert!(!error.termination_unknown, "{error:?}");
        for pid in pids {
            assert_eq!(unsafe { libc::kill(pid, 0) }, -1, "PID {pid} survived");
        }
    }
    #[tokio::test]
    async fn timeout_kills_term_resistant_cli_and_does_not_retry() {
        let root = tempfile::tempdir().unwrap();
        let fixture = tempfile::tempdir().unwrap();

        let body = "signal.signal(signal.SIGTERM,signal.SIG_IGN)\nwith open('pid','w') as out:\n    out.write(str(os.getpid()))\ntime.sleep(120)".to_owned();
        let executable = fake_cli(fixture.path(), &body);
        let mut request = spec(executable, root.path());
        request.timeout_seconds = 1;
        let (tx, _rx) = mpsc::channel(1);
        let work = tokio::spawn(ai::generate(request, CancellationToken::new(), tx));
        let pid: i32 = wait_runtime_record(root.path(), "pid")
            .await
            .parse()
            .unwrap();
        let error = work.await.unwrap().unwrap_err();
        assert_eq!(error.code, "timeout");
        assert!(!error.termination_unknown, "{error:?}");
        assert_eq!(unsafe { libc::kill(pid, 0) }, -1);
    }
    #[tokio::test]
    async fn consumed_io_task_errors_return_ai_errors_without_panicking() {
        for (fault, body, expected) in [
            (
                "stdout",
                "print('malformed JSON',flush=True)\ntime.sleep(120)",
                "invalid_cli_output",
            ),
            (
                "stderr",
                "os.write(2,b'x'*(16*1024*1024+8192))\ntime.sleep(120)",
                "output_limit",
            ),
            ("stdin", "time.sleep(120)", "cli_io_error"),
        ] {
            let root = tempfile::tempdir().unwrap();
            let fixture = tempfile::tempdir().unwrap();
            let executable = fake_cli(fixture.path(), body);
            let mut request = spec(executable.clone(), root.path());
            if fault == "stdin" {
                let script = std::fs::read_to_string(&executable)
                    .unwrap()
                    .replace("prompt=sys.stdin.read()", "os.close(0)");
                std::fs::write(&executable, script).unwrap();
                request.prompt = "p".repeat(1024 * 1024);
            }
            let (tx, _rx) = mpsc::channel(1);
            let outcome = tokio::time::timeout(
                Duration::from_secs(12),
                tokio::spawn(ai::generate(request, CancellationToken::new(), tx)),
            )
            .await
            .unwrap();
            assert!(
                outcome.is_ok(),
                "{fault}: product adapter panicked instead of returning an error: {outcome:?}"
            );
            let error = outcome.unwrap().unwrap_err();
            assert_eq!(error.code, expected, "{fault}: {error:?}");
            assert!(!error.termination_unknown, "{fault}: {error:?}");
        }
    }
    #[tokio::test]
    async fn terminal_failure_keeps_only_bounded_redacted_stderr_categories() {
        let root = tempfile::tempdir().unwrap();
        let fixture = tempfile::tempdir().unwrap();
        let body = "os.write(2,b'Permission denied writing /private/SYNTHETIC_SECRET/models_cache.json token=SYNTHETIC_TOKEN\\n')\nprint(json.dumps({'type':'system','subtype':'init','tools':[],'mcp_servers':[],'permissionMode':'dontAsk'}),flush=True)\nprint(json.dumps({'type':'result','subtype':'error_during_execution','is_error':True,'errors':['synthetic failure']}),flush=True)\ntime.sleep(120)";
        let executable = fake_cli(fixture.path(), body);
        let (tx, _rx) = mpsc::channel(1);
        let error = ai::generate(spec(executable, root.path()), CancellationToken::new(), tx)
            .await
            .unwrap_err();
        assert_eq!(error.code, "cli_failed");
        assert!(error.message.contains("stdout: event=result"));
        assert!(error.message.contains("stderr:"));
        assert!(error.message.contains("component=model_catalog_cache"));
        assert!(!error.message.contains("SYNTHETIC_"));
        assert!(!error.message.contains("/private/"));
        assert!(error.message.len() < 1500);
    }

    #[tokio::test]
    async fn zero_exit_without_terminal_success_is_rejected() {
        let root = tempfile::tempdir().unwrap();
        let fixture = tempfile::tempdir().unwrap();
        let executable = fake_cli(
            fixture.path(),
            "print(json.dumps({'type':'system','subtype':'init','tools':[],'mcp_servers':[],'permissionMode':'dontAsk'}),flush=True)",
        );
        let (tx, _rx) = mpsc::channel(1);
        let error = ai::generate(spec(executable, root.path()), CancellationToken::new(), tx)
            .await
            .unwrap_err();
        assert_eq!(error.code, "invalid_cli_output");
    }
    #[tokio::test]
    async fn dropping_generation_future_kills_the_process_group() {
        let root = tempfile::tempdir().unwrap();
        let fixture = tempfile::tempdir().unwrap();

        let body = "with open('pid','w') as out:\n    out.write(str(os.getpid()))\ntime.sleep(120)"
            .to_owned();
        let executable = fake_cli(fixture.path(), &body);
        let (tx, _rx) = mpsc::channel(1);
        let work = tokio::spawn(ai::generate(
            spec(executable, root.path()),
            CancellationToken::new(),
            tx,
        ));
        let pid: i32 = wait_runtime_record(root.path(), "pid")
            .await
            .parse()
            .unwrap();
        work.abort();
        let _ = work.await;
        tokio::time::timeout(Duration::from_secs(3), async {
            while unsafe { libc::kill(pid, 0) } == 0 {
                tokio::time::sleep(Duration::from_millis(25)).await;
            }
        })
        .await
        .unwrap();
    }
}

#[tokio::test]
#[ignore = "Uses authenticated installed CLIs and may consume model quota; explicit invocation only"]
async fn real_installed_clis_return_terminal_response() {
    for backend in [Backend::Claude, Backend::Codex] {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir(root.path().join(".architalk")).unwrap();
        let (tx, _rx) = mpsc::channel(128);
        let output = ai::generate(
            RunSpec {
                backend,
                executable: backend.as_str().into(),
                model: None,
                effort: None,
                profile: None,
                timeout_seconds: 90,
                prompt: "Reply exactly ARCHITALK_OK. Do not use tools.".into(),
                project_root: root.path().to_owned(),
            },
            CancellationToken::new(),
            tx,
        )
        .await
        .unwrap();
        assert_eq!(output.text.trim(), "ARCHITALK_OK", "{backend}: {output:?}");
        println!(
            "{backend}: version={} resolved_model={:?} final={}",
            output.cli_version, output.resolved_model, output.text
        );
    }
}
