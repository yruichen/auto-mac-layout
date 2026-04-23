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
