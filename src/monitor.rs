use core_graphics::display::{CGDisplay, CGMainDisplayID};
use std::ffi::c_void;

pub type CGDirectDisplayID = u32;
pub type CGDisplayChangeSummaryFlags = u32;

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    pub fn CGDisplayRegisterReconfigurationCallback(
        callback: Option<
            unsafe extern "C" fn(CGDirectDisplayID, CGDisplayChangeSummaryFlags, *mut c_void),
        >,
        user_info: *mut c_void,
    ) -> i32;
}

pub fn get_fingerprint() -> String {
    // Build a stable fingerprint from hardware identity + pixel dimensions.
    // Include main display and origin to distinguish arrangement states across display switches.
    let main_id = unsafe { CGMainDisplayID() };

    let mut parts: Vec<String> = match CGDisplay::active_displays() {
        Ok(ids) => ids
            .into_iter()
            .map(|id| {
                let d = CGDisplay::new(id);
                let serial = d.serial_number();
                let vendor = d.vendor_number();
                let model = d.model_number();
                let b = d.bounds();
                format!(
                    "id={}:vendor={}:model={}:serial={}:{}x{}@{},{}",
                    id,
                    vendor,
                    model,
                    serial,
                    d.pixels_wide(),
                    d.pixels_high(),
                    b.origin.x.round() as i64,
                    b.origin.y.round() as i64
                )
            })
            .collect(),
        Err(err) => {
            eprintln!("[monitor] failed to query active displays: {}", err);
            Vec::new()
        }
    };

    parts.sort();
    format!(
        "main={}|displays={}|{}",
        main_id,
        parts.len(),
        parts.join(";")
    )
}

/// Return a human-readable summary of the current display configuration.
/// e.g. "1 display: 2560×1600" or "2 displays: 1440×900 + 2560×1440"
pub fn get_display_summary() -> String {
    match CGDisplay::active_displays() {
        Ok(ids) => {
            let count = ids.len();
            let descs: Vec<String> = ids
                .into_iter()
                .map(|id| {
                    let d = CGDisplay::new(id);
                    format!("{}×{}", d.pixels_wide(), d.pixels_high())
                })
                .collect();

            let label = if count == 1 { "display" } else { "displays" };
            format!("{} {}: {}", count, label, descs.join(" + "))
        }
        Err(_) => "Unknown display configuration".to_string(),
    }
}

/// Parse a fingerprint string and return a human-readable description.
/// Extracts resolution info from the fingerprint parts.
pub fn fingerprint_to_summary(fingerprint: &str) -> String {
    // fingerprint format: "main=<id>|displays=<N>|part1;part2;..."
    let parts_section = fingerprint.splitn(3, '|').collect::<Vec<&str>>();

    let display_count = parts_section
        .get(1)
        .and_then(|s| s.strip_prefix("displays="))
        .and_then(|n| n.parse::<usize>().ok())
        .unwrap_or(0);

    if display_count == 0 {
        return "Unknown".to_string();
    }

    let resolutions: Vec<String> = parts_section
        .get(2)
        .map(|s| {
            s.split(';')
                .filter_map(|part| {
                    // Each part: "id=...:vendor=...:model=...:serial=...:WxH@X,Y"
                    let after_serial = part.rsplit(':').next()?;
                    let res = after_serial.split('@').next()?;
                    // Convert 'x' to '×' for display
                    Some(res.replace('x', "×"))
                })
                .collect()
        })
        .unwrap_or_default();

    let label = if display_count == 1 {
        "display"
    } else {
        "displays"
    };
    format!("{} {}: {}", display_count, label, resolutions.join(" + "))
}
