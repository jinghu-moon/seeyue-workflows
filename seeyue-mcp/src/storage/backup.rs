use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use crate::encoding::sha256_hex;
use crate::error::ToolError;

// ─── 备份配置 ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct BackupConfig {
    /// 备份根目录
    pub directory:      PathBuf,
    /// 触发策略
    pub trigger:        BackupTrigger,
    /// 保留天数（自动清理）
    pub retention_days: u32,
    /// 备份目录最大体积 MB（超过则停止备份）
    pub max_size_mb:    u64,
}

impl Default for BackupConfig {
    fn default() -> Self {
        Self {
            directory:      PathBuf::from(".agent-backups"),
            trigger:        BackupTrigger::FirstEdit,
            retention_days: 7,
            max_size_mb:    500,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum BackupTrigger {
    /// session 内每个文件的第一次写入（推荐）
    FirstEdit,
    /// 所有写操作（保守模式）
    AllWrites,
    /// 关闭备份
    None,
}

// ─── 备份元数据 ───────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BackupMeta {
    pub original_path: String,
    pub backup_path:   String,
    pub trigger:       String,
    pub tool:          String,
    pub session_id:    String,
    pub captured_at:   String,
    pub file_size:     u64,
    pub raw_hash:      String,
    pub encoding:      String,
    pub line_ending:   String,
}

#[derive(Debug, Clone)]
pub struct BackupRecord {
    pub backup_path: PathBuf,
    pub meta:        BackupMeta,
}

// ─── 备份管理器 ───────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct BackupManager {
    config:     BackupConfig,
    session_id: String,
    /// session 内已备份的文件（去重用）
    backed_up:  Arc<Mutex<HashSet<PathBuf>>>,
}

impl BackupManager {
    pub fn new(config: BackupConfig, session_id: String) -> Self {
        Self {
            config,
            session_id,
            backed_up: Arc::new(Mutex::new(HashSet::new())),
        }
    }

    /// 执行备份。返回 None 表示本次跳过（已有备份或触发策略不满足）。
    pub fn backup(
        &self,
        file_path:    &Path,
        tool:         &str,
        trigger_hint: BackupTrigger,
        encoding:     &str,
        line_ending:  &str,
    ) -> Result<Option<BackupRecord>, ToolError> {

        if self.config.trigger == BackupTrigger::None {
            return Ok(None);
        }

        // AllWrites 模式每次都备份；FirstEdit 模式每个文件只备份一次
        let key = file_path.to_path_buf();
        if self.config.trigger == BackupTrigger::FirstEdit {
            let already = self.backed_up.lock().unwrap().contains(&key);
            if already {
                return Ok(None);
            }
        }

        // 文件不存在（新建）不备份
        if !file_path.exists() {
            return Ok(None);
        }

        // 磁盘空间检查（粗粒度）
        if let Ok(dir_size) = self.dir_size_mb() {
            if dir_size >= self.config.max_size_mb {
                // 超出限制，跳过但不报错
                return Ok(None);
            }
        }

        // 读取原始内容
        let raw = std::fs::read(file_path).map_err(|e| ToolError::IoError {
            message: e.to_string(),
        })?;

        // 生成备份路径：.agent-backups/{YYYY-MM-DD}/{HH-MM-SS}_{safe_name}.bak
        let now      = Utc::now();
        let date_str = now.format("%Y-%m-%d").to_string();
        let time_str = now.format("%H-%M-%S").to_string();
        let safe_name = file_path
            .to_string_lossy()
            .replace(['/', '\\'], "_")
            .trim_start_matches('_')
            .to_string();

        let backup_dir = self.config.directory.join(&date_str);
        std::fs::create_dir_all(&backup_dir).map_err(|e| ToolError::MkdirFailed {
            path: backup_dir.display().to_string(),
            hint: e.to_string(),
        })?;

        let backup_file = backup_dir.join(format!("{time_str}_{safe_name}.bak"));
        let meta_file   = backup_dir.join(format!("{time_str}_{safe_name}.bak.meta"));

        std::fs::write(&backup_file, &raw).map_err(|e| ToolError::IoError {
            message: e.to_string(),
        })?;

        let meta = BackupMeta {
            original_path: file_path.display().to_string(),
            backup_path:   backup_file.display().to_string(),
            trigger:       format!("{trigger_hint:?}"),
            tool:          tool.to_string(),
            session_id:    self.session_id.clone(),
            captured_at:   now.to_rfc3339(),
            file_size:     raw.len() as u64,
            raw_hash:      sha256_hex(&raw),
            encoding:      encoding.to_string(),
            line_ending:   line_ending.to_string(),
        };

        let meta_json = serde_json::to_string_pretty(&meta).unwrap_or_default();
        std::fs::write(&meta_file, meta_json).ok();

        // 记录已备份
        self.backed_up.lock().unwrap().insert(key);

        Ok(Some(BackupRecord {
            backup_path: backup_file,
            meta,
        }))
    }

    /// 从备份恢复文件
    pub fn restore(&self, backup_path: &Path) -> Result<PathBuf, ToolError> {
        let meta_path = backup_path.with_extension("bak.meta");
        let meta_json = std::fs::read_to_string(&meta_path).map_err(|e| ToolError::IoError {
            message: format!("Cannot read backup meta: {e}"),
        })?;
        let meta: BackupMeta = serde_json::from_str(&meta_json).map_err(|e| ToolError::IoError {
            message: format!("Invalid backup meta JSON: {e}"),
        })?;

        let original = PathBuf::from(&meta.original_path);
        if let Some(parent) = original.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        std::fs::copy(backup_path, &original).map_err(|e| ToolError::IoError {
            message: e.to_string(),
        })?;

        Ok(original)
    }

    /// 清理超过 retention_days 天的备份
    pub fn prune(&self) -> u32 {
        let cutoff = Utc::now()
            - chrono::Duration::days(self.config.retention_days as i64);
        let mut removed = 0u32;

        let dir = &self.config.directory;
        if !dir.exists() { return 0; }

        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                // 目录名是日期 YYYY-MM-DD
                if let Ok(date) = chrono::NaiveDate::parse_from_str(
                    &entry.file_name().to_string_lossy(), "%Y-%m-%d"
                ) {
                    let dir_dt = date.and_hms_opt(0, 0, 0)
                        .and_then(|dt| dt.and_local_timezone(Utc).single());
                    if let Some(dt) = dir_dt {
                        if dt < cutoff
                            && std::fs::remove_dir_all(entry.path()).is_ok()
                        {
                            removed += 1;
                        }
                    }
                }
            }
        }

        removed
    }

    fn dir_size_mb(&self) -> Result<u64, std::io::Error> {
        let mut total = 0u64;
        fn walk(dir: &Path, total: &mut u64) {
            if let Ok(entries) = std::fs::read_dir(dir) {
                for e in entries.flatten() {
                    if let Ok(meta) = e.metadata() {
                        if meta.is_file() {
                            *total += meta.len();
                        } else if meta.is_dir() {
                            walk(&e.path(), total);
                        }
                    }
                }
            }
        }
        walk(&self.config.directory, &mut total);
        Ok(total / 1_048_576)
    }
}
