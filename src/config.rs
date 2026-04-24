use auto_launch::AutoLaunchBuilder;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Preferences {
    #[serde(default = "default_apply_delay_ms")]
    pub apply_delay_ms: u64,
    #[serde(default = "default_notifications_enabled")]
    pub notifications_enabled: bool,
}

fn default_apply_delay_ms() -> u64 {
    1000
}

fn default_notifications_enabled() -> bool {
    true
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            apply_delay_ms: default_apply_delay_ms(),
            notifications_enabled: default_notifications_enabled(),
        }
    }
}

pub fn get_config_dir() -> PathBuf {
    let mut path = dirs::data_local_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push("auto-mac-layout");
    fs::create_dir_all(&path).ok();
    path
}

pub fn get_config_path() -> PathBuf {
    let mut path = get_config_dir();
    path.push("layouts.json");
    path
}

fn get_preferences_path() -> PathBuf {
    let mut path = get_config_dir();
    path.push("preferences.json");
    path
}

pub fn load_preferences() -> Preferences {
    let path = get_preferences_path();
    match fs::read_to_string(&path) {
        Ok(content) => match serde_json::from_str::<Preferences>(&content) {
            Ok(prefs) => prefs,
            Err(err) => {
                eprintln!("[config] invalid preferences.json, using defaults: {err}");
                Preferences::default()
            }
        },
        Err(_) => Preferences::default(),
    }
}

pub fn save_preferences(prefs: &Preferences) {
    let path = get_preferences_path();
    match serde_json::to_string_pretty(prefs) {
        Ok(json) => {
            if let Err(err) = fs::write(&path, json) {
                eprintln!("[config] failed to write preferences: {err}");
            }
        }
        Err(err) => {
            eprintln!("[config] failed to serialize preferences: {err}");
        }
    }
}

fn build_auto_launcher() -> Option<auto_launch::AutoLaunch> {
    let exe_path = match std::env::current_exe() {
        Ok(p) => p,
        Err(_) => {
            eprintln!("[config] failed to resolve current executable path");
            return None;
        }
    };

    let exe_str = match exe_path.to_str() {
        Some(s) => s,
        None => {
            eprintln!("[config] executable path is not valid UTF-8");
            return None;
        }
    };

    match AutoLaunchBuilder::new()
        .set_app_name("AutoMacLayout")
        .set_app_path(exe_str)
        .build()
    {
        Ok(l) => Some(l),
        Err(err) => {
            eprintln!("[config] failed to build auto-launch config: {err}");
            None
        }
    }
}

pub fn set_auto_start(enable: bool) {
    if let Some(launcher) = build_auto_launcher() {
        if enable {
            if let Err(err) = launcher.enable() {
                eprintln!("[config] failed to enable launch at login: {err}");
            }
        } else {
            if let Err(err) = launcher.disable() {
                eprintln!("[config] failed to disable launch at login: {err}");
            }
        }
    }
}

pub fn is_auto_start() -> bool {
    if let Some(launcher) = build_auto_launcher() {
        match launcher.is_enabled() {
            Ok(enabled) => return enabled,
            Err(err) => {
                eprintln!("[config] failed to query launch-at-login state: {err}");
            }
        }
    }
    false
}
