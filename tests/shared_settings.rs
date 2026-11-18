use architalk::{
    app::App,
    config::{self, Settings},
    model::{Command, id},
};
use serde_json::{Value, json};
use std::fs;

fn command(revision: u64, settings: Value) -> Command {
    Command {
        command_id: id(),
        expected_revision: revision,
        payload: json!({"settings": settings}),
    }
}

#[test]
fn common_ai_preferences_cross_projects_but_document_paths_and_history_do_not() {
    let temporary = tempfile::tempdir().unwrap();
    let a = temporary.path().join("project-one");
    let b = temporary.path().join("project-two");
    fs::create_dir(&a).unwrap();
    fs::create_dir(&b).unwrap();
    let common = temporary.path().join("common/config.toml");
    let first = App::open_with_shared_config(&a, &common).unwrap();
    let second = App::open_with_shared_config(&b, &common).unwrap();
    let before = first.settings().unwrap();
    let mut changed = before["settings"].clone();
    changed["ai"]["backends"]["claude"]["model"] = json!("shared-model");
    changed["ai"]["roles"]["design"]["effort"] = json!("high");
    changed["documents"]["directory"] = json!("docs/private-project-one");
    first
        .command(
            "settings",
            None,
            command(before["revision"].as_u64().unwrap(), changed),
        )
        .unwrap();
    let seen = second.settings().unwrap();
    assert_eq!(
        seen["settings"]["ai"]["backends"]["claude"]["model"],
        "shared-model"
    );
    assert_eq!(seen["settings"]["ai"]["roles"]["design"]["effort"], "high");
    assert_eq!(
        seen["settings"]["documents"]["directory"],
        "docs/architecture"
    );
    let mut changed_b = seen["settings"].clone();
    changed_b["documents"]["directory"] = json!("docs/private-project-two");
    second
        .command(
            "settings",
            None,
            command(seen["revision"].as_u64().unwrap(), changed_b),
        )
        .unwrap();
    let bytes = fs::read_to_string(&common).unwrap();
    assert!(
        !bytes.contains("private-project")
            && !bytes.contains("project-one")
            && !bytes.contains("project-two")
    );
    assert_eq!(
        config::load(&common).unwrap().documents,
        Settings::default().documents
    );
    assert_eq!(first.workspace().unwrap()["sessions"], json!([]));
    assert_eq!(second.workspace().unwrap()["sessions"], json!([]));
    drop(first);
    drop(second);
    let reopened = App::open_with_shared_config(&a, &common).unwrap();
    let settings = reopened.settings().unwrap();
    assert_eq!(
        settings["settings"]["ai"]["backends"]["claude"]["model"],
        "shared-model"
    );
    assert_eq!(
        settings["settings"]["documents"]["directory"],
        "docs/private-project-one"
    );
}

#[test]
fn stale_shared_preferences_cannot_overwrite_another_project_and_retry_is_idempotent() {
    let temporary = tempfile::tempdir().unwrap();
    let common = temporary.path().join("config.toml");
    let defaults = Settings::default();
    let old_hash = config::ai_hash(&defaults.ai).unwrap();
    let mut first = defaults.ai.clone();
    first.backends.claude.model = Some("first".into());
    let new_hash = config::save_ai(&common, &first, &old_hash).unwrap();
    let bytes = fs::read(&common).unwrap();
    assert_eq!(
        config::save_ai(&common, &first, &old_hash).unwrap(),
        new_hash
    );
    assert_eq!(fs::read(&common).unwrap(), bytes);
    let mut stale = defaults.ai;
    stale.backends.claude.model = Some("stale".into());
    assert!(
        config::save_ai(&common, &stale, &old_hash)
            .unwrap_err()
            .to_string()
            .contains("shared_settings_conflict")
    );
    assert_eq!(fs::read(&common).unwrap(), bytes);
}

#[test]
fn another_project_change_invalidates_an_open_settings_form() {
    let temporary = tempfile::tempdir().unwrap();
    let a = temporary.path().join("a");
    let b = temporary.path().join("b");
    fs::create_dir(&a).unwrap();
    fs::create_dir(&b).unwrap();
    let common = temporary.path().join("shared.toml");
    let first = App::open_with_shared_config(&a, &common).unwrap();
    let second = App::open_with_shared_config(&b, &common).unwrap();
    let stale = second.settings().unwrap();
    let before = first.settings().unwrap();
    let mut changed = before["settings"].clone();
    changed["ai"]["timeout_seconds"] = json!(480);
    first
        .command(
            "settings",
            None,
            command(before["revision"].as_u64().unwrap(), changed),
        )
        .unwrap();
    assert!(
        second
            .command(
                "settings",
                None,
                command(
                    stale["revision"].as_u64().unwrap(),
                    stale["settings"].clone()
                )
            )
            .is_err()
    );
    assert_eq!(config::load(&common).unwrap().ai.timeout_seconds, 480);
}

#[test]
fn a_shared_change_while_stopped_invalidates_the_previous_form_after_restart() {
    let temporary = tempfile::tempdir().unwrap();
    let project = temporary.path().join("project");
    fs::create_dir(&project).unwrap();
    let common = temporary.path().join("settings.toml");
    let app = App::open_with_shared_config(&project, &common).unwrap();
    let old_form = app.settings().unwrap();
    drop(app);
    let mut latest = Settings::default().ai;
    latest.backends.codex.model = Some("new-model-while-stopped".into());
    config::save_ai(
        &common,
        &latest,
        &config::ai_hash(&Settings::default().ai).unwrap(),
    )
    .unwrap();
    let restarted = App::open_with_shared_config(&project, &common).unwrap();
    assert!(
        restarted
            .command(
                "settings",
                None,
                command(
                    old_form["revision"].as_u64().unwrap(),
                    old_form["settings"].clone()
                )
            )
            .is_err()
    );
    assert_eq!(
        config::load(&common)
            .unwrap()
            .ai
            .backends
            .codex
            .model
            .as_deref(),
        Some("new-model-while-stopped")
    );
}

#[tokio::test]
async fn next_run_reads_a_shared_change_without_reopening_settings_or_the_project() {
    let temporary = tempfile::tempdir().unwrap();
    let project = temporary.path().join("private-project");
    fs::create_dir(&project).unwrap();
    let common = temporary.path().join("common.toml");
    let mut defaults = Settings::default();
    defaults.ai.backends.claude.executable = "missing-architalk-shared-settings-probe".into();
    fs::write(&common, toml::to_string(&defaults).unwrap()).unwrap();
    let app = App::open_with_shared_config(&project, &common).unwrap();
    let mut latest = defaults.ai.clone();
    latest.backends.claude.model = Some("new-shared-model".into());
    latest.timeout_seconds = 480;
    config::save_ai(&common, &latest, &config::ai_hash(&defaults.ai).unwrap()).unwrap();
    let created = app.command("create", None, Command {
        command_id: id(), expected_revision: 0,
        payload: json!({"title":"private-project", "intro":"private project introduction", "context_paths":[]}),
    }).unwrap();
    assert_eq!(
        created["run"]["effective_settings"]["model"],
        "new-shared-model"
    );
    assert_eq!(created["run"]["effective_settings"]["timeout_seconds"], 480);
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        while !app.is_idle() {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(
        app.workspace().unwrap()["runs"][0]["error"]["code"],
        "cli_not_found"
    );
    assert!(
        !fs::read_to_string(common)
            .unwrap()
            .contains("private project introduction")
    );
}
