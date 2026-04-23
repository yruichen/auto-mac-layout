mod config;
mod layout;
mod monitor;

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::Duration;
use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu};
use tray_icon::{Icon, TrayIconBuilder, TrayIconEvent};

const DEFAULT_APPLY_DELAY_MS: u64 = 1000;
const DESKTOP_WATCH_POLL_SECS: u64 = 2;

#[cfg(target_os = "macos")]
fn hide_from_dock() {
    use cocoa::appkit::{NSApp, NSApplication, NSApplicationActivationPolicyAccessory};
    use cocoa::base::nil;

    unsafe {
        // Ensure NSApplication exists before setting activation policy.
        let _ = NSApplication::sharedApplication(nil);
        let app = NSApp();
        if app == nil {
            eprintln!("[main] failed to get NSApplication instance for Dock hiding");
            return;
        }

        let _ = app.setActivationPolicy_(NSApplicationActivationPolicyAccessory);
    }
}

#[cfg(not(target_os = "macos"))]
fn hide_from_dock() {}

struct SyncScheduler {
    seq: AtomicU64,
    debounce_ms: AtomicU64,
    storage_lock: Mutex<()>,
}

impl SyncScheduler {
    fn new() -> Self {
        Self {
            seq: AtomicU64::new(0),
            debounce_ms: AtomicU64::new(DEFAULT_APPLY_DELAY_MS),
            storage_lock: Mutex::new(()),
        }
    }

    fn schedule(self: &Arc<Self>) {
        let seq = self.seq.fetch_add(1, Ordering::SeqCst) + 1;
        let delay_ms = self.debounce_ms.load(Ordering::SeqCst);

        let me = Arc::clone(self);
        thread::spawn(move || {
            thread::sleep(Duration::from_millis(delay_ms));

            if me.seq.load(Ordering::SeqCst) != seq {
                return;
            }

            me.run(false);
        });
    }

    fn run_force_save_now(self: &Arc<Self>) {
        let me = Arc::clone(self);
        thread::spawn(move || {
            me.run(true);
        });
    }

    fn set_debounce_ms(&self, delay_ms: u64) {
        self.debounce_ms.store(delay_ms, Ordering::SeqCst);
    }

    fn run(&self, force_save: bool) {
        let _guard = match self.storage_lock.lock() {
            Ok(g) => g,
            Err(err) => {
                eprintln!("[main] storage lock poisoned: {err}");
                return;
            }
        };

        let fp = monitor::get_fingerprint();
        let path = config::get_config_path();
        let mut storage = read_storage(&path);

        if force_save {
            let current = layout::fetch_current_layout();
            if !current.is_empty() {
                storage.insert(fp, current);
                write_storage_atomic(&path, &storage);
            }
            return;
        }

        if let Some(saved) = storage.get(&fp) {
            layout::apply_layout(saved);
            return;
        }

        let current = layout::fetch_current_layout();
        if !current.is_empty() {
            storage.insert(fp, current);
            write_storage_atomic(&path, &storage);
        }
    }
}

fn read_storage(path: &Path) -> HashMap<String, Vec<layout::Icon>> {
    match fs::read_to_string(path) {
        Ok(content) => match serde_json::from_str::<HashMap<String, Vec<layout::Icon>>>(&content) {
            Ok(map) => map,
            Err(err) => {
                eprintln!("[main] invalid layouts.json, starting empty map: {err}");
                HashMap::new()
            }
        },
        Err(_err) => HashMap::new(),
    }
}

fn write_storage_atomic(path: &Path, storage: &HashMap<String, Vec<layout::Icon>>) {
    let json = match serde_json::to_string_pretty(storage) {
        Ok(j) => j,
        Err(err) => {
            eprintln!("[main] failed to serialize layouts: {err}");
            return;
        }
    };

    let mut tmp = PathBuf::from(path);
    let ext = match path.extension().and_then(|e| e.to_str()) {
        Some(e) => format!("{e}.tmp"),
        None => "tmp".to_string(),
    };
    tmp.set_extension(ext);

    if let Err(err) = fs::write(&tmp, json) {
        eprintln!("[main] failed to write temp layouts file: {err}");
        return;
    }

    if let Err(err) = fs::rename(&tmp, path) {
        eprintln!("[main] failed to replace layouts file atomically: {err}");
        let _ = fs::remove_file(&tmp);
    }
}

fn scheduler() -> &'static Arc<SyncScheduler> {
    static SCHEDULER: OnceLock<Arc<SyncScheduler>> = OnceLock::new();
    SCHEDULER.get_or_init(|| Arc::new(SyncScheduler::new()))
}

fn run_sync(force_save: bool) {
    if force_save {
        scheduler().run_force_save_now();
    } else {
        scheduler().schedule();
    }
}

fn set_apply_delay_ms(delay_ms: u64) {
    scheduler().set_debounce_ms(delay_ms);
}

fn desktop_items_signature() -> Option<String> {
    let desktop = dirs::desktop_dir()?;
    let mut names: Vec<String> = fs::read_dir(desktop)
        .ok()?
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| entry.file_name().into_string().ok())
        .collect();
    names.sort();
    Some(names.join("\n"))
}

fn start_desktop_change_watcher() {
    thread::spawn(|| {
        let mut last_sig = desktop_items_signature();

        loop {
            thread::sleep(Duration::from_secs(DESKTOP_WATCH_POLL_SECS));
            let sig = desktop_items_signature();

            if sig != last_sig {
                last_sig = sig;
                run_sync(true);
            }
        }
    });
}

fn open_config_file() {
    let path = config::get_config_path();

    if !path.exists() {
        if let Err(err) = fs::write(&path, "{}\n") {
            eprintln!(
                "[main] failed to initialize config file {}: {}",
                path.display(),
                err
            );
            return;
        }
    }

    match Command::new("open").arg(&path).status() {
        Ok(status) if status.success() => {}
        Ok(status) => {
            eprintln!(
                "[main] failed to open config file {} (status={})",
                path.display(),
                status
            );
        }
        Err(err) => {
            eprintln!(
                "[main] failed to run 'open' for {}: {}",
                path.display(),
                err
            );
        }
    }
}

fn load_tray_icon() -> Option<Icon> {
    let logo_path = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/logo.png");

    let img = match image::open(logo_path) {
        Ok(i) => i.into_rgba8(),
        Err(err) => {
            eprintln!("[main] failed to load logo.png: {err}");
            return None;
        }
    };

    let (width, height) = img.dimensions();
    let rgba = img.into_raw();

    match Icon::from_rgba(rgba, width, height) {
        Ok(icon) => Some(icon),
        Err(err) => {
            eprintln!("[main] failed to create icon from logo.png: {err}");
            None
        }
    }
}

unsafe extern "C" fn display_callback(
    _id: monitor::CGDirectDisplayID,
    flags: monitor::CGDisplayChangeSummaryFlags,
    _info: *mut std::ffi::c_void,
) {
    if flags & 1 << 0 == 0 {
        run_sync(false);
    }
}

fn main() {
    let event_loop = EventLoopBuilder::new().build();
    hide_from_dock();
    let menu_channel = MenuEvent::receiver();

    let menu = Menu::new();
    let save_item = MenuItem::new("Save Current Layout", true, None);
    let open_config_item = MenuItem::new("Open Config File", true, None);
    let delay_submenu = Submenu::new("Switch Apply Delay", true);
    let delay_0_item = CheckMenuItem::new("0ms (Fastest)", true, false, None);
    let delay_300_item = CheckMenuItem::new("300ms", true, false, None);
    let delay_1000_item = CheckMenuItem::new("1000ms", true, true, None);
    let delay_2000_item = CheckMenuItem::new("2000ms", true, false, None);
    let delay_3000_item = CheckMenuItem::new("3000ms", true, false, None);
    let auto_start_item =
        CheckMenuItem::new("Launch at Login", true, config::is_auto_start(), None);
    let quit_item = MenuItem::new("Quit", true, None);

    let _ = delay_submenu.append_items(&[
        &delay_0_item,
        &delay_300_item,
        &delay_1000_item,
        &delay_2000_item,
        &delay_3000_item,
    ]);

    let _ = menu.append_items(&[
        &save_item,
        &open_config_item,
        &delay_submenu,
        &PredefinedMenuItem::separator(),
        &auto_start_item,
        &PredefinedMenuItem::separator(),
        &quit_item,
    ]);

    let mut tray_builder = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip("Auto Mac Layout")
        .with_icon_as_template(true);

    if let Some(icon) = load_tray_icon() {
        tray_builder = tray_builder.with_icon(icon);
    }

    let _tray_icon = match tray_builder.build() {
        Ok(icon) => icon,
        Err(err) => {
            eprintln!("[main] failed to create tray icon: {err}");
            return;
        }
    };

    unsafe {
        let rc = monitor::CGDisplayRegisterReconfigurationCallback(
            Some(display_callback),
            std::ptr::null_mut(),
        );
        if rc != 0 {
            eprintln!("[main] failed to register display callback: {}", rc);
        }
    }

    run_sync(false);
    start_desktop_change_watcher();

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        if let tao::event::Event::NewEvents(tao::event::StartCause::Init) = event {
            hide_from_dock();
        }

        if let Ok(event) = menu_channel.try_recv() {
            let id = event.id;
            if id == save_item.id() {
                run_sync(true);
            } else if id == open_config_item.id() {
                open_config_file();
            } else if id == delay_0_item.id() {
                set_apply_delay_ms(0);
                delay_0_item.set_checked(true);
                delay_300_item.set_checked(false);
                delay_1000_item.set_checked(false);
                delay_2000_item.set_checked(false);
                delay_3000_item.set_checked(false);
            } else if id == delay_300_item.id() {
                set_apply_delay_ms(300);
                delay_0_item.set_checked(false);
                delay_300_item.set_checked(true);
                delay_1000_item.set_checked(false);
                delay_2000_item.set_checked(false);
                delay_3000_item.set_checked(false);
            } else if id == delay_1000_item.id() {
                set_apply_delay_ms(1000);
                delay_0_item.set_checked(false);
                delay_300_item.set_checked(false);
                delay_1000_item.set_checked(true);
                delay_2000_item.set_checked(false);
                delay_3000_item.set_checked(false);
            } else if id == delay_2000_item.id() {
                set_apply_delay_ms(2000);
                delay_0_item.set_checked(false);
                delay_300_item.set_checked(false);
                delay_1000_item.set_checked(false);
                delay_2000_item.set_checked(true);
                delay_3000_item.set_checked(false);
            } else if id == delay_3000_item.id() {
                set_apply_delay_ms(3000);
                delay_0_item.set_checked(false);
                delay_300_item.set_checked(false);
                delay_1000_item.set_checked(false);
                delay_2000_item.set_checked(false);
                delay_3000_item.set_checked(true);
            } else if id == auto_start_item.id() {
                config::set_auto_start(auto_start_item.is_checked());
            } else if id == quit_item.id() {
                *control_flow = ControlFlow::Exit;
            }
        }

        let _ = TrayIconEvent::receiver().try_recv();
    });
}
