// tests/test_checkpoint_list.rs
use seeyue_mcp::tools::checkpoint_list::{CheckpointListParams, run_checkpoint_list};
use seeyue_mcp::storage::checkpoint::CheckpointStore;

#[test]
fn test_checkpoint_list_empty() {
    let tmp = tempfile::tempdir().unwrap();
    let session_id = "test_sess";
    let store = CheckpointStore::open(session_id, tmp.path()).unwrap();
    let result = run_checkpoint_list(CheckpointListParams {}, &store).unwrap();
    assert_eq!(result.kind, "empty");
    assert_eq!(result.total, 0);
}

#[test]
fn test_checkpoint_list_after_capture() {
    let tmp = tempfile::tempdir().unwrap();
    // Create a temp file to snapshot
    let file = tmp.path().join("test.rs");
    std::fs::write(&file, "fn main() {}").unwrap();

    let store = CheckpointStore::open("sess", tmp.path()).unwrap();
    store.capture(&file, "call_1", "write").unwrap();

    let result = run_checkpoint_list(CheckpointListParams {}, &store).unwrap();
    assert_eq!(result.kind, "success");
    assert!(result.total >= 1);
    assert!(!result.checkpoints[0].captured_at.is_empty());
}
