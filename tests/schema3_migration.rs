use architalk::storage::Store;
use rusqlite::{Connection, params};
use serde_json::json;
use std::{fs, path::Path, process::Command, time::Duration};

fn prior_database(path: &Path, version: i64) -> Connection {
    let connection = Connection::open(path).unwrap();
    connection.execute_batch("CREATE TABLE workspace (id INTEGER PRIMARY KEY CHECK(id=1), snapshot TEXT NOT NULL);
        CREATE TABLE receipts (command_id TEXT PRIMARY KEY, payload_hash TEXT NOT NULL, response TEXT NOT NULL);
        CREATE TABLE events (cursor INTEGER PRIMARY KEY AUTOINCREMENT, session_id TEXT, kind TEXT NOT NULL, payload TEXT NOT NULL);
        CREATE INDEX events_by_session ON events(session_id,cursor);").unwrap();
    connection
        .pragma_update(None, "user_version", version)
        .unwrap();
    connection
}

#[test]
fn v2_wal_backup_preserves_history_and_new_checkpoints_survive_reopen() {
    let root = tempfile::tempdir().unwrap();
    let directory = root.path().join(".architalk");
    fs::create_dir(&directory).unwrap();
    let path = directory.join("workspace.db");
    let old = json!({"sessions":[{"messages":[{"content":"synthetic old message"}],"drafts":[{"markdown":"synthetic old draft"}]}]});
    {
        let connection = prior_database(&path, 2);
        connection
            .pragma_update(None, "journal_mode", "WAL")
            .unwrap();
        connection
            .pragma_update(None, "wal_autocheckpoint", 0)
            .unwrap();
        connection
            .execute("INSERT INTO workspace VALUES(1,?1)", [old.to_string()])
            .unwrap();
        connection
            .execute(
                "INSERT INTO receipts VALUES(?1,?2,?3)",
                params!["original-command", "payload", r#"{"saved":true}"#],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO events(session_id,kind,payload) VALUES('session','previous','{}')",
                [],
            )
            .unwrap();
        // Closing without a final checkpoint leaves committed pages in the WAL.
        unsafe {
            rusqlite::ffi::sqlite3_db_config(
                connection.handle(),
                rusqlite::ffi::SQLITE_DBCONFIG_NO_CKPT_ON_CLOSE,
                1,
                std::ptr::null_mut::<i32>(),
            );
        }
    }
    assert!(
        fs::metadata(directory.join("workspace.db-wal"))
            .unwrap()
            .len()
            > 0
    );
    let mut store = Store::open(root.path()).unwrap();
    assert_eq!(store.load_workspace().unwrap().unwrap(), old);
    assert_eq!(
        store
            .find_receipt("original-command")
            .unwrap()
            .unwrap()
            .response,
        json!({"saved":true})
    );
    assert_eq!(store.events_after(Some("session"), 0).unwrap().len(), 1);
    let mut staged = old.clone();
    staged["sessions"][0]["design_generations"] =
        json!([{"completed":[{"kind":"drivers","markdown":"synthetic validated checkpoint"}]}]);
    store.commit_workspace(&staged, None, None).unwrap();
    drop(store);
    let backup_paths: Vec<_> = fs::read_dir(&directory)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("workspace.pre-v3-")
                && path.extension().is_some_and(|ext| ext == "db")
        })
        .collect();
    assert_eq!(backup_paths.len(), 1);
    let backup = Connection::open(&backup_paths[0]).unwrap();
    assert_eq!(
        backup
            .pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))
            .unwrap(),
        2
    );
    let saved: String = backup
        .query_row("SELECT snapshot FROM workspace", [], |row| row.get(0))
        .unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&saved).unwrap(),
        old
    );
    assert_eq!(
        backup
            .query_row("SELECT count(*) FROM receipts", [], |row| row
                .get::<_, i64>(0))
            .unwrap(),
        1
    );
    assert_eq!(
        backup
            .query_row("SELECT count(*) FROM events", [], |row| row
                .get::<_, i64>(0))
            .unwrap(),
        1
    );
    let reopened = Store::open(root.path()).unwrap();
    assert_eq!(reopened.load_workspace().unwrap().unwrap(), staged);
    drop(reopened);
    let migrated = Connection::open(path).unwrap();
    assert_eq!(
        migrated
            .pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))
            .unwrap(),
        3
    );
}

#[test]
#[ignore = "requires the previously built schema2 release in ARCHITALK_SCHEMA2_BINARY; never calls a model"]
fn the_actual_schema2_binary_refuses_schema3_without_rewriting_the_database() {
    let binary =
        std::env::var_os("ARCHITALK_SCHEMA2_BINARY").expect("set ARCHITALK_SCHEMA2_BINARY");
    let root = tempfile::tempdir().unwrap();
    let mut store = Store::open(root.path()).unwrap();
    store
        .commit_workspace(
            &json!({"checkpoint":"schema3 data must survive"}),
            None,
            None,
        )
        .unwrap();
    drop(store);
    let path = root.path().join(".architalk/workspace.db");
    let before = fs::read(&path).unwrap();
    let mut child = Command::new(binary)
        .args(["--no-open", "--project"])
        .arg(root.path())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    loop {
        if child.try_wait().unwrap().is_some() {
            break;
        }
        if std::time::Instant::now() >= deadline {
            child.kill().unwrap();
            panic!("schema2 binary did not refuse schema3 before starting");
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    let output = child.wait_with_output().unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("unsupported_schema"));
    let after = fs::read(path).unwrap();
    assert_eq!(after, before);
}
