use architalk::storage::{EventInput, ReceiptInput, Store};
use serde_json::json;
#[test]
fn multi_megabyte_snapshots_commit_and_reopen_beyond_sqlite_page_cache() {
    let root = tempfile::tempdir().unwrap();
    let mut store = Store::open(root.path()).unwrap();
    let document = "x".repeat(3 * 1024 * 1024);
    let snapshot =
        json!({"revision":1,"drafts":[{"markdown":document}],"run":{"status":"completed"}});
    let started = std::time::Instant::now();
    store
        .commit_workspace(
            &snapshot,
            Some(ReceiptInput {
                command_id: "large-one".into(),
                payload_hash: "hash".into(),
                response: json!({"status":"accepted"}),
            }),
            Some(EventInput {
                session_id: Some("s".into()),
                kind: "completed".into(),
                payload: json!({"revision":1}),
            }),
        )
        .unwrap();
    let second = json!({"revision":2,"drafts":[{"markdown":"y".repeat(4*1024*1024)}],"run":{"status":"completed"}});
    store
        .commit_workspace(
            &second,
            None,
            Some(EventInput {
                session_id: Some("s".into()),
                kind: "completed".into(),
                payload: json!({"revision":2}),
            }),
        )
        .unwrap();
    assert_eq!(store.load_workspace().unwrap(), Some(second.clone()));
    drop(store);
    let store = Store::open(root.path()).unwrap();
    assert_eq!(store.load_workspace().unwrap(), Some(second));
    assert!(store.find_receipt("large-one").unwrap().is_some());
    assert_eq!(store.latest_cursor().unwrap(), 2);
    assert!(started.elapsed() < std::time::Duration::from_secs(30));
}
