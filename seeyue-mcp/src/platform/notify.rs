// src/platform/notify.rs
//
// Windows Toast notification via WinRT (PowerShell subprocess).
// Registers SeeyueMcp.Notification AppUserModelID on first call.

use std::process::Command;
use std::sync::Once;

const APP_ID:       &str = "SeeyueMcp.Notification";
const APP_NAME:     &str = "seeyue-mcp";
const REG_PATH:     &str = r"HKCU:\Software\Classes\AppUserModelId\SeeyueMcp.Notification";

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

    /// WinRT scenario attribute for the toast element.
    fn scenario(self) -> &'static str {
        match self {
            Self::Milestone => "reminder",
            Self::Warn      => "urgent",
            Self::Info      => "default",
        }
    }

    /// Audio src for the toast.
    fn audio(self) -> &'static str {
        match self {
            Self::Milestone => "ms-winsoundevent:Notification.Reminder",
            Self::Warn      => "ms-winsoundevent:Notification.Looping.Alarm",
            Self::Info      => "ms-winsoundevent:Notification.Default",
        }
    }
}

#[derive(Debug)]
pub struct ToastResult {
    pub notified: bool,
    pub method:   &'static str, // "winrt" | "fallback" | "error"
    pub error:    Option<String>,
}

/// Send a toast notification. Tries WinRT first, falls back to `msg`.
pub fn send_toast(title: &str, body: &str, level: NotifyLevel) -> ToastResult {
    ensure_registered();

    // Escape XML special chars
    let title = xml_escape(title);
    let body  = xml_escape(body);

    let xml = format!(
        "<toast scenario=\"{scenario}\"><visual><binding template=\"ToastGeneric\">\
         <text>{title}</text><text>{body}</text>\
         </binding></visual>\
         <audio src=\"{audio}\"/></toast>",
        scenario = level.scenario(),
        title    = title,
        body     = body,
        audio    = level.audio(),
    );

    // Escape for PowerShell double-quoted string
    let xml_ps = xml.replace('"', "\\`\"");
    let app_id = APP_ID;

    let script = format!(
        r#"[Windows.UI.Notifications.ToastNotificationManager, Windows.UI.Notifications, ContentType = WindowsRuntime] | Out-Null;
[Windows.Data.Xml.Dom.XmlDocument, Windows.Data.Xml.Dom.XmlDocument, ContentType = WindowsRuntime] | Out-Null;
$xml = New-Object Windows.Data.Xml.Dom.XmlDocument;
$xml.LoadXml("{xml}");
$toast = [Windows.UI.Notifications.ToastNotification]::new($xml);
[Windows.UI.Notifications.ToastNotificationManager]::CreateToastNotifier("{app_id}").Show($toast);
Write-Output 'sent'"#,
        xml    = xml_ps,
        app_id = app_id,
    );

    match Command::new("powershell.exe")
        .args(["-NoProfile", "-Command", &script])
        .output()
    {
        Ok(out) if out.status.success() && String::from_utf8_lossy(&out.stdout).trim() == "sent" => {
            ToastResult { notified: true, method: "winrt", error: None }
        }
        Ok(out) => {
            let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
            // Fallback: msg command
            fallback_msg(body.as_ref());
            ToastResult { notified: true, method: "fallback", error: Some(err) }
        }
        Err(e) => ToastResult { notified: false, method: "error", error: Some(e.to_string()) },
    }
}

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
