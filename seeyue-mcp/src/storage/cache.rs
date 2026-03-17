use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use crate::encoding::{EncodingInfo, LineEnding};

// ─── 读取缓存条目 ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct CacheEntry {
    pub file_path:        PathBuf,
    /// sha256(原始字节) —— Edit 校验的主 hash
    pub raw_hash:         String,
    /// sha256(LF 规范化后) —— CRLF 容错校验
    pub norm_hash:        String,
    pub encoding_name:    String,
    pub line_ending:      LineEnding,
    pub has_non_ascii:    bool,
    pub read_at:          DateTime<Utc>,
    /// session 内此文件的读取次数（用于 "首次 Edit" 备份触发）
    pub read_count:       u32,
    /// session 内此文件的 Edit 次数
    pub edit_count:       u32,
}

// ─── 线程安全的全局读取缓存 ───────────────────────────────────────────────────

#[derive(Clone, Default)]
pub struct ReadCache(Arc<Mutex<HashMap<PathBuf, CacheEntry>>>);

impl ReadCache {
    pub fn new() -> Self {
        Self(Arc::new(Mutex::new(HashMap::new())))
    }

    /// 存入 Read 结果
    pub fn insert(
        &self,
        path:     PathBuf,
        raw_hash: String,
        norm_hash:String,
        enc:      &EncodingInfo,
    ) {
        let mut map = self.0.lock().unwrap();
        let entry   = map.entry(path.clone()).or_insert_with(|| CacheEntry {
            file_path:     path.clone(),
            raw_hash:      raw_hash.clone(),
            norm_hash:     norm_hash.clone(),
            encoding_name: enc.name.clone(),
            line_ending:   enc.line_ending.clone(),
            has_non_ascii: enc.has_non_ascii,
            read_at:       Utc::now(),
            read_count:    0,
            edit_count:    0,
        });

        // 更新 hash（文件可能在两次 Read 之间被修改）
        entry.raw_hash     = raw_hash;
        entry.norm_hash    = norm_hash;
        entry.encoding_name= enc.name.clone();
        entry.line_ending  = enc.line_ending.clone();
        entry.has_non_ascii= enc.has_non_ascii;
        entry.read_at      = Utc::now();
        entry.read_count   += 1;
    }

    /// 查询（返回 clone 副本）
    pub fn get(&self, path: &PathBuf) -> Option<CacheEntry> {
        self.0.lock().unwrap().get(path).cloned()
    }

    /// Edit 完成后更新 hash（避免下次 Edit 误报 FILE_MODIFIED）
    pub fn update_after_edit(&self, path: &PathBuf, new_raw_hash: String, new_norm_hash: String) {
        let mut map = self.0.lock().unwrap();
        if let Some(entry) = map.get_mut(path) {
            entry.raw_hash  = new_raw_hash;
            entry.norm_hash = new_norm_hash;
            entry.read_at   = Utc::now();
            entry.edit_count += 1;
        }
    }

    /// 是否已读过（Write 需要这个判断）
    pub fn has_been_read(&self, path: &PathBuf) -> bool {
        self.0.lock().unwrap().contains_key(path)
    }

    /// session 内某文件的 edit 次数（备份触发需要）
    pub fn edit_count(&self, path: &PathBuf) -> u32 {
        self.0.lock().unwrap()
            .get(path)
            .map(|e| e.edit_count)
            .unwrap_or(0)
    }
}
