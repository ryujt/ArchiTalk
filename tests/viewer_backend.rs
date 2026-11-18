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
use std::{
    fs,
    os::unix::fs::{PermissionsExt, symlink},
};
use tower::ServiceExt;

fn router(app: std::sync::Arc<App>) -> axum::Router {
    http::router(ServerState {
        app,
        token: "test-capability".into(),
        authority: "127.0.0.1:8087".into(),
    })
}
fn request(method: &str, path: &str, body: Body) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(path)
        .header("host", "127.0.0.1:8087")
        .header("origin", "http://127.0.0.1:8087")
        .header("authorization", "Bearer test-capability")
        .header("content-type", "application/json")
        .body(body)
        .unwrap()
}
async fn value(response: axum::response::Response) -> Value {
    serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap()
}

#[tokio::test]
async fn viewer_catalog_prunes_unsafe_files_and_reads_real_unicode_without_selecting_evidence() {
    let project = tempfile::tempdir().unwrap();
    fs::create_dir(project.path().join("docs")).unwrap();
    fs::create_dir(project.path().join("empty")).unwrap();
    fs::create_dir(project.path().join("node_modules")).unwrap();
    use unicode_normalization::UnicodeNormalization;
    let actual_name: String = "docs/설계.md".nfd().collect();
    fs::write(project.path().join(&actual_name), "# 설계").unwrap();
    for (path, bytes) in [
        ("docs/page.html", b"<script>bad()</script>".as_slice()),
        ("empty/picture.png", b"PNG"),
        ("docs/secret-notes.md", b"private"),
        ("docs/.env", b"KEY=value"),
        ("docs/auth.json", b"{\"api_key\":\"synthetic\"}"),
        ("node_modules/a.js", b"dependency"),
        ("docs/binary.txt", b"a\0b"),
        ("docs/bad.txt", b"\xff"),
    ] {
        fs::write(project.path().join(path), bytes).unwrap();
    }
    symlink("/etc/passwd", project.path().join("docs/link.txt")).unwrap();
    let external = tempfile::NamedTempFile::new().unwrap();
    fs::write(external.path(), "outside project sentinel").unwrap();
    fs::hard_link(external.path(), project.path().join("docs/external.txt")).unwrap();
    let app = App::open(project.path(), Settings::default()).unwrap();
    let api = router(app.clone());
    let response = api
        .clone()
        .oneshot(request("GET", "/api/v1/files/tree", Body::empty()))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let catalog = value(response).await;
    let files = catalog["files"].as_array().unwrap();
    assert_eq!(files.len(), 2, "{catalog}");
    assert_eq!(catalog["entries"].as_array().unwrap().len(), 1);
    let markdown = files.iter().find(|f| f["kind"] == "markdown").unwrap();
    let path = markdown["path"].as_str().unwrap();
    let encoded: String = path.bytes().map(|b| format!("%{b:02X}")).collect();
    let response = api
        .clone()
        .oneshot(request(
            "GET",
            &format!("/api/v1/files/read?path={encoded}"),
            Body::empty(),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let file = value(response).await;
    assert_eq!(file["path"], path);
    // Canonically equivalent Unicode input must still return the actual
    // directory entry spelling rather than echoing a caller's alias.
    let nfc: String = path.nfc().collect();
    assert_eq!(app.viewer_read(nfc).await.unwrap()["path"], path);
    assert_eq!(file["content"], "# 설계");
    assert_eq!(
        file["hash"],
        architalk::files::hash_bytes("# 설계".as_bytes())
    );
    assert_eq!(
        fs::read_to_string(file["absolute_path"].as_str().unwrap()).unwrap(),
        "# 설계"
    );
    assert_eq!(app.workspace().unwrap()["sessions"], json!([]));
    for path in [
        "../Cargo.toml",
        "docs/../Cargo.toml",
        "docs/link.txt",
        "docs/external.txt",
        "docs/.env",
        "docs/secret-notes.md",
        "docs/binary.txt",
        "docs/bad.txt",
    ] {
        let encoded: String = path.bytes().map(|b| format!("%{b:02X}")).collect();
        let response = api
            .clone()
            .oneshot(request(
                "GET",
                &format!("/api/v1/files/read?path={encoded}"),
                Body::empty(),
            ))
            .await
            .unwrap();
        assert!(!response.status().is_success(), "{path}");
    }
    fs::remove_dir_all(project.path().join(".architalk")).unwrap();
    let response = api
        .oneshot(request("GET", "/api/v1/files/tree", Body::empty()))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::GONE);
    assert!(!project.path().join(".architalk").exists());
}

#[tokio::test]
async fn transcription_secrets_are_write_only_shared_private_and_explicitly_removable() {
    let project = tempfile::tempdir().unwrap();
    let config = tempfile::tempdir().unwrap();
    let app =
        App::open_with_shared_config(project.path(), &config.path().join("config.toml")).unwrap();
    let api = router(app.clone());
    let secret = "sk-synthetic-private-never-an-api-key";
    let response = api
        .clone()
        .oneshot(request(
            "PATCH",
            "/api/v1/transcription/key",
            Body::from(json!({"api_key":secret,"model":"gpt-transcribe"}).to_string()),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let settings = value(response).await;
    assert_eq!(settings["configured"], true);
    assert_eq!(settings["key_source"], "file");
    assert!(!settings.to_string().contains(secret));
    let secret_file = config.path().join("transcription-secrets.toml");
    assert_eq!(
        fs::metadata(&secret_file).unwrap().permissions().mode() & 0o777,
        0o600
    );
    assert!(!app.workspace().unwrap().to_string().contains(secret));
    for name in ["workspace.db", "workspace.db-wal"] {
        if let Ok(bytes) = fs::read(project.path().join(".architalk").join(name)) {
            assert!(!bytes.windows(secret.len()).any(|s| s == secret.as_bytes()));
        }
    }
    let response = api
        .clone()
        .oneshot(request(
            "PATCH",
            "/api/v1/transcription/key",
            Body::from("{\"model\":\"gpt-transcribe\"}"),
        ))
        .await
        .unwrap();
    assert_eq!(value(response).await["configured"], true);
    let response = api
        .clone()
        .oneshot(request(
            "PATCH",
            "/api/v1/transcription/key",
            Body::from("{\"api_key\":null}"),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert!(!fs::read_to_string(secret_file).unwrap().contains(secret));
    let isolated_project = tempfile::tempdir().unwrap();
    let isolated = router(App::open(isolated_project.path(), Settings::default()).unwrap());
    let response = isolated
        .oneshot(request(
            "GET",
            "/api/v1/transcription/settings",
            Body::empty(),
        ))
        .await
        .unwrap();
    assert_eq!(value(response).await["configured"], false);
}

#[tokio::test]
async fn missing_transcription_key_is_reported_before_the_audio_body_is_polled() {
    let project = tempfile::tempdir().unwrap();
    let api = router(App::open(project.path(), Settings::default()).unwrap());
    let polled = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let observed = polled.clone();
    let unread = Body::from_stream(async_stream::stream! {
        observed.store(true, std::sync::atomic::Ordering::SeqCst);
        yield Ok::<_, std::io::Error>(axum::body::Bytes::new());
    });
    let response = api
        .oneshot(request("POST", "/api/v1/transcription", unread))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        value(response).await["error"]["code"],
        "transcription_key_missing"
    );
    assert!(!polled.load(std::sync::atomic::Ordering::SeqCst));
}

#[tokio::test]
async fn viewer_limits_and_read_time_symlink_replacement_fail_closed() {
    let project = tempfile::tempdir().unwrap();
    let docs = project.path().join("docs");
    fs::create_dir(&docs).unwrap();
    for index in 0..2002 {
        fs::write(docs.join(format!("{index:04}.md")), "ok").unwrap();
    }
    fs::write(
        project.path().join("large.txt"),
        vec![b'x'; 2 * 1024 * 1024 + 1],
    )
    .unwrap();
    let app = App::open(project.path(), Settings::default()).unwrap();
    let tree = app.viewer_tree().await.unwrap();
    assert_eq!(tree["files"].as_array().unwrap().len(), 2000);
    assert_eq!(tree["truncated"], true);
    assert!(app.viewer_read("large.txt".into()).await.is_err());
    fs::rename(&docs, project.path().join("original-docs")).unwrap();
    symlink("/etc", &docs).unwrap();
    assert!(app.viewer_read("docs/passwd.txt".into()).await.is_err());
    for path in [
        "/etc/passwd",
        "docs//a.md",
        "docs\\a.md",
        "docs/./a.md",
        "docs/%2e%2e/a.md",
    ] {
        assert!(app.viewer_read(path.into()).await.is_err(), "{path}");
    }
}

#[tokio::test]
async fn secret_links_permissions_and_invalid_key_updates_never_leak_or_overwrite() {
    let project = tempfile::tempdir().unwrap();
    let config = tempfile::tempdir().unwrap();
    let secret_file = config.path().join("transcription-secrets.toml");
    let outside = tempfile::NamedTempFile::new().unwrap();
    fs::write(outside.path(), "outside sentinel").unwrap();
    symlink(outside.path(), &secret_file).unwrap();
    let app =
        App::open_with_shared_config(project.path(), &config.path().join("config.toml")).unwrap();
    assert!(app.transcription_settings().is_err());
    assert!(
        app.update_transcription(&json!({"api_key":"synthetic"}))
            .is_err()
    );
    assert_eq!(
        fs::read_to_string(outside.path()).unwrap(),
        "outside sentinel"
    );
    fs::remove_file(&secret_file).unwrap();
    fs::hard_link(outside.path(), &secret_file).unwrap();
    assert!(app.transcription_settings().is_err());
    fs::remove_file(&secret_file).unwrap();
    fs::write(
        &secret_file,
        "model='gpt-transcribe'\napi_key='synthetic'\n",
    )
    .unwrap();
    fs::set_permissions(&secret_file, fs::Permissions::from_mode(0o644)).unwrap();
    assert!(app.transcription_settings().is_err());
    fs::set_permissions(&secret_file, fs::Permissions::from_mode(0o600)).unwrap();
    for value in [
        json!({"model":"gpt\nsecret"}),
        json!({"model":"../model"}),
        json!({"api_key":"bad\r\nheader"}),
        json!({"api_key":14}),
        json!({"events":[]}),
    ] {
        let error = app.update_transcription(&value).err().unwrap();
        assert!(!error.message.contains("synthetic"));
    }
    assert!(app.transcription_settings().unwrap().configured);
}

async fn pending_audio(
    api: axum::Router,
) -> (
    tokio::task::JoinHandle<axum::response::Response>,
    std::sync::Arc<tokio::sync::Notify>,
) {
    let observed = std::sync::Arc::new(tokio::sync::Notify::new());
    let notify = observed.clone();
    let stream = async_stream::stream! {
        notify.notify_one();
        yield Ok::<_, std::io::Error>(axum::body::Bytes::from_static(b"--boundary\r\nContent-Disposition: form-data; name=\"audio\"; filename=\"x.webm\"\r\nContent-Type: audio/webm\r\n\r\n"));
        std::future::pending::<()>().await;
    };
    let mut req = request("POST", "/api/v1/transcription", Body::from_stream(stream));
    req.headers_mut().insert(
        "content-type",
        "multipart/form-data; boundary=boundary".parse().unwrap(),
    );
    let task = tokio::spawn(async move { api.oneshot(req).await.unwrap() });
    (task, observed)
}

#[tokio::test]
async fn reset_and_shutdown_cancel_pending_audio_and_concurrency_has_one_slot() {
    for reset in [true, false] {
        let project = tempfile::tempdir().unwrap();
        let config = tempfile::tempdir().unwrap();
        let app = App::open_with_shared_config(project.path(), &config.path().join("config.toml"))
            .unwrap();
        app.update_transcription(&json!({"api_key":"synthetic-private-test-key"}))
            .unwrap();
        let api = router(app.clone());
        let (pending, observed) = pending_audio(api.clone()).await;
        tokio::time::timeout(std::time::Duration::from_secs(2), observed.notified())
            .await
            .unwrap();
        let busy = api
            .clone()
            .oneshot(request("POST", "/api/v1/transcription", Body::empty()))
            .await
            .unwrap();
        assert_eq!(busy.status(), StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(value(busy).await["error"]["code"], "transcription_busy");
        if reset {
            fs::remove_dir_all(project.path().join(".architalk")).unwrap();
        } else {
            app.shutdown.cancel();
        }
        let response = tokio::time::timeout(std::time::Duration::from_secs(2), pending)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            response.status(),
            if reset {
                StatusCode::GONE
            } else {
                StatusCode::SERVICE_UNAVAILABLE
            }
        );
        assert_eq!(
            value(response).await["error"]["code"],
            if reset {
                "workspace_reset_required"
            } else {
                "shutting_down"
            }
        );
        if reset {
            assert!(!project.path().join(".architalk").exists());
        }
    }
}
