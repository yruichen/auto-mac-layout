use serde::{Deserialize, Serialize};
use std::process::Command;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Icon {
    pub name: String,
    pub x: f64,
    pub y: f64,
}

pub fn fetch_current_layout() -> Vec<Icon> {
    let script = r#"
        set output to "{\"icons\":["
        set errorOutput to ""
        set firstItem to true
        set firstError to true
        set totalCount to 0

        on json_escape(t)
            set escaped to ""
            repeat with i from 1 to length of t
                set c to character i of t
                if c is "\\" then
                    set escaped to escaped & "\\\\"
                else if c is "\"" then
                    set escaped to escaped & "\\\""
                else if c is return then
                    set escaped to escaped & "\\r"
                else if c is linefeed then
                    set escaped to escaped & "\\n"
                else if c is tab then
                    set escaped to escaped & "\\t"
                else
                    set escaped to escaped & c
                end if
            end repeat
            return escaped
        end json_escape

        tell application "Finder"
            set namesList to name of every item of desktop
            set posList to desktop position of every item of desktop
            set totalCount to count of namesList

            repeat with idx from 1 to totalCount
                try
                    set nm to my json_escape(item idx of namesList as text)
                    set pos to item idx of posList
                    set px to item 1 of pos
                    set py to item 2 of pos

                    if firstItem then
                        set firstItem to false
                    else
                        set output to output & ","
                    end if

                    set output to output & "{\"name\":\"" & nm & "\",\"x\":" & px & ",\"y\":" & py & "}"
                on error errMsg number errNum
                    set escapedErr to my json_escape(errMsg as text)
                    if firstError then
                        set firstError to false
                    else
                        set errorOutput to errorOutput & ","
                    end if
                    set errorOutput to errorOutput & "{\"index\":" & idx & ",\"code\":" & errNum & ",\"message\":\"" & escapedErr & "\"}"
                end try
            end repeat
        end tell

        set output to output & "],\"errors\":[" & errorOutput & "],\"total\":" & totalCount & "}"
        return output
    "#;

    let out = match Command::new("osascript").arg("-e").arg(script).output() {
        Ok(o) => o,
        Err(err) => {
            eprintln!("[layout] failed to execute osascript: {err}");
            return vec![];
        }
    };

    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        eprintln!(
            "[layout] fetch_current_layout failed (check Finder Automation/Accessibility permissions): {}",
            stderr.trim()
        );
        return vec![];
    }

    #[derive(Deserialize)]
    struct FetchError {
        index: usize,
        code: i64,
        message: String,
    }

    #[derive(Deserialize)]
    struct FetchPayload {
        icons: Vec<Icon>,
        errors: Vec<FetchError>,
    }

    let stdout = String::from_utf8_lossy(&out.stdout);
    match serde_json::from_str::<FetchPayload>(&stdout) {
        Ok(payload) => {
            for err in &payload.errors {
                eprintln!(
                    "[layout] fetch item failed index={} code={} message={}",
                    err.index, err.code, err.message
                );
            }

            payload.icons
        }
        Err(err) => {
            // Backward-compatible fallback for older array-only output formats.
            match serde_json::from_str::<Vec<Icon>>(&stdout) {
                Ok(v) => v,
                Err(_) => {
                    eprintln!("[layout] failed to parse AppleScript JSON output: {err}");
                    vec![]
                }
            }
        }
    }
}

pub fn apply_layout(icons: &[Icon]) {
    if icons.is_empty() {
        return;
    }

    // Apply all icon moves in one script call to reduce visible one-by-one stepping.
    let script = r#"
        on run argv
            set argc to count of argv
            if (argc mod 3) is not 0 then
                error "invalid argv length for icon triples"
            end if

            set failedCount to 0
            tell application "Finder"
                repeat with i from 1 to argc by 3
                    set itemName to item i of argv
                    set px to item (i + 1) of argv as real
                    set py to item (i + 2) of argv as real
                    try
                        set desktop position of item itemName of desktop to {px, py}
                    on error
                        set failedCount to failedCount + 1
                    end try
                end repeat

                update every item of desktop
            end tell

            return failedCount as text
        end run
    "#;

    let mut cmd = Command::new("osascript");
    cmd.arg("-e").arg(script);
    for icon in icons {
        cmd.arg(&icon.name)
            .arg(icon.x.to_string())
            .arg(icon.y.to_string());
    }

    match cmd.output() {
        Ok(out) if out.status.success() => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            let failed = stdout.trim().parse::<usize>().unwrap_or(0);
            if failed > 0 {
                eprintln!("[layout] apply_layout had {} per-item failures", failed);
            }
        }
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr);
            eprintln!("[layout] batch apply failed: {}", stderr.trim());
        }
        Err(err) => {
            eprintln!("[layout] failed to execute batch apply osascript: {err}");
        }
    }
}
