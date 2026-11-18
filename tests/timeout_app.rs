use architalk::{
    app::App,
    config::{self, Settings},
    domain,
    model::{Run, RunError, Workspace, id, now},
    storage::Store,
};
use serde_json::{Value, json};

fn legacy_snapshot(root: &std::path::Path) -> Value {
    let mut settings = Settings::default();
    settings.ai.timeout_seconds = 300;
    settings.documents.directory = "docs/project-specific".into();
    let session = domain::new_session(
        "합성 내부 문서 검색".into(),
        "사내 문서를 검색하고 근거와 함께 답변하는 도구를 설계합니다.".into(),
        vec![],
    );
    let mut workspace = Workspace::new(settings);
    workspace.runs.push(Run {
        id: id(),
        session_id: session.id.clone(),
        kind: "design".into(),
        status: "failed".into(),
        input_revision: session.design_revision,
        created_at: now(),
        completed_at: Some(now()),
        effective_settings: json!({"timeout_seconds":300,"backend":"claude"}),
        error: Some(RunError {
            code: "timeout".into(),
            message: "Synthetic previous 300-second timeout".into(),
        }),
        process_termination: "confirmed".into(),
        progress: "Synthetic previous failure".into(),
        resolved_model: None,
        cli_version: None,
    });
    workspace.sessions.push(session);
    workspace.settings_overridden = true;
    workspace.settings_revision = 3;
    let snapshot = serde_json::to_value(workspace).unwrap();
    let mut store = Store::open(root).unwrap();
    store.commit_workspace(&snapshot, None, None).unwrap();
    snapshot
}

#[test]
fn a_missing_shared_file_updates_the_default_without_rewriting_past_work() {
    let project = tempfile::tempdir().unwrap();
    let common = tempfile::tempdir().unwrap();
    let path = common.path().join("config.toml");
    let before = legacy_snapshot(project.path());
    let app = App::open_with_shared_config(project.path(), &path).unwrap();
    let after = app.workspace().unwrap();
    assert_eq!(after["settings"]["ai"]["timeout_seconds"], 1800);
    assert_eq!(
        after["settings"]["documents"],
        before["settings"]["documents"]
    );
    assert_eq!(after["sessions"], before["sessions"]);
    assert_eq!(after["runs"], before["runs"]);
    assert_eq!(
        after["runs"][0]["effective_settings"]["timeout_seconds"],
        300
    );
    assert!(
        !path.exists(),
        "reading defaults must not create shared settings"
    );
}

#[test]
fn an_explicit_shared_limit_survives_an_upgrade_with_its_history() {
    let project = tempfile::tempdir().unwrap();
    let common = tempfile::tempdir().unwrap();
    let path = common.path().join("config.toml");
    let before = legacy_snapshot(project.path());
    let mut explicit = Settings::default();
    explicit.ai.timeout_seconds = 300;
    let original = toml::to_string(&explicit).unwrap();
    std::fs::write(&path, &original).unwrap();
    let app = App::open_with_shared_config(project.path(), &path).unwrap();
    let after = app.workspace().unwrap();
    assert_eq!(after["settings"]["ai"]["timeout_seconds"], 300);
    assert_eq!(
        after["settings"]["documents"],
        before["settings"]["documents"]
    );
    assert_eq!(after["sessions"], before["sessions"]);
    assert_eq!(after["runs"], before["runs"]);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), original);
    assert_eq!(config::load(&path).unwrap().ai.timeout_seconds, 300);
}
