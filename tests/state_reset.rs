use architalk::{
    app::App,
    config::Settings,
    domain,
    http::{self, ServerState},
    model::{Command, Draft, Run, Workspace, hash, id, now},
    storage::{EventInput, Store},
};
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::json;
use std::{fs, path::Path, sync::Arc, time::Duration};
use tower::ServiceExt;

const PRIVATE: &str = "synthetic-old-workspace-must-not-return";

fn seeded(root: &Path) -> (Arc<App>, String, String) {
    let mut session = domain::new_session("reset fixture".into(), PRIVATE.into(), vec![]);
    let sid = session.id.clone();
    session.drafts.push(Draft {
        id: id(),
        input_revision: 0,
        markdown: PRIVATE.into(),
        documents: vec![],
        hash: hash(PRIVATE),
        created_at: now(),
        content_status: "draft".into(),
        reviewed: false,
    });
    let rid = id();
    let run = Run {
        id: rid.clone(),
        session_id: sid.clone(),
        kind: "conversation".into(),
        status: "completed".into(),
        input_revision: 0,
        created_at: now(),
        completed_at: Some(now()),
        effective_settings: json!({}),
        error: None,
        process_termination: "confirmed".into(),
        progress: PRIVATE.into(),
        resolved_model: None,
        cli_version: None,
    };
    let mut workspace = Workspace::new(Settings::default());
    workspace.sessions.push(session);
    workspace.runs.push(run);
    let mut store = Store::open(root).unwrap();
    store
        .commit_workspace(
            &serde_json::to_value(&workspace).unwrap(),
            None,
            Some(EventInput {
                session_id: Some(sid.clone()),
                kind: "old_record".into(),
                payload: json!({"text":PRIVATE}),
            }),
        )
        .unwrap();
    store
        .commit_workspace(
            &serde_json::to_value(&workspace).unwrap(),
            None,
            Some(EventInput {
                session_id: Some(sid.clone()),
                kind: "second_old_record".into(),
                payload: json!({"text":PRIVATE}),
            }),
        )
        .unwrap();
    drop(store);
    (App::open(root, Settings::default()).unwrap(), sid, rid)
}

fn router(app: Arc<App>) -> axum::Router {
    http::router(ServerState {
        app,
        token: "reset-test-capability".into(),
        authority: "127.0.0.1:7788".into(),
    })
}

fn request(path: &str) -> Request<Body> {
    Request::builder()
        .uri(path)
        .header("host", "127.0.0.1:7788")
        .header("authorization", "Bearer reset-test-capability")
        .body(Body::empty())
        .unwrap()
}

#[test]
fn deleted_state_directory_stops_cached_reads_and_shutdown_never_recreates_it() {
    let root = tempfile::tempdir().unwrap();
    let (app, sid, rid) = seeded(root.path());
    fs::remove_dir_all(root.path().join(".architalk")).unwrap();
    assert_eq!(
        app.workspace().unwrap_err().code,
        "workspace_reset_required"
    );
    assert_eq!(
        app.session(&sid).unwrap_err().code,
        "workspace_reset_required"
    );
    assert_eq!(app.run(&rid).unwrap_err().code, "workspace_reset_required");
    assert_eq!(
        app.draft(&sid).unwrap_err().code,
        "workspace_reset_required"
    );
    assert_eq!(
        app.replay(Some(&sid), 0).unwrap_err().code,
        "workspace_reset_required"
    );
    app.stop();
    drop(app);
    assert!(!root.path().join(".architalk").exists());
    let restarted = App::open(root.path(), Settings::default()).unwrap();
    assert!(
        restarted.workspace().unwrap()["sessions"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}

#[test]
fn replacing_the_state_name_latches_reset_and_preserves_both_directories() {
    let root = tempfile::tempdir().unwrap();
    let mut store = Store::open(root.path()).unwrap();
    store
        .commit_workspace(&json!({"private":PRIVATE}), None, None)
        .unwrap();
    let original = fs::read(root.path().join(".architalk/workspace.db")).unwrap();
    fs::rename(
        root.path().join(".architalk"),
        root.path().join("saved-state"),
    )
    .unwrap();
    fs::create_dir(root.path().join(".architalk")).unwrap();
    assert!(
        store
            .load_workspace()
            .unwrap_err()
            .to_string()
            .contains("workspace_reset_required")
    );
    assert!(
        store
            .commit_workspace(&json!({"resurrected":true}), None, None)
            .is_err()
    );
    assert_eq!(
        fs::read(root.path().join("saved-state/workspace.db")).unwrap(),
        original
    );
    assert!(
        fs::read_dir(root.path().join(".architalk"))
            .unwrap()
            .next()
            .is_none()
    );
    fs::remove_dir(root.path().join(".architalk")).unwrap();
    fs::rename(
        root.path().join("saved-state"),
        root.path().join(".architalk"),
    )
    .unwrap();
    assert!(
        store.load_workspace().is_err(),
        "restoring a name cannot revive an invalidated Store"
    );
}

#[tokio::test]
async fn unlinking_only_the_database_returns_http410_and_no_previous_payload() {
    let root = tempfile::tempdir().unwrap();
    let (app, sid, rid) = seeded(root.path());
    fs::remove_file(root.path().join(".architalk/workspace.db")).unwrap();
    let router = router(app);
    for path in [
        "/api/v1/workspace".to_owned(),
        "/api/v1/settings".into(),
        format!("/api/v1/sessions/{sid}"),
        format!("/api/v1/sessions/{sid}/draft"),
        format!("/api/v1/runs/{rid}"),
    ] {
        let response = router.clone().oneshot(request(&path)).await.unwrap();
        assert_eq!(response.status(), StatusCode::GONE, "{path}");
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let text = String::from_utf8(bytes.to_vec()).unwrap();
        assert!(text.contains("workspace_reset_required"));
        assert!(!text.contains(PRIVATE));
    }
    assert!(!root.path().join(".architalk/workspace.db").exists());
}

#[tokio::test]
async fn an_existing_sse_stream_sends_reset_instead_of_old_replay_or_delta() {
    let root = tempfile::tempdir().unwrap();
    let (app, sid, _) = seeded(root.path());
    let response = router(app)
        .oneshot(request(&format!("/api/v1/sessions/{sid}/events")))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    fs::remove_dir_all(root.path().join(".architalk")).unwrap();
    let mut body = response.into_body();
    let frame = tokio::time::timeout(Duration::from_secs(2), body.frame())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    let text = String::from_utf8(frame.into_data().unwrap().to_vec()).unwrap();
    assert!(text.contains("event: workspace_reset_required"), "{text}");
    assert!(!text.contains(PRIVATE));
}

#[tokio::test]
async fn a_partially_consumed_replay_cannot_emit_its_next_old_record_after_reset_and_stop() {
    let root = tempfile::tempdir().unwrap();
    let (app, sid, _) = seeded(root.path());
    let response = router(app.clone())
        .oneshot(request(&format!("/api/v1/sessions/{sid}/events")))
        .await
        .unwrap();
    let mut body = response.into_body();
    let first = body.frame().await.unwrap().unwrap().into_data().unwrap();
    assert!(String::from_utf8(first.to_vec()).unwrap().contains(PRIVATE));
    fs::remove_dir_all(root.path().join(".architalk")).unwrap();
    app.stop();
    let next = tokio::time::timeout(Duration::from_secs(2), body.frame())
        .await
        .unwrap()
        .unwrap()
        .unwrap()
        .into_data()
        .unwrap();
    let text = String::from_utf8(next.to_vec()).unwrap();
    assert!(text.contains("event: workspace_reset_required"), "{text}");
    assert!(!text.contains(PRIVATE));
    assert!(body.frame().await.is_none());
}

#[test]
fn symlink_replacement_never_follows_or_writes_to_another_directory() {
    use std::os::unix::fs::symlink;
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    fs::write(outside.path().join("marker"), b"preserved").unwrap();
    let (app, _, _) = seeded(root.path());
    fs::remove_dir_all(root.path().join(".architalk")).unwrap();
    symlink(outside.path(), root.path().join(".architalk")).unwrap();
    assert_eq!(
        app.workspace().unwrap_err().code,
        "workspace_reset_required"
    );
    app.stop();
    assert_eq!(
        fs::read(outside.path().join("marker")).unwrap(),
        b"preserved"
    );
    assert_eq!(fs::read_dir(outside.path()).unwrap().count(), 1);
}

#[cfg(target_os = "macos")]
#[tokio::test]
async fn a_quiet_active_process_is_cancelled_and_cannot_commit_or_recreate_deleted_state() {
    use std::os::unix::fs::PermissionsExt;
    for replacement in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let executable_dir = tempfile::tempdir().unwrap();
        let executable = executable_dir.path().join("fixture.py");
        fs::write(&executable, include_str!("fixtures/claude-workflow.py")).unwrap();
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
        let mut settings = Settings::default();
        settings.ai.backends.claude.executable = executable.to_string_lossy().into();
        let app = App::open(root.path(), settings).unwrap();
        let mut events = app.events.subscribe();
        app.command(
            "create",
            None,
            Command {
                command_id: id(),
                expected_revision: 0,
                payload: json!({"title":"reset active fixture","intro":format!("SLOW_REQUEST {PRIVATE}"),"context_paths":[]}),
            },
        )
        .unwrap();
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                if events.recv().await.unwrap().payload["message"]
                    == "Claude is generating a response."
                {
                    break;
                }
            }
        })
        .await
        .expect("the actual fixture subprocess must reach its quiet 30-second wait");
        while events.try_recv().is_ok() {}
        let state = root.path().join(".architalk");
        let saved = root.path().join("saved-state");
        let database = fs::read(state.join("workspace.db")).unwrap();
        if replacement {
            fs::rename(&state, &saved).unwrap();
            fs::create_dir(&state).unwrap();
        } else {
            fs::remove_dir_all(&state).unwrap();
        }
        // is_idle does not touch the Store while workers > 0. Therefore no
        // getter or command triggers cancellation during this bounded wait.
        let terminated = tokio::time::timeout(Duration::from_secs(3), async {
            while !app.is_idle() {
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await;
        app.stop(); // Always cancel/clean up, even if the independent watcher failed.
        assert!(
            terminated.is_ok(),
            "quiet workers must stop before their normal response"
        );
        assert!(
            events.try_recv().is_err(),
            "no late completion/delta can be broadcast"
        );
        assert_eq!(
            app.workspace().unwrap_err().code,
            "workspace_reset_required"
        );
        drop(app);
        if replacement {
            assert_eq!(fs::read(saved.join("workspace.db")).unwrap(), database);
            assert_eq!(fs::read_dir(&state).unwrap().count(), 0);
        } else {
            assert!(!state.exists());
        }
    }
}
