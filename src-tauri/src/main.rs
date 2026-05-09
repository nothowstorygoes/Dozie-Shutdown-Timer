#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{TrayIconBuilder, TrayIconEvent, MouseButton},
    Manager, Runtime, Emitter
};
use tauri_plugin_decorum::WebviewWindowExt;

unsafe extern "system" {
    fn SendMessageW(hwnd: isize, msg: u32, wparam: usize, lparam: isize) -> isize;
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
            let label = format!("Time remaining: {:02}:{:02}", mins, secs);
            update_tray_text(&time_item_ref, label);

            tokio::time::sleep(Duration::from_secs(1)).await;
            remaining -= 1;
        }

        update_tray_text(&time_item_ref, "Executing action...".to_string());
        let _ = app.emit("timer-tick", 0);
        
        let cancelled = { *flag.lock().unwrap() };
        if !cancelled {
            match action.as_str() {
                "shutdown" => { Command::new("shutdown").args(["/s", "/t", "0"]).spawn().ok(); }
             "sleep" => {
    unsafe {
        // HWND_BROADCAST = 0xFFFF, WM_SYSCOMMAND = 0x0112
        // SC_MONITORPOWER = 0xF170, 2 = display off
        SendMessageW(0xFFFF, 0x0112, 0xF170, 2);
    }
}
                "hibernate" => { Command::new("shutdown").args(["/h"]).spawn().ok(); }
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
    use winreg::enums::*;
    use winreg::RegKey;
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let dwm_path = "Software\\Microsoft\\Windows\\DWM";
    if let Ok(dwm_key) = hkcu.open_subkey(dwm_path) {
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
                &quit_i
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