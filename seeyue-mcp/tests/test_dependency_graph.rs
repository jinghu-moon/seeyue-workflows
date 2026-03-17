// tests/test_dependency_graph.rs
//
// Tests for tools::dependency_graph::run_dependency_graph.
// Run: cargo test --test test_dependency_graph

use seeyue_mcp::tools::dependency_graph::{DependencyGraphParams, run_dependency_graph};

fn ws() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

fn params(path: &str) -> DependencyGraphParams {
    DependencyGraphParams {
        path:      path.into(),
        depth:     Some(1),
        direction: Some("imports".into()),
    }
}

#[test]
fn test_dependency_graph_rust_file_ok() {
    let result = run_dependency_graph(params("src/main.rs"), &ws()).unwrap();
    assert_eq!(result.status, "ok");
}

#[test]
fn test_dependency_graph_nodes_accessible() {
    let result = run_dependency_graph(params("src/main.rs"), &ws()).unwrap();
    let _ = result.nodes.len();
}

#[test]
fn test_dependency_graph_direction_returned() {
    let result = run_dependency_graph(params("src/main.rs"), &ws()).unwrap();
    assert!(
        result.direction == "imports"
            || result.direction == "imported_by"
            || result.direction == "both",
        "unexpected direction: {}", result.direction
    );
}

#[test]
fn test_dependency_graph_path_escape_blocked() {
    let err = run_dependency_graph(
        DependencyGraphParams { path: "../../outside.rs".into(), depth: None, direction: None },
        &ws(),
    ).unwrap_err();
    let msg = format!("{err:?}");
    assert!(
        msg.contains("PathEscape") || msg.contains("outside") || msg.contains("FileNotFound"),
        "unexpected error: {msg}"
    );
}

#[test]
fn test_dependency_graph_nonexistent_file_errors() {
    let err = run_dependency_graph(
        DependencyGraphParams {
            path:      "src/does_not_exist_xyz.rs".into(),
            depth:     None,
            direction: None,
        },
        &ws(),
    ).unwrap_err();
    let msg = format!("{err:?}");
    assert!(
        msg.contains("FileNotFound") || msg.contains("NotFound") || msg.contains("IoError"),
        "unexpected error: {msg}"
    );
}

#[test]
fn test_dependency_graph_edges_accessible() {
    let result = run_dependency_graph(params("src/main.rs"), &ws()).unwrap();
    let _ = result.edges.len();
}
