//! Explicit local CLI smoke; ordinary tests never consume model quota.
#![cfg(target_os = "macos")]
use architalk::{ai, config::Settings, storage::Store};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

#[tokio::test]
#[ignore = "uses authenticated installed Claude; one synthetic request and model quota"]
async fn real_claude_327_uses_a_project_private_runtime() {
    smoke(ai::Backend::Claude, "2.1.327").await;
}

#[tokio::test]
#[ignore = "uses authenticated installed Codex; one synthetic request and model quota"]
async fn real_codex_uses_a_project_private_runtime() {
    smoke(ai::Backend::Codex, "0.169.0").await;
}

async fn smoke(backend: ai::Backend, expected_version: &str) {
    let project = tempfile::tempdir().unwrap();
    let _store = Store::open(project.path()).unwrap();
    let mut settings = Settings::default();
    settings.ai.default_backend = backend;
    settings.ai.backends.claude.effort = Some("low".into());
    settings.ai.backends.codex.effort = Some("low".into());
    settings.ai.timeout_seconds = 90;
    let spec = settings.run_spec(
        "conversation",
        "Do not use tools. Return ONLY the exact JSON object {\"ok\":true}. This is a synthetic CLI compatibility test.".into(),
        project.path().to_owned(),
    );
    let (tx, _rx) = mpsc::channel(64);
    let output = ai::generate(spec, CancellationToken::new(), tx)
        .await
        .unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&output.text).unwrap(),
        serde_json::json!({"ok":true})
    );
    assert!(output.cli_version.contains(expected_version));
    assert_eq!(
        std::fs::read_dir(project.path().join(".architalk/runtime"))
            .unwrap()
            .count(),
        0
    );
    println!(
        "{backend} {expected_version} returned terminal JSON with run-only write sandbox and cleaned project runtime; resolved model={:?}",
        output.resolved_model
    );
}
