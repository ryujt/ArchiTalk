use architalk::storage::{EventInput, Store};
use serde_json::json;
use std::{fs, os::unix::fs::symlink, process::Command};
#[test]
fn future_schema_and_corruption_preserve_original_database() {
    for future in [true, false] {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir(root.path().join(".architalk")).unwrap();
        let path = root.path().join(".architalk/workspace.db");
        if future {
            let connection = rusqlite::Connection::open(&path).unwrap();
            connection.execute_batch("PRAGMA user_version=2; CREATE TABLE future_data(value TEXT); INSERT INTO future_data VALUES('preserved');").unwrap();
        } else {
            fs::write(&path, b"corrupt database must survive").unwrap();
        }
        let original = fs::read(&path).unwrap();
        assert!(Store::open(root.path()).is_err());
        assert_eq!(fs::read(path).unwrap(), original);
    }
}
#[test]
fn unversioned_database_is_backed_up_before_migration() {
    let root = tempfile::tempdir().unwrap();
    let dir = root.path().join(".architalk");
    fs::create_dir(&dir).unwrap();
    let path = dir.join("workspace.db");
    {
        let db = rusqlite::Connection::open(path).unwrap();
        db.execute_batch("CREATE TABLE legacy(value TEXT);INSERT INTO legacy VALUES('original');")
            .unwrap();
    }
    let store = Store::open(root.path()).unwrap();
    assert!(store.load_workspace().unwrap().is_none());
    drop(store);
    let backup = fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .find(|p| {
            p.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("workspace.pre-v1-")
        })
        .unwrap();
    let db = rusqlite::Connection::open(backup).unwrap();
    let value: String = db
        .query_row("SELECT value FROM legacy", [], |r| r.get(0))
        .unwrap();
    assert_eq!(value, "original");
    let version: i64 = db
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .unwrap();
    assert_eq!(version, 0);
}
#[test]
fn sqlite_journal_symlink_created_after_open_cannot_modify_external_file() {
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let marker = outside.path().join("marker");
    fs::write(&marker, "preserved").unwrap();
    let mut store = Store::open(root.path()).unwrap();
    store
        .commit_workspace(&json!({"revision":1}), None, None)
        .unwrap();
    symlink(&marker, root.path().join(".architalk/workspace.db-journal")).unwrap();
    assert!(
        store
            .commit_workspace(&json!({"revision":2}), None, None)
            .is_err()
    );
    assert_eq!(fs::read_to_string(marker).unwrap(), "preserved");
    // Unsafe journal presence blocks reads as well as writes.
    assert!(store.load_workspace().is_err());
    fs::remove_file(root.path().join(".architalk/workspace.db-journal")).unwrap();
    assert_eq!(store.load_workspace().unwrap().unwrap()["revision"], 1);
}
#[test]
fn receipts_and_events_survive_restart_with_monotonic_cursor_and_filter() {
    let root = tempfile::tempdir().unwrap();
    let mut store = Store::open(root.path()).unwrap();
    for (i, s) in ["first", "second", "first"].into_iter().enumerate() {
        let event = store
            .commit_workspace(
                &json!({"revision":i+1,"export":{"status":"applying"}}),
                None,
                Some(EventInput {
                    session_id: Some(s.into()),
                    kind: "changed".into(),
                    payload: json!({"revision":i+1}),
                }),
            )
            .unwrap()
            .unwrap();
        assert_eq!(event.cursor, i as u64 + 1);
    }
    assert_eq!(store.latest_cursor().unwrap(), 3);
    drop(store);
    let store = Store::open(root.path()).unwrap();
    let events = store.events_after(Some("first"), 1).unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].cursor, 3);
    assert!(store.events_after(None, u64::MAX).unwrap().is_empty());
    assert_eq!(
        store.load_workspace().unwrap().unwrap()["export"]["status"],
        "applying"
    );
}
#[test]
fn project_lock_is_held_across_real_process_boundary() {
    let root = tempfile::tempdir().unwrap();
    let store = Store::open(root.path()).unwrap();
    let output = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "lock_child_probe", "--nocapture"])
        .env("ARCHITALK_TEST_LOCK_ROOT", root.path())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    drop(store);
    assert!(Store::open(root.path()).is_ok());
}
#[test]
fn lock_child_probe() {
    if let Some(root) = std::env::var_os("ARCHITALK_TEST_LOCK_ROOT") {
        assert!(Store::open(std::path::Path::new(&root)).is_err());
    }
}

#[test]
fn sqlite_journal_hardlink_created_after_open_cannot_modify_external_file() {
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let marker = outside.path().join("marker");
    fs::write(&marker, "preserved").unwrap();
    let mut store = Store::open(root.path()).unwrap();
    store
        .commit_workspace(&json!({"revision":1}), None, None)
        .unwrap();
    let journal = root.path().join(".architalk/workspace.db-journal");
    fs::hard_link(&marker, &journal).unwrap();
    assert!(
        store
            .commit_workspace(&json!({"revision":2}), None, None)
            .is_err()
    );
    assert_eq!(fs::read_to_string(&marker).unwrap(), "preserved");
    fs::remove_file(journal).unwrap();
    assert_eq!(store.load_workspace().unwrap().unwrap()["revision"], 1);
}

#[test]
fn project_inode_lock_survives_state_directory_replacement() {
    let root = tempfile::tempdir().unwrap();
    let mut store = Store::open(root.path()).unwrap();
    fs::rename(
        root.path().join(".architalk"),
        root.path().join("saved-state"),
    )
    .unwrap();
    fs::create_dir(root.path().join(".architalk")).unwrap();
    assert!(Store::open(root.path()).is_err());
    assert!(
        fs::read_dir(root.path().join(".architalk"))
            .unwrap()
            .next()
            .is_none()
    );
    store
        .commit_workspace(&json!({"held":"original"}), None, None)
        .unwrap();
    drop(store);
    assert!(Store::open(root.path()).is_ok());
}
#[test]
fn fifo_journal_does_not_block_before_regular_file_validation() {
    let root = tempfile::tempdir().unwrap();
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "fifo_child_probe", "--nocapture"])
        .env("ARCHITALK_TEST_FIFO_ROOT", root.path())
        .spawn()
        .unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            assert!(status.success());
            break;
        }
        if std::time::Instant::now() > deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("SQLite FIFO journal open blocked");
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}
#[test]
fn fifo_child_probe() {
    if let Some(root) = std::env::var_os("ARCHITALK_TEST_FIFO_ROOT") {
        let root = std::path::Path::new(&root);
        let mut store = Store::open(root).unwrap();
        store
            .commit_workspace(&json!({"revision":1}), None, None)
            .unwrap();
        let journal = root.join(".architalk/workspace.db-journal");
        let path = std::ffi::CString::new(journal.to_str().unwrap()).unwrap();
        assert_eq!(unsafe { libc::mkfifo(path.as_ptr(), 0o600) }, 0);
        assert!(
            store
                .commit_workspace(&json!({"revision":2}), None, None)
                .is_err()
        );
        fs::remove_file(journal).unwrap();
        assert_eq!(store.load_workspace().unwrap().unwrap()["revision"], 1);
    }
}
