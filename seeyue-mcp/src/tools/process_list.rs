// src/tools/process_list.rs
//
// process_list: List running processes on Windows (via tasklist /FO CSV).
// Useful for detecting port conflicts, stale server processes, lock holders.

use serde::{Deserialize, Serialize};

use crate::error::ToolError;

// ─── Params / Result ─────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, Default)]
pub struct ProcessListParams {
    /// Filter by process name substring (case-insensitive).
    pub filter_name: Option<String>,
    /// Filter by port number (checks netstat output, Windows only).
    pub filter_port: Option<u16>,
    /// Maximum results to return (default: 50, max: 200).
    pub limit:       Option<usize>,
}

#[derive(Debug, Serialize)]
pub struct ProcessEntry {
    pub name:     String,
    pub pid:      u32,
    pub mem_kb:   Option<u64>,
    pub session:  Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ProcessListResult {
    #[serde(rename = "type")]
    pub kind:      String, // "success" | "empty"
    pub total:     usize,
    pub truncated: bool,
    pub processes: Vec<ProcessEntry>,
    /// Populated when filter_port is set.
    pub port_pids: Vec<u32>,
}

// ─── Implementation ──────────────────────────────────────────────────────────

pub fn run_process_list(
    params: ProcessListParams,
) -> Result<ProcessListResult, ToolError> {
    let limit = params.limit.unwrap_or(50).min(200);

    // Run tasklist /FO CSV /NH
    let output = std::process::Command::new("tasklist")
        .args(["/FO", "CSV", "/NH"])
        .output()
        .map_err(|e| ToolError::IoError {
            message: format!("tasklist failed: {e}"),
        })?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let filter = params.filter_name.as_deref()
        .map(|s| s.to_lowercase());

    let mut all: Vec<ProcessEntry> = stdout
        .lines()
        .filter_map(|line| parse_tasklist_csv(line))
        .filter(|p| {
            filter.as_ref().map_or(true, |f| p.name.to_lowercase().contains(f.as_str()))
        })
        .collect();

    // Port filtering via netstat
    let mut port_pids: Vec<u32> = Vec::new();
    if let Some(port) = params.filter_port {
        port_pids = find_pids_for_port(port);
        if !port_pids.is_empty() {
            all.retain(|p| port_pids.contains(&p.pid));
        }
    }

    let total     = all.len();
    let truncated = total > limit;
    all.truncate(limit);

    Ok(ProcessListResult {
        kind:      if all.is_empty() { "empty" } else { "success" }.into(),
        total,
        truncated,
        processes: all,
        port_pids,
    })
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn parse_tasklist_csv(line: &str) -> Option<ProcessEntry> {
    // Format: "Image Name","PID","Session Name","Session#","Mem Usage"
    let cols: Vec<&str> = line.splitn(5, ',').collect();
    if cols.len() < 5 { return None; }

    let name    = cols[0].trim_matches('"').to_string();
    let pid_str = cols[1].trim_matches('"');
    let session = cols[2].trim_matches('"').to_string();
    let mem_str = cols[4].trim_matches('"')
        .replace(" K", "").replace(',', "").trim().to_string();

    let pid    = pid_str.parse::<u32>().ok()?;
    let mem_kb = mem_str.parse::<u64>().ok();

    if name.is_empty() || pid == 0 { return None; }

    Some(ProcessEntry { name, pid, mem_kb, session: Some(session) })
}

fn find_pids_for_port(port: u16) -> Vec<u32> {
    let output = std::process::Command::new("netstat")
        .args(["-ano", "-p", "TCP"])
        .output()
        .ok();

    let Some(out) = output else { return vec![]; };
    let stdout = String::from_utf8_lossy(&out.stdout);
    let port_str = format!(":{}", port);

    stdout
        .lines()
        .filter(|l| l.contains(&port_str))
        .filter_map(|l| {
            l.split_whitespace().last()?.parse::<u32>().ok()
        })
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect()
}
