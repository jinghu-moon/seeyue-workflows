use rusqlite::{params, Connection};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use crate::error::ToolError;

// ─── Checkpoint 数据库（SQLite，每个 session 一个文件）───────────────────────

#[derive(Clone)]
pub struct CheckpointStore(Arc<Mutex<Connection>>);

impl CheckpointStore {
    pub fn open(session_id: &str, db_dir: &Path) -> Result<Self, ToolError> {
        std::fs::create_dir_all(db_dir).map_err(|e| ToolError::IoError {
            message: e.to_string(),
        })?;

        let db_path = db_dir.join(format!("{session_id}.db"));
        let conn    = Connection::open(&db_path).map_err(|e| ToolError::IoError {
            message: format!("Cannot open checkpoint DB: {e}"),
        })?;

        // WAL 模式（更快的并发写入）
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL;")
            .ok();

        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS snapshots (
                id           INTEGER PRIMARY KEY AUTOINCREMENT,
                file_path    TEXT    NOT NULL,
                content      BLOB,              -- NULL = 文件快照时不存在（新文件）
                tool_call_id TEXT    NOT NULL,
                tool_name    TEXT    NOT NULL,
                captured_at  INTEGER NOT NULL    -- unix ms
             );
             CREATE INDEX IF NOT EXISTS idx_captured
               ON snapshots(captured_at DESC);",
        ).map_err(|e| ToolError::IoError {
            message: format!("Cannot init checkpoint DB: {e}"),
        })?;

        Ok(Self(Arc::new(Mutex::new(conn))))
    }

    /// 写前快照（每次 Edit / Write / MultiEdit 前调用）
    pub fn capture(
        &self,
        file_path:    &Path,
        tool_call_id: &str,
        tool_name:    &str,
    ) -> Result<(), ToolError> {
        let content = std::fs::read(file_path).ok(); // None = 文件不存在（新建场景）
        let now_ms  = chrono::Utc::now().timestamp_millis();

        let conn = self.0.lock().unwrap();
        conn.execute(
            "INSERT INTO snapshots(file_path, content, tool_call_id, tool_name, captured_at)
             VALUES(?1, ?2, ?3, ?4, ?5)",
            params![
                file_path.display().to_string(),
                content,
                tool_call_id,
                tool_name,
                now_ms,
            ],
        ).map_err(|e| ToolError::IoError { message: e.to_string() })?;

        Ok(())
    }

    /// /rewind N：撤销最近 N 步写入操作
    pub fn rewind(&self, steps: usize) -> Result<Vec<PathBuf>, ToolError> {
        let conn = self.0.lock().unwrap();

        // 获取最近 N 个快照（按时间倒序）
        let mut stmt = conn.prepare(
            "SELECT id, file_path, content
             FROM snapshots
             ORDER BY captured_at DESC
             LIMIT ?1",
        ).map_err(|e| ToolError::IoError { message: e.to_string() })?;

        let rows: Vec<(i64, String, Option<Vec<u8>>)> = stmt.query_map(
            params![steps as i64],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .map_err(|e| ToolError::IoError { message: e.to_string() })?
        .filter_map(|r| r.ok())
        .collect();

        let mut restored = Vec::new();

        for (id, path_str, content) in &rows {
            let path = PathBuf::from(path_str);

            match content {
                None => {
                    // 文件原本不存在 → 删除
                    let _ = std::fs::remove_file(&path);
                }
                Some(data) => {
                    // 恢复文件内容
                    if let Some(parent) = path.parent() {
                        std::fs::create_dir_all(parent).ok();
                    }
                    std::fs::write(&path, data).map_err(|e| ToolError::IoError {
                        message: e.to_string(),
                    })?;
                }
            }

            restored.push(path);

            // 删除已恢复的快照（防止重复 rewind）
            conn.execute("DELETE FROM snapshots WHERE id = ?1", params![id]).ok();
        }

        Ok(restored)
    }

    /// Read the snapshot content for a specific file by tool_call_id.
    /// Returns the stored bytes, or an error if not found.
    pub fn read_snapshot(&self, tool_call_id: &str, file_path: &std::path::Path) -> Result<Vec<u8>, ToolError> {
        let conn = self.0.lock().unwrap();
        let path_str = file_path.display().to_string();
        let result: Option<Option<Vec<u8>>> = conn.query_row(
            "SELECT content FROM snapshots WHERE tool_call_id = ?1 AND file_path = ?2 LIMIT 1",
            rusqlite::params![tool_call_id, path_str],
            |row| row.get(0),
        ).ok();
        match result {
            Some(Some(bytes)) => Ok(bytes),
            Some(None) => Ok(Vec::new()), // file did not exist at snapshot time
            None => Err(ToolError::FileNotFound {
                file_path: path_str,
                hint: format!("No snapshot found for tool_call_id={}", tool_call_id),
            }),
        }
    }

    /// 清理全部快照（SessionEnd 时调用，后台异步执行）
    pub fn cleanup(&self) {
        let conn = self.0.lock().unwrap();
        conn.execute("DELETE FROM snapshots", []).ok();
    }

    /// 查看快照列表（用于 /checkpoints 命令）
    pub fn list(&self) -> Vec<SnapshotInfo> {
        let conn = self.0.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT file_path, tool_name, captured_at
             FROM snapshots
             ORDER BY captured_at DESC
             LIMIT 50",
        ).unwrap();

        stmt.query_map([], |row| {
            Ok(SnapshotInfo {
                file_path:  row.get(0)?,
                tool_name:  row.get(1)?,
                captured_at_ms: row.get(2)?,
            })
        })
        .unwrap()
        .filter_map(|r| r.ok())
        .collect()
    }
}

#[derive(Debug)]
pub struct SnapshotInfo {
    pub file_path:      String,
    pub tool_name:      String,
    pub captured_at_ms: i64,
}
