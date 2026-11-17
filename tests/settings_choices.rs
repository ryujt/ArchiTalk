use architalk::{
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
use tower::ServiceExt;

#[tokio::test]
async fn settings_provides_known_choices_without_cli_diagnostics() {
    let root = tempfile::tempdir().unwrap();
    let mut settings = Settings::default();
    settings.ai.backends.claude.executable = "/nonexistent/claude".into();
    settings.ai.backends.codex.executable = "/nonexistent/codex".into();
    let app = App::open(root.path(), settings.clone()).unwrap();
    let router = http::router(ServerState {
        app,
        token: "test-capability".into(),
        authority: "127.0.0.1:7788".into(),
    });
    let response = router
        .oneshot(
            Request::builder()
                .uri("/api/v1/settings")
                .header("host", "127.0.0.1:7788")
                .header("authorization", "Bearer test-capability")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(body["settings"], serde_json::to_value(settings).unwrap());
    assert_eq!(body["revision"], 0);
    assert_eq!(body["diagnostics"], json!([]));
    assert_eq!(
        body["choices"],
        json!({
            "claude": {
                "models": ["sonnet", "opus", "fable"],
                "efforts": ["low", "medium", "high", "xhigh", "max"]
            },
            "codex": {
                "models": ["gpt-6.2-sol", "gpt-6.2-astra", "gpt-6.1-terra", "gpt-6.1-sol", "gpt-6-astra", "gpt-6-sol"],
                "efforts": ["minimal", "low", "medium", "high", "xhigh", "max", "ultra"]
            }
        })
    );
}

#[tokio::test]
async fn diagnostic_effort_examples_match_settings_choices_when_cli_is_missing() {
    for backend in [
        architalk::ai::Backend::Claude,
        architalk::ai::Backend::Codex,
    ] {
        let diagnostic = architalk::ai::diagnose(backend, "/nonexistent/architalk-test-cli").await;
        assert!(!diagnostic.available);
        assert_eq!(diagnostic.efforts, backend.choices().efforts);
    }
}
