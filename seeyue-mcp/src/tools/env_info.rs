use serde::Serialize;
use std::path::{Component, Path, PathBuf};

// ─── 响应 ───────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct EnvInfoResult {
    #[serde(rename = "type")]
    pub kind:                   String, // "success"
    pub os:                     String,
    pub arch:                   String,
    pub workspace:              String,
    pub codepage:               u32,
    pub codepage_name:          String,
    pub line_ending:            String,
    pub volume_kind:            String,
    pub disk_free_mb:           u64,
    pub rust_analyzer_available: bool,
    pub git_available:          bool,
    pub agent_editor_version:   String,
}

// ─── 工具主逻辑 ───────────────────────────────────────────────────────────────

pub fn run_env_info(workspace: &Path) -> EnvInfoResult {
    let codepage = get_codepage();
    let volume_kind = get_volume_kind(workspace).unwrap_or_else(|| "Unknown".into());
    let disk_free_mb = get_disk_free_mb(workspace).unwrap_or(0);

    EnvInfoResult {
        kind:                    "success".into(),
        os:                      get_os_string(),
        arch:                    std::env::consts::ARCH.to_string(),
        workspace:               workspace.to_string_lossy().to_string(),
        codepage,
        codepage_name:           codepage_name(codepage),
        line_ending:             if cfg!(windows) { "CRLF".into() } else { "LF".into() },
        volume_kind,
        disk_free_mb,
        rust_analyzer_available: which::which("rust-analyzer").is_ok(),
        git_available:           which::which("git").is_ok(),
        agent_editor_version:    env!("CARGO_PKG_VERSION").to_string(),
    }
}

// ─── 辅助 ────────────────────────────────────────────────────────────────────

fn codepage_name(codepage: u32) -> String {
    match codepage {
        936   => "GBK".into(),
        932   => "Shift-JIS".into(),
        65001 => "UTF-8".into(),
        other => format!("CP{other}"),
    }
}

#[cfg(windows)]
fn get_codepage() -> u32 {
    use windows_sys::Win32::Globalization::GetACP;
    unsafe { GetACP() }
}

#[cfg(not(windows))]
fn get_codepage() -> u32 {
    65001
}

#[cfg(windows)]
fn get_os_string() -> String {
    if let Some(v) = windows_version() {
        return format!("Windows {}.{} ({})", v.0, v.1, v.2);
    }
    "Windows".into()
}

#[cfg(not(windows))]
fn get_os_string() -> String {
    std::env::consts::OS.to_string()
}

#[cfg(windows)]
fn windows_version() -> Option<(u32, u32, u32)> {
    use windows_sys::Win32::System::SystemInformation::{GetVersionExW, OSVERSIONINFOEXW};

    let mut info: OSVERSIONINFOEXW = unsafe { std::mem::zeroed() };
    info.dwOSVersionInfoSize = std::mem::size_of::<OSVERSIONINFOEXW>() as u32;

    let ok = unsafe { GetVersionExW(&mut info as *mut _ as *mut _) };
    if ok == 0 { return None; }
    Some((info.dwMajorVersion, info.dwMinorVersion, info.dwBuildNumber))
}

#[cfg(windows)]
fn get_volume_kind(path: &Path) -> Option<String> {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::GetVolumeInformationW;

    let root = volume_root(path)?;
    let root_w: Vec<u16> = OsStr::new(&root).encode_wide().chain(Some(0)).collect();

    let mut fs_name_buf = vec![0u16; 64];
    let mut serial: u32 = 0;
    let mut max_comp_len: u32 = 0;
    let mut flags: u32 = 0;

    let ok = unsafe {
        GetVolumeInformationW(
            root_w.as_ptr(),
            std::ptr::null_mut(),
            0,
            &mut serial,
            &mut max_comp_len,
            &mut flags,
            fs_name_buf.as_mut_ptr(),
            fs_name_buf.len() as u32,
        )
    };
    if ok == 0 {
        return None;
    }

    let end = fs_name_buf.iter().position(|&c| c == 0).unwrap_or(fs_name_buf.len());
    Some(String::from_utf16_lossy(&fs_name_buf[..end]))
}

#[cfg(not(windows))]
fn get_volume_kind(_path: &Path) -> Option<String> {
    None
}

#[cfg(windows)]
fn get_disk_free_mb(path: &Path) -> Option<u64> {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;

    let root = volume_root(path)?;
    let root_w: Vec<u16> = OsStr::new(&root).encode_wide().chain(Some(0)).collect();

    let mut free_bytes: u64 = 0;
    let mut total_bytes: u64 = 0;
    let mut total_free: u64 = 0;

    let ok = unsafe {
        GetDiskFreeSpaceExW(
            root_w.as_ptr(),
            &mut free_bytes as *mut u64,
            &mut total_bytes as *mut u64,
            &mut total_free as *mut u64,
        )
    };
    if ok == 0 {
        return None;
    }

    Some(free_bytes / 1024 / 1024)
}

#[cfg(not(windows))]
fn get_disk_free_mb(_path: &Path) -> Option<u64> {
    None
}

#[cfg(windows)]
fn volume_root(path: &Path) -> Option<String> {
    let canonical = path.canonicalize().unwrap_or_else(|_| PathBuf::from(path));
    let mut components = canonical.components();

    let mut root = PathBuf::new();
    if let Some(prefix) = components.next() {
        root.push(prefix.as_os_str());
    }
    if let Some(Component::RootDir) = components.next() {
        root.push(Component::RootDir.as_os_str());
    }

    let mut root_str = root.to_string_lossy().to_string();
    if !root_str.ends_with('\\') {
        root_str.push('\\');
    }
    Some(root_str)
}

#[cfg(not(windows))]
fn volume_root(_path: &Path) -> Option<String> {
    None
}
