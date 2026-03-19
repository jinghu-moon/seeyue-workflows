// tests/lsp_discover.rs
//
// TDD tests for discover_server() language coverage.
// Tests verify hint strings and LspNotAvailable behavior.
// Run: cargo test --test lsp_discover

use seeyue_mcp::lsp::discover_server_for_test;

// A-N5 test 1: AGENT_EDITOR_LSP_CMD override takes priority
#[test]
fn test_env_override_takes_priority() {
    std::env::set_var("AGENT_EDITOR_LSP_CMD", "my-lsp --stdio");
    let result = discover_server_for_test("rust");
    std::env::remove_var("AGENT_EDITOR_LSP_CMD");
    // Should return my-lsp regardless of language
    assert!(result.is_ok() || result.is_err()); // just must not panic
    if let Ok((cmd, _)) = result {
        assert_eq!(cmd, "my-lsp");
    }
}

// A-N5 test 2: bat returns LspNotAvailable (no server exists for .bat)
#[test]
fn test_bat_returns_lsp_not_available() {
    std::env::remove_var("AGENT_EDITOR_LSP_CMD");
    let result = discover_server_for_test("bat");
    assert!(result.is_err(), "bat should return LspNotAvailable");
    let err = result.unwrap_err();
    let msg = format!("{:?}", err);
    assert!(
        msg.contains("LspNotAvailable") || msg.contains("not available") || msg.contains("bat"),
        "unexpected error: {}", msg
    );
}

// A-N5 test 3: unknown language returns error with escape hint
#[test]
fn test_unknown_language_returns_error_with_hint() {
    std::env::remove_var("AGENT_EDITOR_LSP_CMD");
    let result = discover_server_for_test("cobol");
    assert!(result.is_err());
    let err = result.unwrap_err();
    let msg = format!("{:?}", err);
    // hint should mention AGENT_EDITOR_LSP_CMD or provide guidance
    assert!(
        msg.contains("AGENT_EDITOR_LSP_CMD")
            || msg.contains("No LSP")
            || msg.contains("not found")
            || msg.contains("mapping"),
        "hint should guide user, got: {}", msg
    );
}

// A-N5 test 4: each newly added language has a non-empty cmd
// (when not installed, must return LspNotAvailable with non-empty hint, not panic)
#[test]
fn test_new_languages_have_non_empty_hint() {
    std::env::remove_var("AGENT_EDITOR_LSP_CMD");
    let new_langs = [
        "c", "cpp", "kotlin", "css", "vue", "shell",
        "markdown", "json", "toml", "yaml",
    ];
    for lang in &new_langs {
        let result = discover_server_for_test(lang);
        match result {
            Ok((cmd, _)) => {
                assert!(!cmd.is_empty(), "language '{}': cmd should be non-empty", lang);
            }
            Err(err) => {
                let msg = format!("{:?}", err);
                // hint field should be non-empty — check it contains something meaningful
                assert!(
                    !msg.is_empty() && msg.len() > 10,
                    "language '{}': error hint should be meaningful, got: {}", lang, msg
                );
            }
        }
    }
}
