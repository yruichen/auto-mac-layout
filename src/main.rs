mod config;
mod layout;
mod monitor;
mod notification;

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};
use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu};
use tray_icon::{Icon, TrayIconBuilder, TrayIconEvent};

const DESKTOP_WATCH_POLL_SECS: u64 = 2;

// ─── Last Action Tracking ───────────────────────────────────────────────────

#[derive(Clone)]
enum ActionKind {
    Saved,
    Restored,
    NewProfileSaved,
}

struct LastAction {
    kind: ActionKind,
    icon_count: usize,
    when: Instant,
}

static LAST_ACTION: OnceLock<Mutex<Option<LastAction>>> = OnceLock::new();

fn last_action_store() -> &'static Mutex<Option<LastAction>> {
    LAST_ACTION.get_or_init(|| Mutex::new(None))
}

fn record_action(kind: ActionKind, icon_count: usize) {
    if let Ok(mut guard) = last_action_store().lock() {
        *guard = Some(LastAction {
            kind,
            icon_count,
            when: Instant::now(),
        });
    }
}

fn format_last_action() -> String {
    let guard = match last_action_store().lock() {
        Ok(g) => g,
        Err(_) => return "Last action: —".to_string(),
    };

    match guard.as_ref() {
        None => "Last action: —".to_string(),
        Some(action) => {
            let elapsed = action.when.elapsed();
            let time_str = if elapsed.as_secs() < 5 {
                "just now".to_string()
            } else if elapsed.as_secs() < 60 {
                format!("{}s ago", elapsed.as_secs())
            } else if elapsed.as_secs() < 3600 {
                format!("{}m ago", elapsed.as_secs() / 60)
            } else {
                format!("{}h ago", elapsed.as_secs() / 3600)
            };

            let verb = match action.kind {
                ActionKind::Saved => "Saved",
                ActionKind::Restored => "Restored",
                ActionKind::NewProfileSaved => "New profile saved",
            };

            format!(
                "Last: {} ({} icons) — {}",
                verb, action.icon_count, time_str
            )
        }
    }
}

// ─── Dock Hiding ────────────────────────────────────────────────────────────

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

// ─── Sync Scheduler ────────────────────────────────────────────────────────

struct SyncScheduler {
    seq: AtomicU64,
    debounce_ms: AtomicU64,
    storage_lock: Mutex<()>,
}

impl SyncScheduler {
    fn new(initial_delay_ms: u64) -> Self {
        Self {
            seq: AtomicU64::new(0),
            debounce_ms: AtomicU64::new(initial_delay_ms),
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

    fn run_apply_now(self: &Arc<Self>) {
        let me = Arc::clone(self);
        thread::spawn(move || {
            me.apply_only();
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
                let count = current.len();
                storage.insert(fp, current);
                write_storage_atomic(&path, &storage);
                record_action(ActionKind::Saved, count);
                notification::notify(
                    "Auto Mac Layout",
                    &format!("Layout saved — {} icons captured", count),
                );
            }
            return;
        }

        if let Some(saved) = storage.get(&fp) {
            let count = saved.len();
            layout::apply_layout(saved);
            record_action(ActionKind::Restored, count);
            notification::notify(
                "Auto Mac Layout",
                &format!("Layout restored — {} icons positioned", count),
            );
            return;
        }

        let current = layout::fetch_current_layout();
        if !current.is_empty() {
            let count = current.len();
            storage.insert(fp, current);
            write_storage_atomic(&path, &storage);
            record_action(ActionKind::NewProfileSaved, count);
            notification::notify(
                "Auto Mac Layout",
                &format!("New display setup detected — {} icons saved", count),
            );
        }
    }

    fn apply_only(&self) {
        let _guard = match self.storage_lock.lock() {
            Ok(g) => g,
            Err(err) => {
                eprintln!("[main] storage lock poisoned: {err}");
                return;
            }
        };

        let fp = monitor::get_fingerprint();
        let path = config::get_config_path();
        let storage = read_storage(&path);

        if let Some(saved) = storage.get(&fp) {
            let count = saved.len();
            layout::apply_layout(saved);
            record_action(ActionKind::Restored, count);
            notification::notify(
                "Auto Mac Layout",
                &format!("Layout applied — {} icons positioned", count),
            );
        } else {
            notification::notify(
                "Auto Mac Layout",
                "No saved layout found for current display setup",
            );
        }
    }
}

// ─── Storage I/O ────────────────────────────────────────────────────────────

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

// ─── Scheduler Singleton ────────────────────────────────────────────────────

fn init_scheduler(initial_delay_ms: u64) -> &'static Arc<SyncScheduler> {
    static SCHEDULER: OnceLock<Arc<SyncScheduler>> = OnceLock::new();
    SCHEDULER.get_or_init(|| Arc::new(SyncScheduler::new(initial_delay_ms)))
}

fn scheduler() -> &'static Arc<SyncScheduler> {
    init_scheduler(1000) // fallback if called before init
}

fn run_sync(force_save: bool) {
    if force_save {
        scheduler().run_force_save_now();
    } else {
        scheduler().schedule();
    }
}

fn run_apply_now() {
    scheduler().run_apply_now();
}

fn set_apply_delay_ms(delay_ms: u64) {
    scheduler().set_debounce_ms(delay_ms);
}

// ─── Desktop Change Watcher ─────────────────────────────────────────────────

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

// ─── Helpers ────────────────────────────────────────────────────────────────

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

fn get_profile_count() -> usize {
    let path = config::get_config_path();
    read_storage(&path).len()
}

fn delete_profile(fingerprint: &str) {
    let path = config::get_config_path();
    let mut storage = read_storage(&path);
    if storage.remove(fingerprint).is_some() {
        write_storage_atomic(&path, &storage);
        notification::notify("Auto Mac Layout", "Profile deleted");
    }
}

fn delete_all_profiles() {
    let path = config::get_config_path();
    let storage: HashMap<String, Vec<layout::Icon>> = HashMap::new();
    write_storage_atomic(&path, &storage);
    notification::notify("Auto Mac Layout", "All profiles deleted");
}

// ─── Display Callback ───────────────────────────────────────────────────────

unsafe extern "C" fn display_callback(
    _id: monitor::CGDirectDisplayID,
    flags: monitor::CGDisplayChangeSummaryFlags,
    _info: *mut std::ffi::c_void,
) {
    // Bit 0 = kCGDisplayBeginConfigurationFlag; skip begin events, act only on completion.
    if flags & (1 << 0) == 0 {
        run_sync(false);
    }
}

// ─── Main ───────────────────────────────────────────────────────────────────

fn main() {
    // Load persisted preferences.
    let prefs = config::load_preferences();
    notification::set_enabled(prefs.notifications_enabled);

    let event_loop = EventLoopBuilder::new().build();
    hide_from_dock();
    let menu_channel = MenuEvent::receiver();

    // Initialize scheduler with persisted delay.
    init_scheduler(prefs.apply_delay_ms);

    // ── Build Menu ──────────────────────────────────────────────────────

    let menu = Menu::new();

    // Status section (disabled items showing live info).
    let display_info_item = MenuItem::new(
        &format!("📺  {}", monitor::get_display_summary()),
        false,
        None,
    );
    let profile_count_item = MenuItem::new(
        &format!("💾  Saved profiles: {}", get_profile_count()),
        false,
        None,
    );
    let last_action_item = MenuItem::new(&format!("🕐  {}", format_last_action()), false, None);

    // Action items.
    let save_item = MenuItem::new("Save Current Layout", true, None);
    let apply_item = MenuItem::new("Apply Layout Now", true, None);
    let open_config_item = MenuItem::new("Open Config File", true, None);

    // Profiles submenu.
    let profiles_submenu = Submenu::new("Saved Profiles", true);
    let delete_all_item = MenuItem::new("Delete All Profiles", true, None);

    // Populate profiles submenu.
    let storage = read_storage(&config::get_config_path());
    let current_fp = monitor::get_fingerprint();
    let mut profile_items: Vec<(String, MenuItem)> = Vec::new();

    for (fp, icons) in &storage {
        let summary = monitor::fingerprint_to_summary(fp);
        let is_current = fp == &current_fp;
        let prefix = if is_current { "✓ " } else { "   " };
        let label = format!("{}{} — {} icons", prefix, summary, icons.len());
        let item = MenuItem::new(&label, true, None);
        let _ = profiles_submenu.append(&item);
        profile_items.push((fp.clone(), item));
    }

    if !storage.is_empty() {
        let _ = profiles_submenu.append(&PredefinedMenuItem::separator());
    }
    let _ = profiles_submenu.append(&delete_all_item);

    // Delay submenu.
    let delay_submenu = Submenu::new("Switch Apply Delay", true);
    let delay_values: [u64; 5] = [0, 300, 1000, 2000, 3000];
    let delay_labels = ["0ms (Fastest)", "300ms", "1000ms", "2000ms", "3000ms"];

    let delay_items: Vec<CheckMenuItem> = delay_values
        .iter()
        .zip(delay_labels.iter())
        .map(|(&ms, &label)| {
            CheckMenuItem::new(label, true, ms == prefs.apply_delay_ms, None)
        })
        .collect();

    for item in &delay_items {
        let _ = delay_submenu.append(item);
    }

    // Settings.
    let notifications_item = CheckMenuItem::new(
        "Enable Notifications",
        true,
        prefs.notifications_enabled,
        None,
    );
    let auto_start_item =
        CheckMenuItem::new("Launch at Login", true, config::is_auto_start(), None);
    let quit_item = MenuItem::new("Quit", true, None);

    // Assemble menu.
    let _ = menu.append_items(&[
        &display_info_item,
        &profile_count_item,
        &last_action_item,
        &PredefinedMenuItem::separator(),
        &save_item,
        &apply_item,
        &open_config_item,
        &PredefinedMenuItem::separator(),
        &profiles_submenu,
        &delay_submenu,
        &PredefinedMenuItem::separator(),
        &notifications_item,
        &auto_start_item,
        &PredefinedMenuItem::separator(),
        &quit_item,
    ]);

    // ── Build Tray Icon ─────────────────────────────────────────────────

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

    // ── Register Display Change Callback ────────────────────────────────

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

    // ── Event Loop ──────────────────────────────────────────────────────

    // Timer for updating status menu items.
    let mut last_status_update = Instant::now();

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::WaitUntil(Instant::now() + Duration::from_secs(30));

        if let tao::event::Event::NewEvents(tao::event::StartCause::Init) = event {
            hide_from_dock();
        }

        // Periodically update status items.
        if last_status_update.elapsed() >= Duration::from_secs(30) {
            last_status_update = Instant::now();
            let _ = display_info_item
                .set_text(&format!("📺  {}", monitor::get_display_summary()));
            let _ =
                profile_count_item.set_text(&format!("💾  Saved profiles: {}", get_profile_count()));
            let _ =
                last_action_item.set_text(&format!("🕐  {}", format_last_action()));
        }

        if let Ok(event) = menu_channel.try_recv() {
            // Update status items on any menu interaction.
            let _ = display_info_item
                .set_text(&format!("📺  {}", monitor::get_display_summary()));
            let _ =
                profile_count_item.set_text(&format!("💾  Saved profiles: {}", get_profile_count()));
            let _ =
                last_action_item.set_text(&format!("🕐  {}", format_last_action()));

            let id = event.id;

            if id == save_item.id() {
                run_sync(true);
            } else if id == apply_item.id() {
                run_apply_now();
            } else if id == open_config_item.id() {
                open_config_file();
            } else if id == delete_all_item.id() {
                delete_all_profiles();
            } else if id == notifications_item.id() {
                let enabled = notifications_item.is_checked();
                notification::set_enabled(enabled);
                let mut prefs = config::load_preferences();
                prefs.notifications_enabled = enabled;
                config::save_preferences(&prefs);
            } else if id == auto_start_item.id() {
                config::set_auto_start(auto_start_item.is_checked());
            } else if id == quit_item.id() {
                *control_flow = ControlFlow::Exit;
            } else {
                // Check delay items.
                for (idx, delay_item) in delay_items.iter().enumerate() {
                    if id == delay_item.id() {
                        let ms = delay_values[idx];
                        set_apply_delay_ms(ms);
                        // Update checked state for all delay items.
                        for (j, other) in delay_items.iter().enumerate() {
                            other.set_checked(j == idx);
                        }
                        // Persist preference.
                        let mut prefs = config::load_preferences();
                        prefs.apply_delay_ms = ms;
                        config::save_preferences(&prefs);
                        break;
                    }
                }

                // Check profile delete items.
                for (fp, item) in &profile_items {
                    if id == item.id() {
                        delete_profile(fp);
                        break;
                    }
                }
            }
        }

        let _ = TrayIconEvent::receiver().try_recv();
    });
}
