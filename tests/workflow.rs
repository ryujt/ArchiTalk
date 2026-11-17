use architalk::{
    app::App,
    config::Settings,
    domain,
    http::{self, ServerState},
    model::{Command, Workspace},
};
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use std::{sync::Arc, time::Duration};
use tower::ServiceExt;

fn cmd(revision: u64, payload: Value) -> Command {
    Command {
        command_id: uuid::Uuid::new_v4().to_string(),
        expected_revision: revision,
        payload,
    }
}
#[test]
fn unknown_quote_cannot_be_promoted_and_risk_cannot_be_hidden() {
    let mut s = domain::new_session(
        "결제 도우미".into(),
        "결제 결과가 중복되면 안 됩니다. 규모는 아직 모르겠어요.".into(),
        vec![],
    );
    let source = s.messages[0].id.clone();
    let candidate = json!({"reply":"미정은 미정으로 남깁니다.","coverage_updates":[{"id":"scope","reason":"임의의 회원 수","evidence_refs":[source],"source_quote":"규모는 아직 모르겠어요."}],"questions":[{"target_ref":"payment_recovery","text":"결과를 확인할 수 없을 때 누가 복구하나요?","reason":"금전 실패","priority_class":4}]});
    domain::apply_conversation(&mut s, serde_json::from_value(candidate).unwrap()).unwrap();
    assert_eq!(
        s.coverage.iter().find(|c| c.id == "scope").unwrap().status,
        "unknown"
    );
    assert!(
        s.coverage
            .iter()
            .find(|c| c.id == "payment_recovery")
            .unwrap()
            .blocker
    );
    assert_eq!(domain::select_question(&s).unwrap().priority_class, 2);
    let wrong = json!({"reply":"x","claims":[{"text":"회원 2000명","evidence_refs":[source],"source_quote":"회원 2000명"}]});
    assert!(domain::apply_conversation(&mut s, serde_json::from_value(wrong).unwrap()).is_err());
}

#[tokio::test]
async fn http_enforces_token_host_origin_and_immutable_settings() {
    let root = tempfile::tempdir().unwrap();
    let app = App::open(root.path(), Settings::default()).unwrap();
    let router = http::router(ServerState {
        app: app.clone(),
        token: "secret-capability".into(),
        authority: "127.0.0.1:7788".into(),
    });
    let request =
        |auth: bool, host: &str, origin: Option<&str>, method: &str, path: &str, body: String| {
            let mut r = Request::builder()
                .uri(path)
                .method(method)
                .header("host", host)
                .header("content-type", "application/json");
            if auth {
                r = r.header("authorization", "Bearer secret-capability");
            }
            if let Some(o) = origin {
                r = r.header("origin", o);
            }
            r.body(Body::from(body)).unwrap()
        };
    assert_eq!(
        router
            .clone()
            .oneshot(request(
                false,
                "127.0.0.1:7788",
                None,
                "GET",
                "/api/v1/workspace",
                String::new()
            ))
            .await
            .unwrap()
            .status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        router
            .clone()
            .oneshot(request(
                true,
                "evil.example:7788",
                None,
                "GET",
                "/api/v1/workspace",
                String::new()
            ))
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        router
            .clone()
            .oneshot(request(
                true,
                "127.0.0.1:7788",
                Some("http://evil.example"),
                "GET",
                "/api/v1/workspace",
                String::new()
            ))
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );
    let settings = serde_json::to_value(Settings::default()).unwrap();
    let body = serde_json::to_string(&cmd(0, json!({"settings":settings}))).unwrap();
    assert_eq!(
        router
            .clone()
            .oneshot(request(
                true,
                "127.0.0.1:7788",
                None,
                "PATCH",
                "/api/v1/settings",
                body.clone()
            ))
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );
    let response = router
        .clone()
        .oneshot(request(
            true,
            "127.0.0.1:7788",
            Some("http://127.0.0.1:7788"),
            "PATCH",
            "/api/v1/settings",
            body,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let mut settings = Settings::default();
    settings.ai.backends.claude.executable = "/tmp/untrusted".into();
    let response = router
        .clone()
        .oneshot(request(
            true,
            "127.0.0.1:7788",
            Some("http://127.0.0.1:7788"),
            "PATCH",
            "/api/v1/settings",
            serde_json::to_string(&cmd(1, json!({"settings":settings}))).unwrap(),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let response = router
        .oneshot(request(
            false,
            "127.0.0.1:7788",
            None,
            "GET",
            "/app.js",
            String::new(),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    assert!(!body.is_empty());
}

#[test]
fn reopening_a_major_finding_revokes_approval_and_invalidates_preview() {
    use architalk::model::*;
    let root = tempfile::tempdir().unwrap();
    let mut store = architalk::storage::Store::open(root.path()).unwrap();
    let mut s = domain::new_session(
        "검토 계약".into(),
        "메모를 로컬 파일에 저장합니다.".into(),
        vec![],
    );
    let source = s.messages[0].id.clone();
    for c in &mut s.coverage {
        c.status = "covered".into();
        c.evidence_refs = vec![source.clone()];
    }
    s.requirements.push(Requirement {
        id: "REQ-1".into(),
        text: "메모 저장".into(),
        acceptance: "추가 후 파일에서 같은 메모를 읽음".into(),
        evidence_refs: vec![source],
        status: "proposed".into(),
        supersedes: None,
        source_quote: "메모를 로컬 파일에 저장합니다.".into(),
    });
    let draft = Draft {
        id: id(),
        input_revision: s.design_revision,
        markdown: "# 메모 설계\n로컬 파일에 저장합니다.".into(),
        hash: hash("# 메모 설계\n로컬 파일에 저장합니다."),
        created_at: now(),
        content_status: "approved".into(),
        reviewed: true,
    };
    let did = draft.id.clone();
    let fid = id();
    s.drafts.push(draft);
    s.content_status = "approved".into();
    s.findings.push(Finding {
        id: fid.clone(),
        draft_id: did.clone(),
        severity: "major".into(),
        text: "복구 책임 미확인".into(),
        evidence: "실패 시나리오".into(),
        impact: "데이터 손실".into(),
        proposed_resolution: "복구 계획 명시".into(),
        disposition: Some("resolved".into()),
        disposition_reason: Some("처리됨".into()),
    });
    let sid = s.id.clone();
    let mut w = Workspace::new(Settings::default());
    w.sessions.push(s);
    store
        .commit_workspace(&serde_json::to_value(w).unwrap(), None, None)
        .unwrap();
    drop(store);
    let app = App::open(root.path(), Settings::default()).unwrap();
    let preview = app
        .command(
            "export",
            Some(&sid),
            cmd(0, json!({"mode":"preview","draft_id":did})),
        )
        .unwrap();
    app.command("review",Some(&sid),cmd(0,json!({"action":"resolve_finding","finding_id":fid,"disposition":"accepted","reason":"아직 수정되지 않아 다시 수용합니다."}))).unwrap();
    let s = app.session(&sid).unwrap();
    assert_eq!(s["content_status"], "draft");
    assert_eq!(s["drafts"][0]["content_status"], "draft");
    let error = app
        .command(
            "export",
            Some(&sid),
            cmd(
                s["revision"].as_u64().unwrap(),
                json!({"mode":"apply","plan_id":preview["export"]["id"]}),
            ),
        )
        .unwrap_err();
    assert_eq!(error.code, "preview_stale");
    let preview = app
        .command(
            "export",
            Some(&sid),
            cmd(
                s["revision"].as_u64().unwrap(),
                json!({"mode":"preview","draft_id":did}),
            ),
        )
        .unwrap();
    let content = preview["plan"]["files"][0]["content"].as_str().unwrap();
    assert!(content.contains("내용 상태: draft"));
    assert!(content.contains("accepted"));
}

#[test]
fn initial_snapshot_does_not_override_user_config_but_explicit_web_settings_do() {
    let root = tempfile::tempdir().unwrap();
    drop(App::open(root.path(), Settings::default()).unwrap());
    let mut global = Settings::default();
    global.ai.backends.claude.model = Some("configured-model".into());
    let app = App::open(root.path(), global.clone()).unwrap();
    assert_eq!(
        app.settings()["settings"]["ai"]["backends"]["claude"]["model"],
        "configured-model"
    );
    let mut web = global.clone();
    web.ai.backends.claude.model = Some("web-override".into());
    app.command("settings", None, cmd(0, json!({"settings":web})))
        .unwrap();
    drop(app);
    global.ai.backends.claude.model = Some("new-global-model".into());
    let app = App::open(root.path(), global).unwrap();
    assert_eq!(
        app.settings()["settings"]["ai"]["backends"]["claude"]["model"],
        "web-override"
    );
}

#[cfg(target_os = "macos")]
fn fixture(dir: &std::path::Path) -> String {
    use std::os::unix::fs::PermissionsExt;
    let path = dir.join("fixture-cli.py");
    std::fs::write(&path, include_str!("fixtures/claude-workflow.py")).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
    path.to_str().unwrap().into()
}
#[cfg(target_os = "macos")]
async fn settled(app: &Arc<App>) {
    tokio::time::timeout(Duration::from_secs(30), async {
        while !app.is_idle() {
            tokio::time::sleep(Duration::from_millis(30)).await;
        }
    })
    .await
    .unwrap();
}
#[cfg(target_os = "macos")]
#[tokio::test]
async fn diagnostics_reserves_the_project_slot_before_spawning() {
    let root = tempfile::tempdir().unwrap();
    let external = tempfile::tempdir().unwrap();
    let executable = fixture(external.path());
    let code = std::fs::read_to_string(&executable).unwrap().replace(
        "if '--help' in sys.argv:\n",
        "if '--help' in sys.argv:\n time.sleep(0.5)\n",
    );
    std::fs::write(&executable, code).unwrap();
    let mut settings = Settings::default();
    settings.ai.backends.claude.executable = executable;
    settings.ai.backends.codex.executable = "/nonexistent/architalk-cli".into();
    let app = App::open(root.path(), settings).unwrap();
    let worker = app.clone();
    let diagnostic = tokio::spawn(async move { worker.diagnostics().await });
    tokio::time::timeout(Duration::from_secs(2), async {
        while app.workspace().unwrap()["active_run"].is_null() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    let w = app.workspace().unwrap();
    assert_eq!(w["active_run"]["kind"], "diagnostic");
    let result = app.command(
        "create",
        None,
        cmd(
            w["workspace_revision"].as_u64().unwrap(),
            json!({"title":"겹치는 요청","intro":"진단 중 새 모델 호출","context_paths":[]}),
        ),
    );
    assert_eq!(result.unwrap_err().code, "run_busy");
    assert_eq!(app.workspace().unwrap()["sessions"], json!([]));
    diagnostic.await.unwrap().unwrap();
    assert!(app.is_idle());
}

#[cfg(target_os = "macos")]
#[tokio::test]
async fn critique_preparation_failure_keeps_completed_design_instead_of_pending() {
    use architalk::model::*;
    let root = tempfile::tempdir().unwrap();
    let external = tempfile::tempdir().unwrap();
    let mut settings = Settings::default();
    settings.ai.backends.claude.executable = fixture(external.path());
    let mut s = domain::new_session(
        "큰 문맥".into(),
        "작은 설계 문서를 만들어 주세요.".into(),
        vec![],
    );
    for _ in 0..20 {
        s.messages.push(Message {
            id: id(),
            role: "user".into(),
            content: "업무 설명 ".repeat(1800),
            created_at: now(),
        });
    }
    for n in 0..4 {
        s.evidence.push(Evidence {
            id: id(),
            path: format!("context-{n}.md"),
            hash: "fixture".into(),
            text: "a".repeat(80_000),
        });
    }
    let before = domain::prompt(&s, "design", "ko").unwrap().len();
    assert!(before < 900_000);
    let sid = s.id.clone();
    let mut w = Workspace::new(settings.clone());
    w.sessions.push(s);
    let mut store = architalk::storage::Store::open(root.path()).unwrap();
    store
        .commit_workspace(&serde_json::to_value(w).unwrap(), None, None)
        .unwrap();
    drop(store);
    let app = App::open(root.path(), settings).unwrap();
    app.command("draft", Some(&sid), cmd(0, json!({}))).unwrap();
    settled(&app).await;
    let w = app.workspace().unwrap();
    assert_eq!(w["runs"][0]["status"], "completed");
    assert_eq!(w["runs"][1]["error"]["code"], "review_preparation_failed");
    let s = app.session(&sid).unwrap();
    assert_eq!(s["drafts"].as_array().unwrap().len(), 1);
    assert!(s["active_run_id"].is_null());
}
#[cfg(target_os = "macos")]
#[tokio::test]
async fn full_controller_workflow_uses_real_subprocess_db_files_and_recovers() {
    let root = tempfile::tempdir().unwrap();
    let external = tempfile::tempdir().unwrap();
    let mut settings = Settings::default();
    settings.ai.backends.claude.executable = fixture(external.path());
    let app = App::open(root.path(), settings.clone()).unwrap();
    let create = cmd(
        0,
        json!({"title":"회원 알림 서비스","intro":"가입 안내 알림을 보내고 싶어요. 규모는 아직 모르겠어요.","context_paths":[]}),
    );
    let result = app.command("create", None, create.clone()).unwrap();
    let sid = result["session"]["id"].as_str().unwrap().to_owned();
    let run_id = result["run"]["id"].as_str().unwrap().to_owned();
    assert!(
        app.command(
            "create",
            None,
            cmd(
                1,
                json!({"title":"중복 실행","intro":"다른 대화","context_paths":[]})
            )
        )
        .is_err()
    );
    settled(&app).await;
    assert_eq!(app.run(&run_id).unwrap()["status"], "completed");
    assert_eq!(app.command("create", None, create.clone()).unwrap(), result); // stale expected_revision still replays receipt.
    let mut changed = create;
    changed.payload["intro"] = json!("different");
    assert_eq!(
        app.command("create", None, changed).unwrap_err().code,
        "idempotency_conflict"
    );
    let s = app.session(&sid).unwrap();
    assert_eq!(s["claims"][0]["status"], "proposed");
    assert_eq!(
        s["coverage"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["id"] == "scope")
            .unwrap()["status"],
        "unknown"
    );
    let first_revision = s["revision"].as_u64().unwrap();
    app.command("draft", Some(&sid), cmd(first_revision, json!({})))
        .unwrap();
    settled(&app).await;
    let s = app.session(&sid).unwrap();
    assert_eq!(s["drafts"][0]["reviewed"], true);
    let did = s["drafts"][0]["id"].as_str().unwrap();
    let d_rev = s["drafts"][0]["input_revision"].clone();
    let error = app
        .command(
            "review",
            Some(&sid),
            cmd(
                s["revision"].as_u64().unwrap(),
                json!({"action":"approve","draft_id":did}),
            ),
        )
        .unwrap_err();
    assert_eq!(error.code, "not_ready");
    let preview = app
        .command(
            "export",
            Some(&sid),
            cmd(
                s["revision"].as_u64().unwrap(),
                json!({"mode":"preview","draft_id":did}),
            ),
        )
        .unwrap();
    let pid = preview["export"]["id"].as_str().unwrap();
    let saved = app
        .command(
            "export",
            Some(&sid),
            cmd(
                s["revision"].as_u64().unwrap(),
                json!({"mode":"apply","plan_id":pid}),
            ),
        )
        .unwrap();
    assert_eq!(saved["export"]["status"], "completed");
    let path = saved["export"]["result"]["paths"][0]
        .as_str()
        .unwrap()
        .to_owned();
    let original = std::fs::read(root.path().join(&path)).unwrap();
    let s = app.session(&sid).unwrap();
    assert_eq!(s["content_status"], "draft");
    assert_eq!(s["design_revision"], d_rev); // Metadata did not stale the document.
    app.command(
        "message",
        Some(&sid),
        cmd(
            s["revision"].as_u64().unwrap(),
            json!({"content":"회원 수는 아직 미정이에요. 원본은 보존해 주세요."}),
        ),
    )
    .unwrap();
    settled(&app).await;
    let s = app.session(&sid).unwrap();
    let old = s["drafts"][0]["id"].as_str().unwrap();
    let preview = app
        .command(
            "export",
            Some(&sid),
            cmd(
                s["revision"].as_u64().unwrap(),
                json!({"mode":"preview","draft_id":old}),
            ),
        )
        .unwrap();
    assert!(
        preview["plan"]["relative_directory"]
            .as_str()
            .unwrap()
            .contains("개정-2")
    );
    let saved = app
        .command(
            "export",
            Some(&sid),
            cmd(
                s["revision"].as_u64().unwrap(),
                json!({"mode":"apply","plan_id":preview["export"]["id"]}),
            ),
        )
        .unwrap();
    assert_eq!(saved["export"]["status"], "completed");
    assert_eq!(std::fs::read(root.path().join(path)).unwrap(), original);
    let snapshot = app.workspace().unwrap();
    assert_eq!(snapshot["sessions"].as_array().unwrap().len(), 1);
    drop(app);
    let app = App::open(root.path(), settings.clone()).unwrap();
    assert_eq!(
        app.session(&sid).unwrap()["drafts"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    drop(app);
    // Persist an interrupted process as if the host died. The real reopen path must keep the gate.
    let mut store = architalk::storage::Store::open(root.path()).unwrap();
    let mut w: Workspace =
        serde_json::from_value(store.load_workspace().unwrap().unwrap()).unwrap();
    w.runs[0].status = "running".into();
    w.runs[0].process_termination = "pending".into();
    w.sessions[0].active_run_id = Some(w.runs[0].id.clone());
    store
        .commit_workspace(&serde_json::to_value(w).unwrap(), None, None)
        .unwrap();
    drop(store);
    let app = App::open(root.path(), settings).unwrap();
    assert!(!app.is_idle());
    let s = app.session(&sid).unwrap();
    assert_eq!(
        app.command(
            "draft",
            Some(&sid),
            cmd(s["revision"].as_u64().unwrap(), json!({}))
        )
        .unwrap_err()
        .code,
        "run_busy"
    );
    app.command(
        "recover",
        Some(&run_id),
        cmd(s["revision"].as_u64().unwrap(), json!({"confirmed":true})),
    )
    .unwrap();
    assert!(app.is_idle());
}
