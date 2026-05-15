#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{
    Emitter, Manager, Runtime,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, TrayIconBuilder, TrayIconEvent},
};
use tauri_plugin_decorum::WebviewWindowExt;
#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[link(name = "user32")]
unsafe extern "system" {
    fn SendMessageW(hwnd: isize, msg: u32, wparam: usize, lparam: isize) -> isize;
    fn keybd_event(bvk: u8, bscan: u8, dwflags: u32, dwextrainfo: usize);
}

#[link(name = "powrprof")]
unsafe extern "system" {
    fn SetSuspendState(bhibernate: u8, bforce: u8, bwakeupeventsdisabled: u8) -> u8;
}

#[link(name = "advapi32")]
unsafe extern "system" {
    fn OpenProcessToken(processhandle: isize, desiredaccess: u32, tokenhandle: *mut isize) -> i32;
    fn LookupPrivilegeValueW(lpsystemname: *const u16, lpname: *const u16, lpluid: *mut [u32; 2]) -> i32;
    fn AdjustTokenPrivileges(tokenhandle: isize, disableallprivileges: i32, newstate: *const u8, bufferlength: u32, previousstate: *mut u8, returnlength: *mut u32) -> i32;
    fn CloseHandle(hobject: isize) -> i32;
    fn GetCurrentProcess() -> isize;
}

#[cfg(windows)]
unsafe fn enable_shutdown_privilege() {
    const TOKEN_ADJUST_PRIVILEGES: u32 = 0x0020;
    const TOKEN_QUERY: u32 = 0x0008;
    const SE_PRIVILEGE_ENABLED: u32 = 0x00000002;

    let mut token: isize = 0;
    if OpenProcessToken(GetCurrentProcess(), TOKEN_ADJUST_PRIVILEGES | TOKEN_QUERY, &mut token) == 0 {
        return;
    }

    let name: Vec<u16> = "SeShutdownPrivilege\0".encode_utf16().collect();
    let mut luid = [0u32; 2];
    if LookupPrivilegeValueW(std::ptr::null(), name.as_ptr(), &mut luid) == 0 {
        CloseHandle(token);
        return;
    }

    let tp: [u32; 4] = [1, luid[0], luid[1], SE_PRIVILEGE_ENABLED];
    AdjustTokenPrivileges(
        token, 0,
        tp.as_ptr() as *const u8,
        std::mem::size_of_val(&tp) as u32,
        std::ptr::null_mut(),
        std::ptr::null_mut(),
    );
    CloseHandle(token);
}

#[cfg(windows)]
unsafe fn media_stop() {
    const VK_MEDIA_STOP: u8 = 0xB2;
    const KEYEVENTF_KEYUP: u32 = 0x0002;
    keybd_event(VK_MEDIA_STOP, 0, 0, 0);
    keybd_event(VK_MEDIA_STOP, 0, KEYEVENTF_KEYUP, 0);
}

struct AppState {
    cancel_flag: Arc<Mutex<bool>>,
    remaining_time_item: Arc<Mutex<Option<MenuItem<tauri::Wry>>>>,
}

#[tauri::command]
async fn schedule_action<R: Runtime>(
    app: tauri::AppHandle<R>,
    seconds: u64,
    action: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let flag = state.cancel_flag.clone();
    let time_item_ref = state.remaining_time_item.clone();

    {
        let mut f = flag.lock().unwrap();
        *f = false;
    }

    if let Some(window) = app.get_webview_window("main") {
        window.hide().map_err(|e| e.to_string())?;
    }

    tauri::async_runtime::spawn(async move {
        let mut remaining = seconds;

        while remaining > 0 {
            if *flag.lock().unwrap() {
                update_tray_text(&time_item_ref, "No active timer".to_string());
                let _ = app.emit("timer-tick", 0);
                return;
            }

            let _ = app.emit("timer-tick", remaining);

            let mins = remaining / 60;
            let secs = remaining % 60;
            update_tray_text(&time_item_ref, format!("Time remaining: {:02}:{:02}", mins, secs));

            tokio::time::sleep(Duration::from_secs(1)).await;
            remaining -= 1;
        }

        update_tray_text(&time_item_ref, "Executing action...".to_string());
        let _ = app.emit("timer-tick", 0);

        let cancelled = { *flag.lock().unwrap() };
        if !cancelled {
            match action.as_str() {
                "shutdown" => {
                    Command::new("shutdown").args(["/s", "/t", "0"]).spawn().ok();
                    app.exit(0);
                }
                "sleep" => {
                    #[cfg(windows)]
                    unsafe {
                        enable_shutdown_privilege();
                        media_stop();
                        tokio::time::sleep(Duration::from_millis(100)).await;
                        // Tentativo 1: SetSuspendState (S3 o sistemi con hibernate abilitato)
                        let result = SetSuspendState(0, 0, 0);
                        if result == 0 {
                            // Tentativo 2: SC_MONITORPOWER (S0 Modern Standby — Win11)
                            SendMessageW(0xFFFF, 0x0112, 0xF170, 2);
                        }
                    }
                    tokio::time::sleep(Duration::from_millis(500)).await;
                    app.exit(0);
                }
                "hibernate" => {
                    #[cfg(windows)]
                    unsafe {
                        enable_shutdown_privilege();
                        media_stop();
                        tokio::time::sleep(Duration::from_millis(100)).await;
                        // Tentativo 1: SetSuspendState hibernate (sistemi con hibernate abilitato)
                        let result = SetSuspendState(1, 0, 0);
                        if result == 0 {
                            // Fallback: shutdown /h
                            Command::new("shutdown")
                                .args(["/h"])
                                .creation_flags(0x08000000)
                                .spawn()
                                .ok();
                            tokio::time::sleep(Duration::from_millis(1000)).await;
                        }
                    }
                    tokio::time::sleep(Duration::from_millis(500)).await;
                    app.exit(0);
                }
                _ => {}
            }
        }
    });

    Ok(())
}

fn update_tray_text(item_lock: &Arc<Mutex<Option<MenuItem<tauri::Wry>>>>, text: String) {
    if let Some(item) = item_lock.lock().unwrap().as_ref() {
        let _ = item.set_text(text);
    }
}

#[tauri::command]
fn cancel_action(state: tauri::State<'_, AppState>) -> Result<(), String> {
    let mut f = state.cancel_flag.lock().unwrap();
    *f = true;
    Ok(())
}

#[tauri::command]
fn get_accent_color() -> String {
    use winreg::RegKey;
    use winreg::enums::*;
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    if let Ok(dwm_key) = hkcu.open_subkey("Software\\Microsoft\\Windows\\DWM") {
        let accent: u32 = dwm_key.get_value("AccentColorMenu").unwrap_or(0xff3b82f6);
        let r = (accent & 0xFF) as u8;
        let g = ((accent >> 8) & 0xFF) as u8;
        let b = ((accent >> 16) & 0xFF) as u8;
        return format!("#{:02x}{:02x}{:02x}", r, g, b);
    }
    "#3b82f6".to_string()
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_decorum::init())
        .manage(AppState {
            cancel_flag: Arc::new(Mutex::new(false)),
            remaining_time_item: Arc::new(Mutex::new(None)),
        })
        .setup(|app| {
            let window = app.get_webview_window("main").unwrap();
            window.create_overlay_titlebar().unwrap();
            window.set_resizable(false).unwrap();

            let time_i = MenuItem::with_id(app, "time", "No active timer", true, None::<&str>)?;
            let show_i = MenuItem::with_id(app, "show", "Open App", true, None::<&str>)?;
            let quit_i = MenuItem::with_id(app, "quit", "Cancel and Quit", true, None::<&str>)?;

            let menu = Menu::with_items(app, &[
                &time_i,
                &show_i,
                &PredefinedMenuItem::separator(app)?,
                &quit_i,
            ])?;

            let state = app.state::<AppState>();
            *state.remaining_time_item.lock().unwrap() = Some(time_i);

            let _tray = TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(move |app, event| match event.id.as_ref() {
                    "quit" => { app.exit(0); }
                    "show" | "time" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click { button: MouseButton::Left, .. } = event {
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                })
                .build(app)?;

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            schedule_action,
            cancel_action,
            get_accent_color
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}