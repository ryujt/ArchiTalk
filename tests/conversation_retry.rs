use architalk::{
    app::App,
    config::{self, Settings},
    domain,
    http::{self, ServerState},
    model::{Command, Draft, Run, Session, Workspace, hash, id, now},
    storage::Store,
};
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use std::{path::Path, sync::Arc, time::Duration};
use tower::ServiceExt;

fn command(revision: u64) -> Command {
    Command {
        command_id: id(),
        expected_revision: revision,
        payload: json!({}),
    }
}

fn run(session: &Session, kind: &str, status: &str, termination: &str) -> Run {
    Run {
        id: id(),
        session_id: session.id.clone(),
        kind: kind.into(),
        status: status.into(),
        input_revision: session.design_revision,
        created_at: now(),
        completed_at: Some(now()),
        effective_settings: json!({"model":"old-model"}),
        error: None,
        process_termination: termination.into(),
        progress: "synthetic failure".into(),
        resolved_model: None,
        cli_version: None,
    }
}

fn seed(
    root: &Path,
    latest: Option<(&str, &str, &str)>,
    other_active: bool,
) -> (Session, Settings) {
    let mut settings = Settings::default();
    // A retry really enters the async adapter but cannot invoke a model.
    settings.ai.backends.claude.executable =
        root.join("no-such-fixture-cli").to_string_lossy().into();
    let mut session = domain::new_session(
        "retry fixture".into(),
        "Original user requirement stays exact.".into(),
        vec![],
    );
    session.revision = 11;
    session.design_revision = 7;
    session.drafts.push(Draft {
        id: id(),
        input_revision: 6,
        markdown: "# Existing legacy draft".into(),
        documents: vec![],
        hash: hash("# Existing legacy draft"),
        created_at: now(),
        content_status: "draft".into(),
        reviewed: false,
    });
    let mut workspace = Workspace::new(settings.clone());
    workspace.sessions.push(session.clone());
    // A historical failed conversation must not authorize retry after a later
    // completed conversation or design/critic operation.
    workspace
        .runs
        .push(run(&session, "conversation", "failed", "confirmed"));
    if let Some((kind, status, termination)) = latest {
        workspace
            .runs
            .push(run(&session, kind, status, termination));
    } else {
        workspace.runs.clear();
    }
    if other_active {
        let other = domain::new_session("other".into(), "Other session".into(), vec![]);
        workspace
            .runs
            .push(run(&other, "conversation", "running", "pending"));
        workspace.sessions.push(other);
    }
    let mut store = Store::open(root).unwrap();
    store
        .commit_workspace(&serde_json::to_value(workspace).unwrap(), None, None)
        .unwrap();
    (session, settings)
}

fn semantics(mut value: Value) -> Value {
    let object = value.as_object_mut().unwrap();
    object.remove("revision");
    object.remove("active_run_id");
    // GET adds transport/computed metadata to the persisted Session DTO.
    object.remove("cursor");
    object.remove("readiness_issues");
    value
}

async fn settled(app: &Arc<App>) {
    tokio::time::timeout(Duration::from_secs(3), async {
        while !app.is_idle() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn retry_preserves_user_semantics_and_drafts_and_reads_current_shared_ai() {
    let root = tempfile::tempdir().unwrap();
    let common = tempfile::tempdir().unwrap();
    let path = common.path().join("config.toml");
    let (session, settings) = seed(
        root.path(),
        Some(("conversation", "failed", "confirmed")),
        false,
    );
    std::fs::write(&path, toml::to_string(&settings).unwrap()).unwrap();
    let app = App::open_with_shared_config(root.path(), &path).unwrap();
    let mut changed = settings.ai.clone();
    changed.roles.conversation.model = Some("new-shared-model".into());
    config::save_ai(&path, &changed, &config::ai_hash(&settings.ai).unwrap()).unwrap();
    let result = app
        .command(
            "retry_conversation",
            Some(&session.id),
            command(session.revision),
        )
        .unwrap();
    assert_eq!(
        semantics(result["session"].clone()),
        semantics(serde_json::to_value(&session).unwrap())
    );
    assert_eq!(result["session"]["revision"], 12);
    assert_eq!(result["run"]["kind"], "conversation");
    assert_eq!(result["run"]["input_revision"], 7);
    assert_eq!(
        result["run"]["effective_settings"]["model"],
        "new-shared-model"
    );
    assert_eq!(result["session"]["active_run_id"], result["run"]["id"]);
    assert_eq!(
        app.command("retry_conversation", Some(&session.id), command(12))
            .unwrap_err()
            .code,
        "run_busy",
        "a second explicit click cannot launch another active retry"
    );
    settled(&app).await;
    assert_eq!(
        semantics(app.session(&session.id).unwrap()),
        semantics(serde_json::to_value(session).unwrap())
    );
}

#[tokio::test]
async fn retry_rejects_ineligible_latest_runs_and_unconfirmed_or_active_processes() {
    for (latest, other_active, expected) in [
        (None, false, "retry_not_allowed"),
        (
            Some(("conversation", "completed", "confirmed")),
            false,
            "retry_not_allowed",
        ),
        (
            Some(("design", "failed", "confirmed")),
            false,
            "retry_not_allowed",
        ),
        (
            Some(("critic", "failed", "confirmed")),
            false,
            "retry_not_allowed",
        ),
        (
            Some(("conversation", "failed", "pending")),
            false,
            "retry_not_allowed",
        ),
        (
            Some(("conversation", "interrupted", "unknown")),
            false,
            "run_busy",
        ),
        (
            Some(("conversation", "failed", "confirmed")),
            true,
            "run_busy",
        ),
    ] {
        let root = tempfile::tempdir().unwrap();
        let (session, settings) = seed(root.path(), latest, other_active);
        let app = App::open(root.path(), settings).unwrap();
        let before = app.workspace().unwrap();
        let error = app
            .command("retry_conversation", Some(&session.id), command(11))
            .unwrap_err();
        assert_eq!(
            error.code, expected,
            "{latest:?}, other_active={other_active}"
        );
        assert_eq!(app.workspace().unwrap(), before);
    }
}

#[tokio::test]
async fn cancelled_and_manually_recovered_interrupted_conversations_can_be_retried() {
    for status in ["cancelled", "interrupted"] {
        let root = tempfile::tempdir().unwrap();
        let termination = if status == "interrupted" {
            "unknown"
        } else {
            "confirmed"
        };
        let (session, settings) = seed(
            root.path(),
            Some(("conversation", status, termination)),
            false,
        );
        let app = App::open(root.path(), settings).unwrap();
        let mut revision = 11;
        if status == "interrupted" {
            let rid = app.workspace().unwrap()["runs"]
                .as_array()
                .unwrap()
                .last()
                .unwrap()["id"]
                .as_str()
                .unwrap()
                .to_owned();
            let mut recover = command(revision);
            recover.payload = json!({"confirmed":true});
            app.command("recover", Some(&rid), recover).unwrap();
            revision += 1;
        }
        let result = app
            .command("retry_conversation", Some(&session.id), command(revision))
            .unwrap();
        assert_eq!(
            semantics(result["session"].clone()),
            semantics(serde_json::to_value(session).unwrap())
        );
        settled(&app).await;
    }
}

fn request(sid: &str, command: &Command) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(format!("/api/v1/sessions/{sid}/conversation/retry"))
        .header("host", "127.0.0.1:7788")
        .header("origin", "http://127.0.0.1:7788")
        .header("authorization", "Bearer retry-test")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(command).unwrap()))
        .unwrap()
}

#[tokio::test]
async fn retry_http_accepts_once_replays_receipt_and_checks_revision_and_command_identity() {
    let root = tempfile::tempdir().unwrap();
    let (session, settings) = seed(
        root.path(),
        Some(("conversation", "failed", "confirmed")),
        false,
    );
    let app = App::open(root.path(), settings).unwrap();
    let router = http::router(ServerState {
        app: app.clone(),
        token: "retry-test".into(),
        authority: "127.0.0.1:7788".into(),
    });
    let retry = command(11);
    let first = router
        .clone()
        .oneshot(request(&session.id, &retry))
        .await
        .unwrap();
    assert_eq!(first.status(), StatusCode::ACCEPTED);
    let bytes = first.into_body().collect().await.unwrap().to_bytes();
    let replay = router
        .clone()
        .oneshot(request(&session.id, &retry))
        .await
        .unwrap();
    assert_eq!(replay.status(), StatusCode::ACCEPTED);
    assert_eq!(
        replay.into_body().collect().await.unwrap().to_bytes(),
        bytes
    );
    let mut changed = retry.clone();
    changed.payload = json!({"changed":true});
    assert_eq!(
        router
            .clone()
            .oneshot(request(&session.id, &changed))
            .await
            .unwrap()
            .status(),
        StatusCode::CONFLICT
    );
    assert_eq!(
        router
            .oneshot(request(&session.id, &command(11)))
            .await
            .unwrap()
            .status(),
        StatusCode::CONFLICT
    );
    settled(&app).await;
    assert_eq!(
        app.workspace().unwrap()["runs"].as_array().unwrap().len(),
        3
    );
    assert_eq!(
        semantics(app.session(&session.id).unwrap()),
        semantics(serde_json::to_value(session).unwrap())
    );
}
