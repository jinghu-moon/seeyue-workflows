use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use crate::error::ToolError;

mod protocol;

pub struct LspLocation {
    pub path:   PathBuf,
    pub line:   usize,
    pub column: usize,
}

pub struct LspSessionPool {
    sessions: HashMap<String, LspSession>,
}

impl LspSessionPool {
    pub fn new() -> Self {
        Self { sessions: HashMap::new() }
    }

    pub fn get_or_start(&mut self, language: &str, workspace: &Path) -> Result<&mut LspSession, ToolError> {
        let needs_restart = self.sessions
            .get_mut(language)
            .map(|s| !s.is_alive())
            .unwrap_or(true);

        if needs_restart {
            let (cmd, args) = discover_server(language)?;
            let session = LspSession::new(language, &cmd, &args, workspace)?;
            self.sessions.insert(language.to_string(), session);
        }

        Ok(self.sessions.get_mut(language).unwrap())
    }
}

pub struct LspSession {
    child:       Child,
    stdin:       ChildStdin,
    stdout:      BufReader<ChildStdout>,
    next_id:     u64,
    root_uri:    String,
    initialized: bool,
}

impl LspSession {
    fn new(language: &str, command: &str, args: &[String], workspace: &Path) -> Result<Self, ToolError> {
        let mut child = Command::new(command)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .current_dir(workspace)
            .spawn()
            .map_err(|e| ToolError::LspNotAvailable {
                language: language.to_string(),
                hint: format!("Failed to spawn LSP server: {e}"),
            })?;

        let stdin = child.stdin.take().ok_or_else(|| ToolError::LspError {
            message: "Failed to open LSP stdin".into(),
            hint: "Ensure the LSP server supports stdio.".into(),
        })?;
        let stdout = child.stdout.take().ok_or_else(|| ToolError::LspError {
            message: "Failed to open LSP stdout".into(),
            hint: "Ensure the LSP server supports stdio.".into(),
        })?;

        Ok(Self {
            child,
            stdin,
            stdout: BufReader::new(stdout),
            next_id: 1,
            root_uri: path_to_uri(workspace),
            initialized: false,
        })
    }

    fn is_alive(&mut self) -> bool {
        match self.child.try_wait() {
            Ok(None) => true,
            _ => false,
        }
    }

    fn ensure_initialized(&mut self) -> Result<(), ToolError> {
        if self.initialized {
            return Ok(());
        }

        let init = json!({
            "jsonrpc": "2.0",
            "id": self.next_id,
            "method": "initialize",
            "params": {
                "processId": std::process::id(),
                "rootUri": self.root_uri,
                "capabilities": {},
            }
        });
        self.next_id += 1;

        protocol::write_message(&mut self.stdin, &init)
            .map_err(|e| ToolError::LspError {
                message: e.to_string(),
                hint: "Failed to write initialize request.".into(),
            })?;

        // Wait for initialize response
        loop {
            let msg = protocol::read_message(&mut self.stdout).map_err(|e| ToolError::LspError {
                message: e.to_string(),
                hint: "Failed to read initialize response.".into(),
            })?;
            if msg.get("id") == init.get("id") {
                break;
            }
        }

        let initialized = json!({
            "jsonrpc": "2.0",
            "method": "initialized",
            "params": {}
        });
        protocol::write_message(&mut self.stdin, &initialized)
            .map_err(|e| ToolError::LspError {
                message: e.to_string(),
                hint: "Failed to send initialized notification.".into(),
            })?;

        self.initialized = true;
        Ok(())
    }

    fn send_request(&mut self, method: &str, params: Value) -> Result<Value, ToolError> {
        let id = self.next_id;
        self.next_id += 1;

        let request = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        });

        protocol::write_message(&mut self.stdin, &request)
            .map_err(|e| ToolError::LspError {
                message: e.to_string(),
                hint: "Failed to write LSP request.".into(),
            })?;

        loop {
            let msg = protocol::read_message(&mut self.stdout).map_err(|e| ToolError::LspError {
                message: e.to_string(),
                hint: "Failed to read LSP response.".into(),
            })?;

            if msg.get("id") == Some(&json!(id)) {
                if let Some(err) = msg.get("error") {
                    return Err(ToolError::LspError {
                        message: err.to_string(),
                        hint: "LSP server returned an error.".into(),
                    });
                }
                return Ok(msg.get("result").cloned().unwrap_or(Value::Null));
            }
        }
    }

    fn send_notification(&mut self, method: &str, params: Value) -> Result<(), ToolError> {
        let notification = json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
        });

        protocol::write_message(&mut self.stdin, &notification)
            .map_err(|e| ToolError::LspError {
                message: e.to_string(),
                hint: "Failed to send LSP notification.".into(),
            })
    }

    fn open_document(&mut self, uri: &str, language_id: &str, text: &str) -> Result<(), ToolError> {
        let params = json!({
            "textDocument": {
                "uri": uri,
                "languageId": language_id,
                "version": 1,
                "text": text,
            }
        });
        self.send_notification("textDocument/didOpen", params)
    }

    pub fn request_definition(
        &mut self,
        path: &Path,
        language_id: &str,
        text: &str,
        line: usize,
        column: usize,
    ) -> Result<Vec<LspLocation>, ToolError> {
        self.ensure_initialized()?;

        let uri = path_to_uri(path);
        self.open_document(&uri, language_id, text)?;

        let params = json!({
            "textDocument": { "uri": uri },
            "position": { "line": line.saturating_sub(1), "character": column.saturating_sub(1) },
        });

        let result = self.send_request("textDocument/definition", params)?;
        Ok(parse_locations(&result))
    }

    pub fn request_references(
        &mut self,
        path: &Path,
        language_id: &str,
        text: &str,
        line: usize,
        column: usize,
    ) -> Result<Vec<LspLocation>, ToolError> {
        self.ensure_initialized()?;

        let uri = path_to_uri(path);
        self.open_document(&uri, language_id, text)?;

        let params = json!({
            "textDocument": { "uri": uri },
            "position": { "line": line.saturating_sub(1), "character": column.saturating_sub(1) },
            "context": { "includeDeclaration": true },
        });

        let result = self.send_request("textDocument/references", params)?;
        Ok(parse_locations(&result))
    }
}

// ─── LSP server discovery ───────────────────────────────────────────────────

fn discover_server(language: &str) -> Result<(String, Vec<String>), ToolError> {
    if let Ok(cmdline) = std::env::var("AGENT_EDITOR_LSP_CMD") {
        let parts: Vec<String> = cmdline.split_whitespace().map(|s| s.to_string()).collect();
        if parts.is_empty() {
            return Err(ToolError::LspNotAvailable {
                language: language.to_string(),
                hint: "AGENT_EDITOR_LSP_CMD is set but empty.".into(),
            });
        }
        let cmd = parts[0].clone();
        let args = parts[1..].to_vec();
        return Ok((cmd, args));
    }

    match language {
        "rust" => pick_cmd(language, "rust-analyzer", vec![]),
        "typescript" | "tsx" | "javascript" | "jsx" => {
            pick_cmd(language, "typescript-language-server", vec!["--stdio".into()])
        }
        "python" => {
            if which::which("pyright-langserver").is_ok() {
                Ok(("pyright-langserver".into(), vec!["--stdio".into()]))
            } else if which::which("pylsp").is_ok() {
                Ok(("pylsp".into(), vec![]))
            } else {
                Err(ToolError::LspNotAvailable {
                    language: language.to_string(),
                    hint: "Install pyright-langserver or pylsp.".into(),
                })
            }
        }
        "go" => pick_cmd(language, "gopls", vec![]),
        _ => Err(ToolError::LspNotAvailable {
            language: language.to_string(),
            hint: "No LSP server mapping for this language.".into(),
        }),
    }
}

fn pick_cmd(language: &str, cmd: &str, args: Vec<String>) -> Result<(String, Vec<String>), ToolError> {
    if which::which(cmd).is_ok() {
        Ok((cmd.into(), args))
    } else {
        Err(ToolError::LspNotAvailable {
            language: language.to_string(),
            hint: format!("LSP server '{cmd}' not found in PATH."),
        })
    }
}

// ─── Language helpers ───────────────────────────────────────────────────────

pub fn language_id(language: &str) -> &'static str {
    match language {
        "rust" => "rust",
        "python" => "python",
        "typescript" => "typescript",
        "tsx" => "typescriptreact",
        "javascript" => "javascript",
        "jsx" => "javascriptreact",
        "go" => "go",
        _ => "plaintext",
    }
}

pub fn path_to_uri(path: &Path) -> String {
    let mut s = path.to_string_lossy().replace('\\', "/");
    if !s.starts_with('/') {
        s = format!("/{s}");
    }
    format!("file://{s}")
}

pub fn uri_to_path(uri: &str) -> Option<PathBuf> {
    let without_scheme = uri.strip_prefix("file://")?;
    let trimmed = without_scheme.trim_start_matches('/');
    let decoded = percent_decode(trimmed);
    let path = decoded.replace('/', "\\");
    Some(PathBuf::from(path))
}

fn percent_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(h), Some(l)) = (from_hex(bytes[i + 1]), from_hex(bytes[i + 2])) {
                out.push((h << 4) | l);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}

fn from_hex(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

fn parse_locations(value: &Value) -> Vec<LspLocation> {
    match value {
        Value::Array(arr) => arr.iter().filter_map(parse_location).collect(),
        Value::Object(_) => parse_location(value).into_iter().collect(),
        _ => Vec::new(),
    }
}

fn parse_location(value: &Value) -> Option<LspLocation> {
    if let Some(uri) = value.get("uri").and_then(|v| v.as_str()) {
        let range = value.get("range")?;
        return parse_range(uri, range);
    }
    if let Some(uri) = value.get("targetUri").and_then(|v| v.as_str()) {
        let range = value.get("targetRange")?;
        return parse_range(uri, range);
    }
    None
}

fn parse_range(uri: &str, range: &Value) -> Option<LspLocation> {
    let start = range.get("start")?;
    let line = start.get("line")?.as_u64()? as usize + 1;
    let column = start.get("character")?.as_u64()? as usize + 1;
    let path = uri_to_path(uri)?;
    Some(LspLocation { path, line, column })
}
