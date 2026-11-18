use architalk::storage::{EventInput, ReceiptInput, Store};
use serde_json::json;
use std::{fs, os::unix::fs::symlink};
#[test]
fn p0_sqlite_atomic_commit_reopen_and_lock() {
    let root = tempfile::tempdir().unwrap();
    let mut store = Store::open(root.path()).unwrap();
    assert!(Store::open(root.path()).is_err());
    let event = store
        .commit_workspace(
            &json!({"revision":1,"run":{"status":"running"}}),
            Some(ReceiptInput {
                command_id: "one".into(),
                payload_hash: "hash".into(),
                response: json!({"ok":true}),
            }),
            Some(EventInput {
                session_id: Some("s".into()),
                kind: "changed".into(),
                payload: json!({"revision":1}),
            }),
        )
        .unwrap()
        .unwrap();
    assert_eq!(event.cursor, 1);
    assert!(
        store
            .commit_workspace(
                &json!({"revision":2}),
                Some(ReceiptInput {
                    command_id: "one".into(),
                    payload_hash: "different".into(),
                    response: json!({})
                }),
                Some(EventInput {
                    session_id: None,
                    kind: "must-rollback".into(),
                    payload: json!({})
                })
            )
            .is_err()
    );
    assert_eq!(store.load_workspace().unwrap().unwrap()["revision"], 1);
    assert_eq!(store.events_after(None, 0).unwrap().len(), 1);
    drop(store);
    let reopened = Store::open(root.path()).unwrap();
    assert_eq!(reopened.load_workspace().unwrap().unwrap()["revision"], 1);
    assert!(reopened.find_receipt("one").unwrap().is_some());
    assert_eq!(reopened.events_after(Some("s"), 0).unwrap().len(), 1);
    assert!(reopened.events_after(Some("other"), 0).unwrap().is_empty());
}
#[test]
fn p0_sqlite_refuses_state_db_and_sidecar_symlinks() {
    for target in [
        ".architalk",
        "workspace.lock",
        "workspace.db",
        "workspace.db-journal",
    ] {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let marker = outside.path().join("marker");
        fs::write(&marker, "preserved").unwrap();
        if target == ".architalk" {
            symlink(outside.path(), root.path().join(target)).unwrap();
        } else {
            fs::create_dir(root.path().join(".architalk")).unwrap();
            symlink(&marker, root.path().join(".architalk").join(target)).unwrap();
        }
        assert!(Store::open(root.path()).is_err(), "{target}");
        assert_eq!(fs::read_to_string(marker).unwrap(), "preserved");
    }
}
#[test]
fn p0_sqlite_state_directory_swap_does_not_follow_external_symlink() {
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let mut store = Store::open(root.path()).unwrap();
    fs::rename(
        root.path().join(".architalk"),
        root.path().join("saved-state"),
    )
    .unwrap();
    symlink(outside.path(), root.path().join(".architalk")).unwrap();
    let previous_bytes = fs::read(root.path().join("saved-state/workspace.db")).unwrap();
    let error = store
        .commit_workspace(&json!({"pinned":true}), None, None)
        .unwrap_err();
    assert!(error.is::<architalk::storage::WorkspaceResetRequired>());
    assert!(store.load_workspace().is_err());
    assert!(fs::read_dir(outside.path()).unwrap().next().is_none());
    assert_eq!(
        fs::read(root.path().join("saved-state/workspace.db")).unwrap(),
        previous_bytes
    );
}
