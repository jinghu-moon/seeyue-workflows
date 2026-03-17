// src/platform/notify.rs
//
// Windows Toast notification via WinRT (PowerShell subprocess).
// Registers SeeyueMcp.Notification AppUserModelID on first call.
// Supports: basic toast, progress-bar toast.

use std::process::Command;
use std::sync::Once;

const APP_ID:   &str = "SeeyueMcp.Notification";
const APP_NAME: &str = "seeyue-mcp";
const REG_PATH: &str = r"HKCU:\Software\Classes\AppUserModelId\SeeyueMcp.Notification";

static REGISTER: Once = Once::new();

/// Register AppUserModelID in HKCU registry (idempotent, runs once per process).
pub fn ensure_registered() {
    REGISTER.call_once(|| {
        let script = format!(
            r#"New-Item -Path '{reg}' -Force | Out-Null; \
               Set-ItemProperty -Path '{reg}' -Name 'DisplayName' -Value '{name}'"#,
            reg  = REG_PATH,
            name = APP_NAME,
        );
        let _ = Command::new("powershell.exe")
            .args(["-NoProfile", "-Command", &script])
            .output();
    });
}

// ─── Level ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum NotifyLevel {
    Info,
    Warn,
    Milestone,
}

impl NotifyLevel {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "warn" | "warning" => Self::Warn,
            "milestone"        => Self::Milestone,
            _                  => Self::Info,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Info      => "info",
            Self::Warn      => "warn",
            Self::Milestone => "milestone",
        }
    }

    fn scenario(self) -> &'static str {
        match self {
            Self::Milestone => "reminder",
            Self::Warn      => "urgent",
            Self::Info      => "default",
        }
    }

    fn audio(self) -> &'static str {
        match self {
            Self::Milestone => "ms-winsoundevent:Notification.Reminder",
            Self::Warn      => "ms-winsoundevent:Notification.Looping.Alarm",
            Self::Info      => "ms-winsoundevent:Notification.Default",
        }
    }
}

// ─── Progress ────────────────────────────────────────────────────────────────

/// Optional progress bar data for a toast notification.
#[derive(Debug, Clone)]
pub struct ToastProgress {
    /// Progress value 0.0–1.0 (use negative for indeterminate).
    pub value:  f32,
    /// Denominator label shown next to bar (e.g. "100").
    pub max:    Option<String>,
    /// Short label above progress bar (e.g. "Building…").
    pub label:  Option<String>,
    /// Status text below bar (e.g. "42 / 100 nodes").
    pub status: Option<String>,
}

// ─── Result ──────────────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct ToastResult {
    pub notified: bool,
    pub method:   &'static str, // "winrt" | "fallback" | "error"
    pub error:    Option<String>,
}

// ─── send_toast ──────────────────────────────────────────────────────────────

/// Send a basic toast notification. Tries WinRT first, falls back to `msg`.
pub fn send_toast(title: &str, body: &str, level: NotifyLevel) -> ToastResult {
    send_toast_inner(title, body, level, None)
}

/// Send a toast with an optional progress bar.
pub fn send_toast_progress(
    title:    &str,
    body:     &str,
    level:    NotifyLevel,
    progress: ToastProgress,
) -> ToastResult {
    send_toast_inner(title, body, level, Some(progress))
}

fn send_toast_inner(
    title:    &str,
    body:     &str,
    level:    NotifyLevel,
    progress: Option<ToastProgress>,
) -> ToastResult {
    ensure_registered();

    let title_e = xml_escape(title);
    let body_e  = xml_escape(body);

    // Build optional progress XML element
    let progress_xml = match &progress {
        None => String::new(),
        Some(p) => {
            let value_attr = if p.value < 0.0 {
                "indeterminate".to_string()
            } else {
                format!("{:.3}", p.value.clamp(0.0, 1.0))
            };
            let title_attr  = xml_escape(p.label.as_deref().unwrap_or(""));
            let status_attr = xml_escape(p.status.as_deref().unwrap_or(""));
            let max_attr    = xml_escape(p.max.as_deref().unwrap_or(""));
            format!(
                "<progress value=\"{value}\" title=\"{title}\" \
                 status=\"{status}\" valueStringOverride=\"{max}\"/>",
                value  = value_attr,
                title  = title_attr,
                status = status_attr,
                max    = max_attr,
            )
        }
    };

    let xml = format!(
        "<toast scenario=\"{scenario}\"><visual><binding template=\"ToastGeneric\">\
         <text>{title}</text><text>{body}</text>{progress}\
         </binding></visual>\
         <audio src=\"{audio}\"/></toast>",
        scenario = level.scenario(),
        title    = title_e,
        body     = body_e,
        progress = progress_xml,
        audio    = level.audio(),
    );

    let xml_ps = xml.replace('"', "\\`\"");
    let script = format!(
        r#"[Windows.UI.Notifications.ToastNotificationManager, Windows.UI.Notifications, ContentType = WindowsRuntime] | Out-Null;
[Windows.Data.Xml.Dom.XmlDocument, Windows.Data.Xml.Dom.XmlDocument, ContentType = WindowsRuntime] | Out-Null;
$xml = New-Object Windows.Data.Xml.Dom.XmlDocument;
$xml.LoadXml("{xml}");
$toast = [Windows.UI.Notifications.ToastNotification]::new($xml);
[Windows.UI.Notifications.ToastNotificationManager]::CreateToastNotifier("{app_id}").Show($toast);
Write-Output 'sent'"#,
        xml    = xml_ps,
        app_id = APP_ID,
    );

    match Command::new("powershell.exe")
        .args(["-NoProfile", "-Command", &script])
        .output()
    {
        Ok(out) if out.status.success()
            && String::from_utf8_lossy(&out.stdout).trim() == "sent" =>
        {
            ToastResult { notified: true, method: "winrt", error: None }
        }
        Ok(out) => {
            let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
            fallback_msg(&body_e);
            ToastResult { notified: true, method: "fallback", error: Some(err) }
        }
        Err(e) => ToastResult { notified: false, method: "error", error: Some(e.to_string()) },
    }
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn fallback_msg(message: &str) {
    let _ = Command::new("cmd.exe")
        .args(["/C", &format!("msg %USERNAME% /time:8 \"{}\"", message)])
        .output();
}

fn xml_escape(s: &str) -> String {
    s.replace('&',  "&amp;")
     .replace('<',  "&lt;")
     .replace('>',  "&gt;")
     .replace('"',  "&quot;")
     .replace('\'', "&apos;")
}
