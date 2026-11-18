use architalk::files::{self, ExportFile, ExportStatus};
use std::{
    fs,
    os::unix::fs::{PermissionsExt, symlink},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
fn plan(root: &std::path::Path) -> files::ExportPlan {
    files::preview(root, "회원 알림 서비스", "# 설계\n", 7, "session").unwrap()
}
fn package_plan(root: &std::path::Path) -> files::ExportPlan {
    let package: architalk::design_package::DesignPackage =
        serde_json::from_str(include_str!("fixtures/design-package.json")).unwrap();
    files::preview_documents_in(
        root,
        "docs/architecture",
        "패키지",
        &package.markdown,
        &package.documents,
        7,
        "session",
    )
    .unwrap()
}

#[test]
fn ten_file_package_recovers_partial_publication_and_retains_manifest_v1() {
    let root = tempfile::tempdir().unwrap();
    let plan = package_plan(root.path());
    assert_eq!(plan.files.len(), 10);
    assert_eq!(plan.document_hash, plan.files[0].hash);
    assert_eq!(
        plan.files[0].relative_path,
        architalk::design_package::MAIN_FILENAME
    );
    let reserved = files::reserve(root.path(), &plan).unwrap();
    let persisted = serde_json::from_value(serde_json::to_value(reserved).unwrap()).unwrap();
    let directory = root.path().join(&plan.relative_directory);
    for index in [0, 3, 8] {
        let file = &plan.files[index];
        fs::write(directory.join(&file.relative_path), &file.content).unwrap();
    }
    assert!(!directory.join(".architalk-export.json").exists());
    let result = files::reconcile(root.path(), &persisted).unwrap();
    assert_eq!(result.status, ExportStatus::Completed);
    assert_eq!(result.paths.len(), 10);
    assert!(result.files.iter().all(|file| file.status == "applied"));
    let manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(directory.join(".architalk-export.json")).unwrap())
            .unwrap();
    assert_eq!(manifest["version"], 1);
    assert_eq!(manifest["document_hash"], plan.files[0].hash);
    assert_eq!(manifest["files"].as_array().unwrap().len(), 10);
    for file in &plan.files {
        assert_eq!(
            files::hash_bytes(&fs::read(directory.join(&file.relative_path)).unwrap()),
            file.hash
        );
    }
    assert_eq!(
        files::apply(root.path(), &persisted).unwrap().status,
        ExportStatus::Completed
    );
}

#[test]
fn conflict_in_last_package_member_prevents_publication_of_all_missing_members() {
    let root = tempfile::tempdir().unwrap();
    let plan = files::reserve(root.path(), &package_plan(root.path())).unwrap();
    let directory = root.path().join(&plan.relative_directory);
    let conflict = directory.join(&plan.files[9].relative_path);
    fs::write(&conflict, "외부 편집 내용").unwrap();
    let result = files::apply(root.path(), &plan).unwrap();
    assert_eq!(result.status, ExportStatus::Conflict);
    assert!(result.paths.is_empty());
    assert_eq!(fs::read_to_string(conflict).unwrap(), "외부 편집 내용");
    assert!(!directory.join(".architalk-export.json").exists());
    for file in &plan.files[..9] {
        assert!(!directory.join(&file.relative_path).exists());
    }
}
#[test]
fn partial_export_recovers_hashes_and_publishes_manifest_last() {
    let root = tempfile::tempdir().unwrap();
    let mut p = plan(root.path());
    p.files.push(ExportFile {
        relative_path: "요구사항.md".into(),
        content: "requirements".into(),
        hash: files::hash_bytes(b"requirements"),
    });
    let reserved = files::reserve(root.path(), &p).unwrap();
    let persisted: files::ExportPlan =
        serde_json::from_value(serde_json::to_value(reserved).unwrap()).unwrap();
    let dir = root.path().join(&p.relative_directory);
    fs::write(dir.join(&p.files[0].relative_path), &p.files[0].content).unwrap();
    assert!(!dir.join(".architalk-export.json").exists());
    let r = files::reconcile(root.path(), &persisted).unwrap();
    assert_eq!(r.status, ExportStatus::Completed);
    assert_eq!(r.paths.len(), 2);
    assert!(r.files.iter().all(|f| f.status == "applied"));
    let manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(dir.join(".architalk-export.json")).unwrap()).unwrap();
    assert_eq!(manifest["document_revision"], 7);
    assert_eq!(manifest["files"].as_array().unwrap().len(), 2);
}
#[test]
fn unrecorded_identity_or_replaced_directory_is_never_adopted() {
    let root = tempfile::tempdir().unwrap();
    let p = plan(root.path());
    let reserved = files::reserve(root.path(), &p).unwrap();
    assert_eq!(
        files::apply(root.path(), &p).unwrap().status,
        ExportStatus::Conflict
    );
    let dir = root.path().join(&p.relative_directory);
    fs::rename(&dir, root.path().join("old-bundle")).unwrap();
    fs::create_dir(&dir).unwrap();
    assert_eq!(
        files::reconcile(root.path(), &reserved).unwrap().status,
        ExportStatus::Conflict
    );
    assert!(fs::read_dir(dir).unwrap().next().is_none());
}
#[test]
fn immutable_revisions_preserve_external_edits_and_use_natural_suffix() {
    let root = tempfile::tempdir().unwrap();
    let p = files::reserve(root.path(), &plan(root.path())).unwrap();
    assert_eq!(
        files::apply(root.path(), &p).unwrap().status,
        ExportStatus::Completed
    );
    let original = root
        .path()
        .join(&p.relative_directory)
        .join(&p.files[0].relative_path);
    fs::write(&original, "external editor").unwrap();
    assert_eq!(
        files::reconcile(root.path(), &p).unwrap().status,
        ExportStatus::Conflict
    );
    let next = plan(root.path());
    assert!(next.relative_directory.ends_with("회원-알림-서비스-개정-2"));
    let next = files::reserve(root.path(), &next).unwrap();
    assert_eq!(
        files::apply(root.path(), &next).unwrap().status,
        ExportStatus::Completed
    );
    assert_eq!(fs::read_to_string(original).unwrap(), "external editor");
}
#[test]
fn symlink_leaf_ancestor_and_manifest_tamper_preserve_targets() {
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let marker = outside.path().join("marker");
    fs::write(&marker, "preserved").unwrap();
    let p = files::reserve(root.path(), &plan(root.path())).unwrap();
    let dir = root.path().join(&p.relative_directory);
    let document = dir.join(&p.files[0].relative_path);
    symlink(&marker, &document).unwrap();
    assert_eq!(
        files::apply(root.path(), &p).unwrap().status,
        ExportStatus::Conflict
    );
    assert_eq!(fs::read_to_string(&marker).unwrap(), "preserved");
    fs::remove_file(&document).unwrap();
    fs::write(dir.join(".architalk-export.json"), "tampered").unwrap();
    assert_eq!(
        files::apply(root.path(), &p).unwrap().status,
        ExportStatus::Conflict
    );
    assert!(!document.exists());
    fs::remove_file(dir.join(".architalk-export.json")).unwrap();
    fs::rename(root.path().join("docs"), root.path().join("old-docs")).unwrap();
    symlink(outside.path(), root.path().join("docs")).unwrap();
    assert_eq!(
        files::apply(root.path(), &p).unwrap().status,
        ExportStatus::Conflict
    );
    assert_eq!(fs::read_dir(outside.path()).unwrap().count(), 1);
}
#[test]
fn paths_normalization_collisions_and_user_directory_are_validated() {
    for bad in [
        "../outside",
        "/absolute",
        "a/../b",
        "a/./b",
        "a//b",
        "a/CON",
        "a/NUL.txt",
        "a/com1.md",
        "a/name.",
        "a/name ",
        "a/b\\c",
        "a/e\u{301}",
        ".architalk/exports",
    ] {
        assert!(files::validate_relative_path(bad).is_err(), "{bad}");
    }
    assert!(files::validate_relative_path(&format!("docs/{}", "가".repeat(60))).is_err());
    let root = tempfile::tempdir().unwrap();
    fs::create_dir(root.path().join("Docs")).unwrap();
    assert!(files::preview(root.path(), "topic", "body", 1, "s").is_err());
    let custom =
        files::preview_in(root.path(), "설계 문서/기본", "cafe\u{301}", "body", 1, "s").unwrap();
    assert!(custom.relative_directory.ends_with("café"));
    let reserved = files::reserve(root.path(), &custom).unwrap();
    assert_eq!(
        files::apply(root.path(), &reserved).unwrap().status,
        ExportStatus::Completed
    );
    let mut bad = files::preview_in(root.path(), "different", "topic", "body", 1, "s").unwrap();
    bad.files[0].relative_path = "../escape.md".into();
    assert!(files::reserve(root.path(), &bad).is_err());
    assert_eq!(
        files::apply(root.path(), &bad).unwrap().status,
        ExportStatus::Failed
    );
    assert!(!root.path().join("escape.md").exists());
}
#[test]
fn unexpected_files_and_wrong_hashes_cannot_complete() {
    let root = tempfile::tempdir().unwrap();
    let mut p = plan(root.path());
    p.files[0].hash = "forged".into();
    assert!(files::reserve(root.path(), &p).is_err());
    assert!(!root.path().join("docs").exists());
    let p = files::reserve(root.path(), &plan(root.path())).unwrap();
    let dir = root.path().join(&p.relative_directory);
    fs::write(dir.join("external.txt"), "preserved").unwrap();
    assert_eq!(
        files::apply(root.path(), &p).unwrap().status,
        ExportStatus::Conflict
    );
    assert!(!dir.join(".architalk-export.json").exists());
}
#[test]
fn publication_permission_failure_is_retryable_and_does_not_fake_completion() {
    if unsafe { libc::geteuid() } == 0 {
        return;
    }
    let root = tempfile::tempdir().unwrap();
    let p = files::reserve(root.path(), &plan(root.path())).unwrap();
    let dir = root.path().join(&p.relative_directory);
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o500)).unwrap();
    let failed = files::apply(root.path(), &p).unwrap();
    assert_eq!(failed.status, ExportStatus::Failed);
    assert!(!dir.join(".architalk-export.json").exists());
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o700)).unwrap();
    assert_eq!(
        files::reconcile(root.path(), &p).unwrap().status,
        ExportStatus::Completed
    );
}
#[test]
fn scanner_excludes_sensitive_paths_and_backend_revalidates_selection() {
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    for name in [
        "README.md",
        "Cargo.toml",
        "main.rs",
        ".env.local",
        "credentials.json",
        "id_rsa",
        "keys.json",
    ] {
        fs::write(root.path().join(name), "safe text").unwrap();
    }
    fs::write(outside.path().join("README.txt"), "not approved").unwrap();
    symlink(
        outside.path().join("README.txt"),
        root.path().join("README.txt"),
    )
    .unwrap();
    fs::create_dir(root.path().join(".git")).unwrap();
    let scan = files::scan(root.path()).unwrap();
    assert!(
        scan.files
            .iter()
            .any(|f| f.path == "README.md" && f.selectable)
    );
    assert!(
        scan.files
            .iter()
            .any(|f| f.path == "main.rs" && !f.selectable)
    );
    for bad in [
        ".env.local",
        "credentials.json",
        "id_rsa",
        "keys.json",
        "README.txt",
    ] {
        assert!(!scan.files.iter().any(|f| f.path == bad), "{bad}");
    }
    for bad in [
        "../README.md",
        "README.txt",
        ".env.local",
        "main.rs",
        "sub/README.md",
    ] {
        assert!(
            files::read_selected(root.path(), &[bad.into()]).is_err(),
            "{bad}"
        );
    }
    let old = files::read_selected(root.path(), &["README.md".into()]).unwrap();
    fs::write(root.path().join("README.md"), "new text").unwrap();
    let new = files::read_selected(root.path(), &["README.md".into()]).unwrap();
    assert_ne!(old[0].hash, new[0].hash);
    assert_eq!(new[0].text, "new text");
    assert!(files::read_selected(root.path(), &["README.md".into(), "README.md".into()]).is_err());
}
#[test]
fn scanner_applies_binary_per_file_and_total_limits() {
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("README.md"), b"text\0binary").unwrap();
    assert!(files::read_selected(root.path(), &["README.md".into()]).is_err());
    fs::write(root.path().join("README.md"), [0xff, 0xfe]).unwrap();
    assert!(files::read_selected(root.path(), &["README.md".into()]).is_err());
    fs::write(
        root.path().join("README.md"),
        vec![b'x'; files::FILE_LIMIT + 1],
    )
    .unwrap();
    assert!(files::read_selected(root.path(), &["README.md".into()]).is_err());
    let names = [
        "README",
        "README.md",
        "README.txt",
        "README.rst",
        "Cargo.toml",
        "package.json",
        "go.mod",
        "pyproject.toml",
        "requirements.txt",
    ];
    for name in names {
        fs::write(root.path().join(name), vec![b'x'; files::FILE_LIMIT]).unwrap();
    }
    assert!(
        files::read_selected(
            root.path(),
            &names.iter().map(|s| s.to_string()).collect::<Vec<_>>()
        )
        .is_err()
    );
    assert_eq!(
        files::read_selected(
            root.path(),
            &names[..8].iter().map(|s| s.to_string()).collect::<Vec<_>>()
        )
        .unwrap()
        .len(),
        8
    );
}
#[test]
fn parent_symlink_swap_race_never_publishes_outside_root() {
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let p = files::reserve(root.path(), &plan(root.path())).unwrap();
    let bundle = root.path().join(&p.relative_directory);
    let holding = root.path().join("holding");
    let destination = outside.path().to_path_buf();
    let stop = Arc::new(AtomicBool::new(false));
    let flag = stop.clone();
    let thread = std::thread::spawn(move || {
        while !flag.load(Ordering::Relaxed) {
            if fs::rename(&bundle, &holding).is_ok() {
                let _ = symlink(&destination, &bundle);
                std::thread::yield_now();
                let _ = fs::remove_file(&bundle);
                fs::rename(&holding, &bundle).unwrap();
            }
        }
    });
    for _ in 0..50 {
        let _ = files::apply(root.path(), &p).unwrap();
    }
    stop.store(true, Ordering::Relaxed);
    thread.join().unwrap();
    assert!(fs::read_dir(outside.path()).unwrap().next().is_none());
    assert_eq!(
        files::reconcile(root.path(), &p).unwrap().status,
        ExportStatus::Completed
    );
}

#[test]
fn store_derived_gateway_stays_on_original_root_after_regular_directory_swap() {
    let parent = tempfile::tempdir().unwrap();
    let project = parent.path().join("project");
    fs::create_dir(&project).unwrap();
    fs::write(project.join("README.md"), "original evidence").unwrap();
    let mut store = architalk::storage::Store::open(&project).unwrap();
    let gateway = store.project_files().unwrap();
    let moved = parent.path().join("original-project");
    fs::rename(&project, &moved).unwrap();
    fs::create_dir(&project).unwrap();
    fs::write(project.join("README.md"), "replacement evidence").unwrap();
    assert_eq!(
        gateway.read_selected(&["README.md".into()]).unwrap()[0].text,
        "original evidence"
    );
    assert!(
        store.project_files().is_err(),
        "the Store refuses a replaced project name"
    );
    let p = gateway.preview("고정 root", "# Original", 1, "s").unwrap();
    let p = gateway.reserve(&p).unwrap();
    assert_eq!(gateway.apply(&p).unwrap().status, ExportStatus::Completed);
    assert!(
        moved
            .join(&p.relative_directory)
            .join("아키텍처-설계.md")
            .exists()
    );
    assert!(!project.join("docs").exists());
    assert!(
        store
            .commit_workspace(&serde_json::json!({"root":"original"}), None, None)
            .is_err()
    );
    assert!(!project.join(".architalk").exists());
}
#[test]
fn pinned_gateway_ignores_original_root_path_replaced_with_external_symlink() {
    let parent = tempfile::tempdir().unwrap();
    let project = parent.path().join("project");
    fs::create_dir(&project).unwrap();
    fs::write(project.join("README.md"), "original").unwrap();
    let store = architalk::storage::Store::open(&project).unwrap();
    let gateway = store.project_files().unwrap();
    let moved = parent.path().join("moved");
    fs::rename(&project, &moved).unwrap();
    let outside = tempfile::tempdir().unwrap();
    fs::write(outside.path().join("README.md"), "external").unwrap();
    symlink(outside.path(), &project).unwrap();
    assert!(files::read_selected(&project, &["README.md".into()]).is_err());
    assert_eq!(
        gateway.read_selected(&["README.md".into()]).unwrap()[0].text,
        "original"
    );
    let p = gateway.preview("fixed", "bytes", 1, "s").unwrap();
    let p = gateway.reserve(&p).unwrap();
    assert_eq!(
        gateway.reconcile(&p).unwrap().status,
        ExportStatus::Completed
    );
    assert_eq!(fs::read_dir(outside.path()).unwrap().count(), 1);
}
