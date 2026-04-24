use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};

static ENABLED: AtomicBool = AtomicBool::new(true);

pub fn set_enabled(enabled: bool) {
    ENABLED.store(enabled, Ordering::SeqCst);
}

pub fn is_enabled() -> bool {
    ENABLED.load(Ordering::SeqCst)
}

/// Send a macOS native notification via osascript.
/// This is non-blocking — it spawns the process and returns immediately.
pub fn notify(title: &str, message: &str) {
    if !is_enabled() {
        return;
    }

    // Escape double-quotes for AppleScript string literals.
    let safe_title = title.replace('\\', "\\\\").replace('"', "\\\"");
    let safe_message = message.replace('\\', "\\\\").replace('"', "\\\"");

    let script = format!(
        "display notification \"{}\" with title \"{}\"",
        safe_message, safe_title
    );

    if let Err(err) = Command::new("osascript")
        .arg("-e")
        .arg(&script)
        .spawn()
    {
        eprintln!("[notification] failed to send notification: {err}");
    }
}
