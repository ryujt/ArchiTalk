use architalk::{
    app::App,
    config::Settings,
    design_package::{DesignPackage, DocumentKind, MAIN_FILENAME, package_hash},
    domain, files,
    model::{Command, Draft, ExportEntry, Workspace, id, now},
    storage::Store,
};
use serde_json::{Value, json};
use std::{fs, path::Path, sync::Arc};

fn command(revision: u64, payload: Value) -> Command {
    Command {
        command_id: id(),
        expected_revision: revision,
        payload,
    }
}

fn package() -> DesignPackage {
    domain::validate_design_package(include_str!("fixtures/design-package.json")).unwrap()
}

fn seed_package(root: &Path) -> (Arc<App>, String, String) {
    let mut session = domain::new_session(
        "자료 조회".into(),
        "자료 목록과 상세를 읽습니다.".into(),
        vec![],
    );
    let package = package();
    let markdown = format!("{}{}", package.markdown, domain::trace_appendix(&session));
    let draft = Draft {
        id: id(),
        input_revision: session.design_revision,
        hash: package_hash(&markdown, &package.documents),
        markdown,
        documents: package.documents,
        created_at: now(),
        content_status: "draft".into(),
        reviewed: true,
    };
    let sid = session.id.clone();
    let did = draft.id.clone();
    session.drafts.push(draft);
    let mut workspace = Workspace::new(Settings::default());
    workspace.sessions.push(session);
    let mut store = Store::open(root).unwrap();
    store
        .commit_workspace(&serde_json::to_value(workspace).unwrap(), None, None)
        .unwrap();
    drop(store);
    (App::open(root, Settings::default()).unwrap(), sid, did)
}

fn preview(app: &Arc<App>, sid: &str, did: &str) -> Value {
    let revision = app.session(sid).unwrap()["revision"].as_u64().unwrap();
    app.command(
        "export",
        Some(sid),
        command(revision, json!({"mode":"preview","draft_id":did})),
    )
    .unwrap()
}

#[test]
fn app_preview_and_publication_cover_all_ten_artifacts_and_server_catalog() {
    let root = tempfile::tempdir().unwrap();
    let (app, sid, did) = seed_package(root.path());
    let catalog = app.workspace().unwrap()["document_types"]
        .as_array()
        .unwrap()
        .clone();
    assert_eq!(catalog.len(), 10);
    assert_eq!(
        catalog[0],
        json!({"kind":"architecture","filename":MAIN_FILENAME,"title":"아키텍처 설계"})
    );
    for (entry, kind) in catalog[1..].iter().zip(DocumentKind::ALL) {
        assert_eq!(entry["kind"], serde_json::to_value(kind).unwrap());
        assert_eq!(entry["filename"], kind.filename());
        assert_eq!(entry["title"], kind.title());
    }
    let before = app.session(&sid).unwrap();
    let preview = preview(&app, &sid, &did);
    let plan: files::ExportPlan = serde_json::from_value(preview["plan"].clone()).unwrap();
    assert_eq!(plan.files.len(), 10);
    assert_eq!(
        preview["export"]["source_document_hash"],
        before["drafts"][0]["hash"]
    );
    assert_eq!(plan.document_hash, plan.files[0].hash);
    for (file, entry) in plan.files.iter().zip(catalog) {
        assert_eq!(file.relative_path, entry["filename"].as_str().unwrap());
        assert!(file.content.starts_with("---\n"));
        assert!(file.content.contains("내용 상태: draft"));
        assert!(file.content.contains("독립 검토: 완료"));
        assert_eq!(file.hash, files::hash_bytes(file.content.as_bytes()));
    }
    let revision = app.session(&sid).unwrap()["revision"].as_u64().unwrap();
    let saved = app
        .command(
            "export",
            Some(&sid),
            command(revision, json!({"mode":"apply","plan_id":plan.id})),
        )
        .unwrap();
    assert_eq!(saved["export"]["status"], "completed");
    assert_eq!(
        saved["export"]["result"]["paths"].as_array().unwrap().len(),
        10
    );
    for file in &plan.files {
        assert_eq!(
            fs::read_to_string(
                root.path()
                    .join(&plan.relative_directory)
                    .join(&file.relative_path)
            )
            .unwrap(),
            file.content
        );
    }
    assert_eq!(
        app.session(&sid).unwrap()["drafts"],
        before["drafts"],
        "export metadata never rewrites the immutable package"
    );
    drop(app);
    let reopened = App::open(root.path(), Settings::default()).unwrap();
    assert_eq!(reopened.session(&sid).unwrap()["drafts"], before["drafts"]);
}

#[test]
fn a_changed_detail_invalidates_its_persisted_preview_even_with_the_same_main_and_revision() {
    let root = tempfile::tempdir().unwrap();
    let (app, sid, did) = seed_package(root.path());
    let preview = preview(&app, &sid, &did);
    let revision = app.session(&sid).unwrap()["revision"].as_u64().unwrap();
    drop(app);
    // Simulate a persisted source replacement through the public Store API.
    // Holding the session revision and main bytes fixed isolates the hash guard.
    let mut store = Store::open(root.path()).unwrap();
    let mut workspace: Workspace =
        serde_json::from_value(store.load_workspace().unwrap().unwrap()).unwrap();
    let draft = &mut workspace.sessions[0].drafts[0];
    let main = draft.markdown.clone();
    let old_hash = draft.hash.clone();
    draft.documents[4]
        .markdown
        .push_str("\n추가 확인 조건은 unknown으로 남긴다.\n");
    draft.hash = package_hash(&draft.markdown, &draft.documents);
    assert_eq!(draft.markdown, main);
    assert_ne!(draft.hash, old_hash);
    assert_eq!(workspace.sessions[0].revision, revision);
    store
        .commit_workspace(&serde_json::to_value(workspace).unwrap(), None, None)
        .unwrap();
    drop(store);
    let app = App::open(root.path(), Settings::default()).unwrap();
    let error = app
        .command(
            "export",
            Some(&sid),
            command(
                revision,
                json!({"mode":"apply","plan_id":preview["export"]["id"]}),
            ),
        )
        .unwrap_err();
    assert_eq!(error.code, "preview_stale");
    assert!(!root.path().join("docs").exists());
}

#[test]
fn restart_reconciles_a_persisted_partially_published_ten_file_plan() {
    let root = tempfile::tempdir().unwrap();
    let (app, sid, did) = seed_package(root.path());
    let preview = preview(&app, &sid, &did);
    drop(app);
    let mut store = Store::open(root.path()).unwrap();
    let mut workspace: Workspace =
        serde_json::from_value(store.load_workspace().unwrap().unwrap()).unwrap();
    let plan: files::ExportPlan = serde_json::from_value(preview["plan"].clone()).unwrap();
    let plan = store.project_files().unwrap().reserve(&plan).unwrap();
    workspace.exports[0].status = "applying".into();
    workspace.exports[0].plan = serde_json::to_value(&plan).unwrap();
    store
        .commit_workspace(&serde_json::to_value(workspace).unwrap(), None, None)
        .unwrap();
    drop(store);
    let directory = root.path().join(&plan.relative_directory);
    for index in [0, 2, 7] {
        fs::write(
            directory.join(&plan.files[index].relative_path),
            &plan.files[index].content,
        )
        .unwrap();
    }
    assert!(!directory.join(".architalk-export.json").exists());
    let app = App::open(root.path(), Settings::default()).unwrap();
    let result = app.export(&sid, &plan.id).unwrap();
    assert_eq!(result["status"], "completed");
    assert_eq!(result["result"]["paths"].as_array().unwrap().len(), 10);
    assert!(directory.join(".architalk-export.json").exists());
    for file in &plan.files {
        assert_eq!(
            fs::read_to_string(directory.join(&file.relative_path)).unwrap(),
            file.content
        );
    }
}

#[test]
fn v1_single_document_snapshot_without_the_new_field_still_recovers_its_export() {
    let root = tempfile::tempdir().unwrap();
    let mut session = domain::new_session(
        "과거 설계".into(),
        "기존 단일 문서를 보존합니다.".into(),
        vec![],
    );
    let markdown = "# Legacy\nKeep original bytes.\n";
    let draft = Draft {
        id: id(),
        input_revision: session.design_revision,
        markdown: markdown.into(),
        documents: vec![],
        hash: package_hash(markdown, &[]),
        created_at: now(),
        content_status: "draft".into(),
        reviewed: false,
    };
    let did = draft.id.clone();
    let sid = session.id.clone();
    session.drafts.push(draft);
    let mut workspace = Workspace::new(Settings::default());
    workspace.sessions.push(session);
    let mut store = Store::open(root.path()).unwrap();
    let plan = store
        .project_files()
        .unwrap()
        .preview("과거 설계", markdown, 0, &sid)
        .unwrap();
    let plan = store.project_files().unwrap().reserve(&plan).unwrap();
    workspace.exports.push(ExportEntry {
        id: plan.id.clone(),
        session_id: sid.clone(),
        document_revision: 0,
        document_hash: plan.document_hash.clone(),
        draft_id: did,
        preview_revision: 0,
        source_document_hash: package_hash(markdown, &[]),
        status: "applying".into(),
        plan: serde_json::to_value(&plan).unwrap(),
        result: None,
        created_at: now(),
    });
    let mut snapshot = serde_json::to_value(workspace).unwrap();
    snapshot["sessions"][0]["drafts"][0]
        .as_object_mut()
        .unwrap()
        .remove("documents");
    store.commit_workspace(&snapshot, None, None).unwrap();
    drop(store);
    {
        let database =
            rusqlite::Connection::open(root.path().join(".architalk/workspace.db")).unwrap();
        database.pragma_update(None, "user_version", 1).unwrap();
    }
    let app = App::open(root.path(), Settings::default()).unwrap();
    let draft = &app.session(&sid).unwrap()["drafts"][0];
    assert_eq!(draft["markdown"], markdown);
    assert_eq!(draft["hash"], package_hash(markdown, &[]));
    assert_eq!(draft["documents"], json!([]));
    let entry = app.export(&sid, &plan.id).unwrap();
    assert_eq!(entry["status"], "completed");
    assert_eq!(entry["result"]["paths"].as_array().unwrap().len(), 1);
    assert_eq!(
        fs::read_to_string(
            root.path()
                .join(&plan.relative_directory)
                .join(MAIN_FILENAME)
        )
        .unwrap(),
        markdown
    );
}

#[test]
#[ignore = "requires a previously built schema-v1 binary in ARCHITALK_LEGACY_BINARY"]
fn actual_schema1_binary_refuses_a_package_database_without_rewriting_its_bytes() {
    use std::{
        process::{Command as ProcessCommand, Stdio},
        time::{Duration, Instant},
    };
    let binary = std::env::var_os("ARCHITALK_LEGACY_BINARY")
        .expect("set ARCHITALK_LEGACY_BINARY to a schema-v1 binary");
    let root = tempfile::tempdir().unwrap();
    let (app, _, _) = seed_package(root.path());
    drop(app);
    let database = root.path().join(".architalk/workspace.db");
    let original = fs::read(&database).unwrap();
    let config = root.path().join("empty-config.toml");
    fs::write(&config, "").unwrap();
    let mut child = ProcessCommand::new(binary)
        .arg("--project")
        .arg(root.path())
        .arg("--config")
        .arg(config)
        .arg("--no-open")
        .arg("--port")
        .arg("0")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if child.try_wait().unwrap().is_some() {
            break;
        }
        if Instant::now() > deadline {
            child.kill().unwrap();
            let _ = child.wait();
            panic!("schema-v1 binary did not reject the package database before starting");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let output = child.wait_with_output().unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("unsupported_schema"));
    assert_eq!(fs::read(database).unwrap(), original);
}
