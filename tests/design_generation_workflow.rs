#[cfg(target_os = "macos")]
mod macos {
    use architalk::{
        app::{App, LiveEvent},
        config::Settings,
        design_generation::GenerationKind,
        domain,
        model::{Command, Workspace, hash, id},
        storage::Store,
    };
    use serde_json::{Value, json};
    use std::{fs, os::unix::fs::PermissionsExt, sync::Arc, time::Duration};
    use tempfile::TempDir;
    use tokio::sync::broadcast;

    struct Fixture {
        project: TempDir,
        tools: TempDir,
        settings: Settings,
        app: Arc<App>,
        sid: String,
    }
    fn command(revision: u64, payload: Value) -> Command {
        Command {
            command_id: id(),
            expected_revision: revision,
            payload,
        }
    }
    impl Fixture {
        fn new() -> Self {
            let project = tempfile::tempdir().unwrap();
            let tools = tempfile::tempdir().unwrap();
            let executable = tools.path().join("fixture.py");
            fs::write(&executable, include_str!("fixtures/claude-workflow.py")).unwrap();
            fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
            fs::write(
                tools.path().join("design-package.json"),
                include_str!("fixtures/design-package.json"),
            )
            .unwrap();
            let mut settings = Settings::default();
            settings.ai.backends.claude.executable = executable.to_string_lossy().into();
            settings.ai.backends.codex.executable = "/nonexistent/never-call-real-codex".into();
            let session = domain::new_session(
                "분할 생성 합성 과업".into(),
                "로컬 자료를 선택하고 상세를 확인합니다.".into(),
                vec![],
            );
            let sid = session.id.clone();
            let mut workspace = Workspace::new(settings.clone());
            workspace.sessions.push(session);
            let mut store = Store::open(project.path()).unwrap();
            store
                .commit_workspace(&serde_json::to_value(workspace).unwrap(), None, None)
                .unwrap();
            drop(store);
            let app = App::open(project.path(), settings.clone()).unwrap();
            Self {
                project,
                tools,
                settings,
                app,
                sid,
            }
        }
        fn control(&self, value: Value) {
            let path = self.tools.path().join("stage-control.json");
            fs::write(path.with_extension("tmp"), value.to_string()).unwrap();
            fs::rename(path.with_extension("tmp"), path).unwrap();
        }
        fn draft(&self, payload: Value) -> Value {
            let revision = self.app.session(&self.sid).unwrap()["revision"]
                .as_u64()
                .unwrap();
            self.app
                .command("draft", Some(&self.sid), command(revision, payload))
                .unwrap()
        }
    }
    async fn settled(app: &Arc<App>) {
        tokio::time::timeout(Duration::from_secs(25), async {
            while !app.is_idle() {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
    }
    async fn at_prefix(fixture: &Fixture, count: usize, current: &str) {
        tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                let w = fixture.app.workspace().unwrap();
                if w["sessions"][0]["design_generations"][0]["completed"]
                    .as_array()
                    .is_some_and(|v| v.len() == count)
                    && w["active_run"]["progress"]
                        .as_str()
                        .is_some_and(|v| v.contains(current))
                {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
    }
    fn attempts(events: &mut broadcast::Receiver<LiveEvent>) -> usize {
        let mut count = 0;
        loop {
            match events.try_recv() {
                Ok(event)
                    if event.kind == "progress"
                        && event.payload["message"] == "Claude is generating a response." =>
                {
                    count += 1
                }
                Ok(_) => (),
                Err(broadcast::error::TryRecvError::Empty) => return count,
                Err(error) => panic!("lost generation observations: {error}"),
            }
        }
    }

    #[tokio::test]
    async fn ten_requests_checkpoint_in_order_then_create_one_package_and_one_critic() {
        let f = Fixture::new();
        let original = f.app.session(&f.sid).unwrap();
        let mut events = f.app.events.subscribe();
        let started = f.draft(json!({}));
        settled(&f.app).await;
        let w = f.app.workspace().unwrap();
        assert_eq!(w["runs"].as_array().unwrap().len(), 2, "{}", w["runs"]);
        assert_eq!(w["runs"][0]["status"], "completed", "{}", w["runs"]);
        assert_eq!(w["runs"][1]["kind"], "critic");
        assert_eq!(w["runs"][1]["status"], "completed");
        assert_eq!(attempts(&mut events), 11);
        let s = &w["sessions"][0];
        let cp = &s["design_generations"][0];
        let parts = cp["completed"].as_array().unwrap();
        assert_eq!(parts.len(), 10);
        assert_eq!(s["drafts"].as_array().unwrap().len(), 1);
        assert_eq!(cp["draft_id"], s["drafts"][0]["id"]);
        for (index, part) in parts.iter().enumerate() {
            assert_eq!(
                part["document"]["kind"],
                GenerationKind::ALL[index].as_str()
            );
            assert_eq!(
                part["content_hash"],
                hash(part["document"]["markdown"].as_str().unwrap())
            );
            assert_eq!(part["source_run_id"], started["run"]["id"]);
            assert_eq!(part["effective_settings"]["timeout_seconds"], 1800);
            assert_eq!(
                part["effective_settings"]["generation_protocol"],
                architalk::design_generation::VERSION
            );
            assert_eq!(
                part["effective_settings"]["generation_contract_hash"],
                cp["contract_hash"]
            );
            assert_eq!(
                part["effective_settings"]["prompt_version"],
                architalk::design_generation::VERSION
            );
            assert_eq!(part["cli_version"], "2.1.326 (Claude Code)");
            assert_eq!(part["resolved_model"], "fixture-model");
        }
        assert_eq!(s["messages"], original["messages"]);
        assert_eq!(s["design_revision"], original["design_revision"]);
        let records = f.app.replay(Some(&f.sid), 0).unwrap();
        let completed: Vec<_> = records
            .iter()
            .filter(|e| e.kind == "design_document_completed")
            .collect();
        assert_eq!(completed.len(), 10);
        for (index, event) in completed.into_iter().enumerate() {
            assert_eq!(event.payload["completed"], index + 1);
            assert_eq!(event.payload["kind"], GenerationKind::ALL[index].as_str());
        }
        f.control(json!({"fail_kind":"drivers"}));
        f.draft(json!({}));
        settled(&f.app).await;
        let s = f.app.session(&f.sid).unwrap();
        assert_eq!(s["design_generations"].as_array().unwrap().len(), 2);
        assert_eq!(s["drafts"].as_array().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn failed_stage_resumes_from_durable_prefix_with_new_spec_and_provenance() {
        let f = Fixture::new();
        f.control(json!({"fail_kind":"decisions"}));
        let mut events = f.app.events.subscribe();
        let first = f.draft(json!({}));
        settled(&f.app).await;
        assert_eq!(attempts(&mut events), 4);
        let before = f.app.workspace().unwrap();
        assert_eq!(before["runs"][0]["error"]["code"], "invalid_model_output");
        assert_eq!(before["sessions"][0]["drafts"], json!([]));
        let prefix = before["sessions"][0]["design_generations"][0]["completed"]
            .as_array()
            .unwrap()
            .clone();
        assert_eq!(prefix.len(), 3);
        let Fixture {
            app,
            project,
            tools,
            settings,
            sid,
        } = f;
        drop(app);
        let app = App::open(project.path(), settings).unwrap();
        assert_eq!(
            app.session(&sid).unwrap()["design_generations"],
            before["sessions"][0]["design_generations"]
        );
        let snapshot = app.settings().unwrap();
        let mut changed = snapshot["settings"].clone();
        changed["ai"]["timeout_seconds"] = json!(1200);
        changed["ai"]["backends"]["claude"]["model"] = json!("resumed-model");
        app.command(
            "settings",
            None,
            command(
                snapshot["revision"].as_u64().unwrap(),
                json!({"settings":changed}),
            ),
        )
        .unwrap();
        fs::write(tools.path().join("stage-control.json"), "{}").unwrap();
        let mut events = app.events.subscribe();
        let revision = app.session(&sid).unwrap()["revision"].as_u64().unwrap();
        let second = app
            .command("draft", Some(&sid), command(revision, json!({})))
            .unwrap();
        settled(&app).await;
        assert_eq!(attempts(&mut events), 8);
        let w = app.workspace().unwrap();
        let s = &w["sessions"][0];
        assert_eq!(s["design_generations"].as_array().unwrap().len(), 1);
        assert_eq!(s["drafts"].as_array().unwrap().len(), 1, "{}", w["runs"]);
        let parts = s["design_generations"][0]["completed"].as_array().unwrap();
        assert_eq!(&parts[..3], prefix.as_slice());
        for part in &parts[3..] {
            assert_eq!(part["source_run_id"], second["run"]["id"]);
            assert_eq!(part["effective_settings"]["timeout_seconds"], 1200);
            assert_eq!(part["effective_settings"]["model"], "resumed-model");
        }
        assert_eq!(w["runs"][0]["id"], first["run"]["id"]);
        assert_eq!(w["runs"][0]["status"], "failed");
        assert_eq!(w["runs"][1]["status"], "completed");
        assert_eq!(w["runs"][2]["kind"], "critic");
    }

    #[tokio::test]
    async fn restart_is_strict_and_fresh_input_and_corrupt_cache_never_reuse_a_prefix() {
        let f = Fixture::new();
        f.control(json!({"fail_kind":"decisions"}));
        let receipt = command(0, json!({}));
        let first = f
            .app
            .command("draft", Some(&f.sid), receipt.clone())
            .unwrap();
        settled(&f.app).await;
        assert_eq!(
            f.app.command("draft", Some(&f.sid), receipt).unwrap(),
            first
        );
        for invalid in [json!("true"), json!(1), Value::Null] {
            let before = f.app.workspace().unwrap();
            let revision = before["sessions"][0]["revision"].as_u64().unwrap();
            assert_eq!(
                f.app
                    .command(
                        "draft",
                        Some(&f.sid),
                        command(revision, json!({"restart":invalid}))
                    )
                    .unwrap_err()
                    .code,
                "invalid_input"
            );
            assert_eq!(f.app.workspace().unwrap(), before);
        }
        let prior = f.app.session(&f.sid).unwrap()["design_generations"][0].clone();
        f.draft(json!({"restart":true}));
        settled(&f.app).await;
        let s = f.app.session(&f.sid).unwrap();
        assert_eq!(s["design_generations"].as_array().unwrap().len(), 2);
        assert_eq!(s["design_generations"][0], prior);
        f.app
            .command(
                "message",
                Some(&f.sid),
                command(
                    s["revision"].as_u64().unwrap(),
                    json!({"content":"추가로 로컬 파일의 내용을 확인합니다."}),
                ),
            )
            .unwrap();
        settled(&f.app).await;
        f.draft(json!({}));
        settled(&f.app).await;
        let w = f.app.workspace().unwrap();
        assert_eq!(
            w["sessions"][0]["design_generations"]
                .as_array()
                .unwrap()
                .len(),
            3
        );
        let mut seeded: Workspace = serde_json::from_value(w).unwrap();
        seeded.sessions[0]
            .design_generations
            .last_mut()
            .unwrap()
            .completed[0]
            .content_hash = "invalid-cache-hash".into();
        let Fixture {
            app,
            project,
            tools,
            settings,
            sid,
        } = f;
        drop(app);
        let mut store = Store::open(project.path()).unwrap();
        store
            .commit_workspace(&serde_json::to_value(seeded).unwrap(), None, None)
            .unwrap();
        drop(store);
        let app = App::open(project.path(), settings).unwrap();
        let mut events = app.events.subscribe();
        let revision = app.session(&sid).unwrap()["revision"].as_u64().unwrap();
        app.command("draft", Some(&sid), command(revision, json!({})))
            .unwrap();
        settled(&app).await;
        assert_eq!(attempts(&mut events), 4);
        assert_eq!(
            app.session(&sid).unwrap()["design_generations"]
                .as_array()
                .unwrap()
                .len(),
            4
        );
        drop(tools);
    }

    #[tokio::test]
    async fn final_architecture_failure_keeps_nine_parts_then_resumes_without_regenerating_them() {
        let f = Fixture::new();
        let package: Value =
            serde_json::from_str(include_str!("fixtures/design-package.json")).unwrap();
        let mut broken = package.clone();
        let mut main = broken["markdown"].as_str().unwrap().to_owned();
        let marker = "## Method-R module design\n";
        let start = main.find(marker).unwrap() + marker.len();
        let end = main[start..].find("\n## ").unwrap() + start;
        main.replace_range(
            start..end,
            "\nThe module has no scoped orchestrator or delegation link.\n",
        );
        architalk::document_format::validate_profile(
            &main,
            architalk::document_format::ValidationProfile::Principal,
        )
        .unwrap();
        broken["markdown"] = json!(main);
        fs::write(
            f.tools.path().join("design-package.json"),
            broken.to_string(),
        )
        .unwrap();
        let mut events = f.app.events.subscribe();
        f.draft(json!({}));
        settled(&f.app).await;
        assert_eq!(attempts(&mut events), 10);
        let before = f.app.workspace().unwrap();
        assert_eq!(before["runs"].as_array().unwrap().len(), 1);
        assert_eq!(before["sessions"][0]["drafts"], json!([]));
        let prefix = before["sessions"][0]["design_generations"][0]["completed"]
            .as_array()
            .unwrap()
            .clone();
        assert_eq!(prefix.len(), 9);
        assert!(
            before["runs"][0]["error"]["message"]
                .as_str()
                .unwrap()
                .contains("아키텍처-설계.md")
        );
        assert!(
            before["runs"][0]["error"]["message"]
                .as_str()
                .unwrap()
                .contains("Method-R module design needs its own orchestrator")
        );
        fs::write(
            f.tools.path().join("design-package.json"),
            package.to_string(),
        )
        .unwrap();
        f.control(json!({}));
        let mut events = f.app.events.subscribe();
        f.draft(json!({}));
        settled(&f.app).await;
        assert_eq!(attempts(&mut events), 2);
        let s = f.app.session(&f.sid).unwrap();
        assert_eq!(
            &s["design_generations"][0]["completed"].as_array().unwrap()[..9],
            prefix.as_slice()
        );
        assert_eq!(s["drafts"].as_array().unwrap().len(), 1);
        assert!(!s["design_generations"][0]["draft_id"].is_null());
    }

    #[tokio::test]
    async fn live_settings_changes_do_not_change_captured_document_requests() {
        let f = Fixture::new();
        f.control(json!({"hold_kind":"quality_scenarios"}));
        f.draft(json!({}));
        at_prefix(&f, 1, "품질속성").await;
        let snapshot = f.app.settings().unwrap();
        let mut changed = snapshot["settings"].clone();
        changed["ai"]["timeout_seconds"] = json!(900);
        changed["ai"]["backends"]["claude"]["model"] = json!("next-run-model");
        changed["documents"]["language"] = json!("en");
        f.app
            .command(
                "settings",
                None,
                command(
                    snapshot["revision"].as_u64().unwrap(),
                    json!({"settings":changed}),
                ),
            )
            .unwrap();
        f.control(json!({}));
        settled(&f.app).await;
        let w = f.app.workspace().unwrap();
        let cp = &w["sessions"][0]["design_generations"][0];
        assert_eq!(cp["language"], "ko");
        assert_eq!(
            cp["completed"].as_array().unwrap().len(),
            10,
            "{}",
            w["runs"]
        );
        for part in cp["completed"].as_array().unwrap() {
            assert_eq!(part["effective_settings"]["timeout_seconds"], 1800);
            assert_eq!(part["effective_settings"]["model"], "cli_default");
        }
        assert_eq!(w["runs"][1]["effective_settings"]["timeout_seconds"], 900);
        f.control(json!({"fail_kind":"drivers"}));
        f.draft(json!({}));
        settled(&f.app).await;
        let s = f.app.session(&f.sid).unwrap();
        assert_eq!(s["design_generations"].as_array().unwrap().len(), 2);
        assert_eq!(s["design_generations"][1]["language"], "en");
    }

    #[tokio::test]
    async fn interrupted_checkpoint_requires_manual_confirmation_before_resume() {
        let f = Fixture::new();
        f.control(json!({"fail_kind":"decisions"}));
        f.draft(json!({}));
        settled(&f.app).await;
        let mut seeded: Workspace = serde_json::from_value(f.app.workspace().unwrap()).unwrap();
        let checkpoint = seeded.sessions[0].design_generations[0].clone();
        let rid = seeded.runs[0].id.clone();
        seeded.runs[0].status = "running".into();
        seeded.sessions[0].active_run_id = Some(rid.clone());
        let Fixture {
            app,
            project,
            tools,
            settings,
            sid,
        } = f;
        drop(app);
        let mut store = Store::open(project.path()).unwrap();
        store
            .commit_workspace(&serde_json::to_value(seeded).unwrap(), None, None)
            .unwrap();
        drop(store);
        let app = App::open(project.path(), settings).unwrap();
        assert!(!app.is_idle());
        let before = app.session(&sid).unwrap();
        assert_eq!(
            before["design_generations"][0],
            serde_json::to_value(checkpoint).unwrap()
        );
        let revision = before["revision"].as_u64().unwrap();
        assert_eq!(
            app.command("draft", Some(&sid), command(revision, json!({})))
                .unwrap_err()
                .code,
            "run_busy"
        );
        assert_eq!(app.run(&rid).unwrap()["process_termination"], "unknown");
        app.command(
            "recover",
            Some(&rid),
            command(revision, json!({"confirmed":true})),
        )
        .unwrap();
        assert_eq!(
            app.run(&rid).unwrap()["process_termination"],
            "manually_confirmed"
        );
        fs::write(tools.path().join("stage-control.json"), "{}").unwrap();
        let mut events = app.events.subscribe();
        let revision = app.session(&sid).unwrap()["revision"].as_u64().unwrap();
        app.command("draft", Some(&sid), command(revision, json!({})))
            .unwrap();
        settled(&app).await;
        assert_eq!(attempts(&mut events), 8);
        assert_eq!(
            app.session(&sid).unwrap()["drafts"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
    }

    #[tokio::test]
    async fn cancellation_and_workspace_reset_stop_between_documents_without_a_draft() {
        for reset in [false, true] {
            let f = Fixture::new();
            f.control(json!({"hold_kind":"quality_scenarios"}));
            let started = f.draft(json!({}));
            at_prefix(&f, 1, "품질속성").await;
            if reset {
                fs::remove_dir_all(f.project.path().join(".architalk")).unwrap();
                settled(&f.app).await;
                assert_eq!(
                    f.app.workspace().unwrap_err().code,
                    "workspace_reset_required"
                );
                assert!(!f.project.path().join(".architalk").exists());
            } else {
                let revision = f.app.session(&f.sid).unwrap()["revision"].as_u64().unwrap();
                assert_eq!(
                    f.app
                        .command("draft", Some(&f.sid), command(revision, json!({})))
                        .unwrap_err()
                        .code,
                    "run_busy"
                );
                let revision = f.app.session(&f.sid).unwrap()["revision"].as_u64().unwrap();
                f.app
                    .command(
                        "cancel",
                        started["run"]["id"].as_str(),
                        command(revision, json!({})),
                    )
                    .unwrap();
                settled(&f.app).await;
                let s = f.app.session(&f.sid).unwrap();
                assert_eq!(
                    s["design_generations"][0]["completed"]
                        .as_array()
                        .unwrap()
                        .len(),
                    1
                );
                assert_eq!(s["drafts"], json!([]));
                assert_eq!(s["messages"].as_array().unwrap().len(), 1);
                assert_eq!(
                    f.app.run(started["run"]["id"].as_str().unwrap()).unwrap()["status"],
                    "cancelled"
                );
            }
        }
    }
}
