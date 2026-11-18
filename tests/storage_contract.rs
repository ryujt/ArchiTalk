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
            connection.execute_batch("PRAGMA user_version=3; CREATE TABLE future_data(value TEXT); INSERT INTO future_data VALUES('preserved');").unwrap();
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

fn create_v1_database(path: &std::path::Path) -> rusqlite::Connection {
    let connection = rusqlite::Connection::open(path).unwrap();
    connection.execute_batch("CREATE TABLE workspace (id INTEGER PRIMARY KEY CHECK(id=1), snapshot TEXT NOT NULL);
        CREATE TABLE receipts (command_id TEXT PRIMARY KEY, payload_hash TEXT NOT NULL, response TEXT NOT NULL);
        CREATE TABLE events (cursor INTEGER PRIMARY KEY AUTOINCREMENT, session_id TEXT, kind TEXT NOT NULL, payload TEXT NOT NULL);
        CREATE INDEX events_by_session ON events(session_id,cursor);
        PRAGMA user_version=1;").unwrap();
    connection
}

fn v2_backups(directory: &std::path::Path) -> Vec<std::path::PathBuf> {
    fs::read_dir(directory)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("workspace.pre-v2-")
                && path.extension().is_some_and(|extension| extension == "db")
        })
        .collect()
}

#[test]
fn v1_snapshot_receipts_and_events_survive_a_backed_up_v2_marker_migration() {
    let root = tempfile::tempdir().unwrap();
    let directory = root.path().join(".architalk");
    fs::create_dir(&directory).unwrap();
    let path = directory.join("workspace.db");
    {
        let connection = create_v1_database(&path);
        connection
            .execute(
                "INSERT INTO workspace VALUES(1,?1)",
                [r#"{"drafts":[{"markdown":"legacy bytes"}]}"#],
            )
            .unwrap();
        connection.execute_batch("INSERT INTO receipts VALUES('command','payload','{\"ok\":true}');
            INSERT INTO events(session_id,kind,payload) VALUES('session','saved','{\"legacy\":true}');").unwrap();
    }
    let mut store = Store::open(root.path()).unwrap();
    assert_eq!(
        store.load_workspace().unwrap().unwrap()["drafts"][0]["markdown"],
        "legacy bytes"
    );
    assert_eq!(
        store.find_receipt("command").unwrap().unwrap().response,
        json!({"ok":true})
    );
    assert_eq!(
        store.events_after(Some("session"), 0).unwrap()[0].payload,
        json!({"legacy":true})
    );
    let next = store
        .commit_workspace(
            &json!({"after":true}),
            None,
            Some(EventInput {
                session_id: Some("session".into()),
                kind: "next".into(),
                payload: json!({}),
            }),
        )
        .unwrap()
        .unwrap();
    assert_eq!(next.cursor, 2);
    drop(store);
    let backups = v2_backups(&directory);
    assert_eq!(backups.len(), 1);
    let backup = rusqlite::Connection::open(&backups[0]).unwrap();
    assert_eq!(
        backup
            .pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))
            .unwrap(),
        1
    );
    assert_eq!(
        backup
            .query_row("SELECT snapshot FROM workspace", [], |row| row
                .get::<_, String>(0))
            .unwrap(),
        r#"{"drafts":[{"markdown":"legacy bytes"}]}"#
    );
    assert_eq!(
        backup
            .query_row("SELECT COUNT(*) FROM receipts", [], |row| row
                .get::<_, i64>(0))
            .unwrap(),
        1
    );
    assert_eq!(
        backup
            .query_row("SELECT COUNT(*) FROM events", [], |row| row
                .get::<_, i64>(0))
            .unwrap(),
        1
    );
    drop(backup);
    drop(Store::open(root.path()).unwrap());
    assert_eq!(v2_backups(&directory).len(), 1);
    let migrated = rusqlite::Connection::open(path).unwrap();
    assert_eq!(
        migrated
            .pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))
            .unwrap(),
        2
    );
}

#[test]
fn migration_backup_includes_committed_pages_that_exist_only_in_the_v1_wal() {
    let root = tempfile::tempdir().unwrap();
    let directory = root.path().join(".architalk");
    fs::create_dir(&directory).unwrap();
    let path = directory.join("workspace.db");
    {
        let connection = create_v1_database(&path);
        connection
            .pragma_update(None, "journal_mode", "WAL")
            .unwrap();
        connection
            .pragma_update(None, "wal_autocheckpoint", 0)
            .unwrap();
        connection
            .execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
            .unwrap();
        connection
            .set_db_config(
                rusqlite::config::DbConfig::SQLITE_DBCONFIG_NO_CKPT_ON_CLOSE,
                true,
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO workspace VALUES(1,?1)",
                [r#"{"wal":"committed snapshot"}"#],
            )
            .unwrap();
        connection
            .execute_batch("INSERT INTO receipts VALUES('wal-command','payload','{\"wal\":true}');")
            .unwrap();
    }
    assert!(
        fs::metadata(directory.join("workspace.db-wal"))
            .unwrap()
            .len()
            > 0
    );
    let store = Store::open(root.path()).unwrap();
    assert_eq!(
        store.load_workspace().unwrap().unwrap(),
        json!({"wal":"committed snapshot"})
    );
    drop(store);
    let backups = v2_backups(&directory);
    assert_eq!(backups.len(), 1);
    let backup = rusqlite::Connection::open(&backups[0]).unwrap();
    assert_eq!(
        backup
            .query_row("SELECT snapshot FROM workspace", [], |row| row
                .get::<_, String>(0))
            .unwrap(),
        r#"{"wal":"committed snapshot"}"#
    );
    assert_eq!(
        backup
            .query_row(
                "SELECT response FROM receipts WHERE command_id='wal-command'",
                [],
                |row| row.get::<_, String>(0)
            )
            .unwrap(),
        r#"{"wal":true}"#
    );
    assert_eq!(
        backup
            .pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))
            .unwrap(),
        1
    );
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
