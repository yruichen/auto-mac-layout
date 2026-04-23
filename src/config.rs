use auto_launch::AutoLaunchBuilder;
use std::fs;
use std::path::PathBuf;

pub fn get_config_path() -> PathBuf {
    let mut path = dirs::data_local_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push("auto-mac-layout");
    fs::create_dir_all(&path).ok();
    path.push("layouts.json");
    path
}

pub fn set_auto_start(enable: bool) {
    if let Ok(exe_path) = std::env::current_exe() {
        let exe_str = match exe_path.to_str() {
            Some(s) => s,
            None => {
                eprintln!("[config] executable path is not valid UTF-8");
                return;
            }
        };

        let launcher = match AutoLaunchBuilder::new()
            .set_app_name("AutoMacLayout")
            .set_app_path(exe_str)
            .build()
        {
            Ok(l) => l,
            Err(err) => {
                eprintln!("[config] failed to build auto-launch config: {err}");
                return;
            }
        };

        if enable {
            if let Err(err) = launcher.enable() {
                eprintln!("[config] failed to enable launch at login: {err}");
            }
        } else {
            if let Err(err) = launcher.disable() {
                eprintln!("[config] failed to disable launch at login: {err}");
            }
        }
    } else {
        eprintln!("[config] failed to resolve current executable path");
    }
}

pub fn is_auto_start() -> bool {
    if let Ok(exe_path) = std::env::current_exe() {
        let exe_str = match exe_path.to_str() {
            Some(s) => s,
            None => {
                eprintln!("[config] executable path is not valid UTF-8");
                return false;
            }
        };

        let launcher = match AutoLaunchBuilder::new()
            .set_app_name("AutoMacLayout")
            .set_app_path(exe_str)
            .build()
        {
            Ok(l) => l,
            Err(err) => {
                eprintln!("[config] failed to build auto-launch config: {err}");
                return false;
            }
        };

        match launcher.is_enabled() {
            Ok(enabled) => return enabled,
            Err(err) => {
                eprintln!("[config] failed to query launch-at-login state: {err}");
            }
        }
    }
    false
}
