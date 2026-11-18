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
use tower::ServiceExt;

#[tokio::test]
async fn the_browser_icon_is_a_local_public_image_with_normal_origin_protection() {
    let project = tempfile::tempdir().unwrap();
    let app = http::router(ServerState {
        app: App::open(project.path(), Settings::default()).unwrap(),
        token: "private-api-capability".into(),
        authority: "127.0.0.1:8087".into(),
    });
    let icon = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/favicon.ico")
                .header("host", "127.0.0.1:8087")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(icon.status(), StatusCode::OK);
    assert_eq!(icon.headers()["content-type"], "image/x-icon");
    assert_eq!(icon.headers()["x-content-type-options"], "nosniff");
    let bytes = icon.into_body().collect().await.unwrap().to_bytes();
    assert!(bytes.len() > 22);
    assert_eq!(&bytes[..4], &[0, 0, 1, 0]);
    assert!(u16::from_le_bytes([bytes[4], bytes[5]]) > 0);
    let html = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/")
                .header("host", "127.0.0.1:8087")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = html.into_body().collect().await.unwrap().to_bytes();
    assert!(
        std::str::from_utf8(&body)
            .unwrap()
            .contains("rel=\"icon\" type=\"image/x-icon\" href=\"/favicon.ico\"")
    );
    let foreign = app
        .oneshot(
            Request::builder()
                .uri("/favicon.ico")
                .header("host", "foreign.invalid")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(foreign.status(), StatusCode::FORBIDDEN);
}
