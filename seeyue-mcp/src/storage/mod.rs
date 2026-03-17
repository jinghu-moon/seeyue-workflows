// src/storage/mod.rs
//
// Persistent state: backup snapshots, read cache, and checkpoint store.

pub mod backup;
pub mod cache;
pub mod checkpoint;

// Flat re-exports for backward-compatible import paths
#[allow(unused_imports)]
pub use backup::{BackupConfig, BackupManager, BackupTrigger};
#[allow(unused_imports)]
pub use cache::ReadCache;
#[allow(unused_imports)]
pub use checkpoint::CheckpointStore;
