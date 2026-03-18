// tests/sandbox/toast_test.rs
// Standalone test: verify Windows toast notification via PowerShell subprocess
fn send_toast(title: &str, message: &str) -> Result<(), String> {
    let ps_script = format!(
        r#"
[Windows.UI.Notifications.ToastNotificationManager, Windows.UI.Notifications, ContentType = WindowsRuntime] | Out-Null
[Windows.Data.Xml.Dom.XmlDocument, Windows.Data.Xml.Dom.XmlDocument, ContentType = WindowsRuntime] | Out-Null
$xml = New-Object Windows.Data.Xml.Dom.XmlDocument
$xml.LoadXml('<toast><visual><binding template="ToastGeneric"><text>{title}</text><text>{message}</text></binding></visual></toast>')
$toast = [Windows.UI.Notifications.ToastNotification]::new($xml)
[Windows.UI.Notifications.ToastNotificationManager]::CreateToastNotifier('seeyue-mcp').Show($toast)
Write-Output 'sent'
"#,
        title = title,
        message = message,
    );

    let output = std::process::Command::new("powershell.exe")
        .args(["-NoProfile", "-Command", &ps_script])
        .output()
        .map_err(|e| format!("spawn: {e}"))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    if output.status.success() && stdout.trim() == "sent" {
        Ok(())
    } else {
        Err(format!("ps exit={} stderr={}", output.status, stderr.trim()))
    }
}

fn main() {
    match send_toast("seeyue-mcp", "Rust → PowerShell → WinRT toast 验证成功") {
        Ok(()) => println!("Toast sent OK"),
        Err(e) => eprintln!("Toast failed: {}", e),
    }
}
