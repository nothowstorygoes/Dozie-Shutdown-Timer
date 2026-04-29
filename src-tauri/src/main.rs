#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{
    menu::{Menu, MenuItem},
    tray::{TrayIconBuilder, TrayIconEvent},
    Manager, Runtime,
};
// Importiamo il plugin decorum
use tauri_plugin_decorum::WebviewWindowExt;

struct AppState {
    cancel_flag: Arc<Mutex<bool>>,
}

#[tauri::command]
async fn schedule_action<R: Runtime>(
    app: tauri::AppHandle<R>,
    seconds: u64,
    action: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let flag = state.cancel_flag.clone();
    {
        let mut f = flag.lock().unwrap();
        *f = false;
    }

    if let Some(window) = app.get_webview_window("main") {
        window.hide().map_err(|e| e.to_string())?;
    }

    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_secs(seconds)).await;
        let cancelled = { *flag.lock().unwrap() };

        if !cancelled {
            match action.as_str() {
                "shutdown" => { Command::new("shutdown").args(["/s", "/t", "0"]).spawn().ok(); }
                "sleep" => { Command::new("rundll32").args(["powrprof.dll,SetSuspendState", "0,1,0"]).spawn().ok(); }
                "hibernate" => { Command::new("shutdown").args(["/h"]).spawn().ok(); }
                _ => {}
            }
        }
    });
    Ok(())
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
        // Usiamo AccentColor che è più affidabile per i toggle UI
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
        // Inizializziamo il plugin decorum
        .plugin(tauri_plugin_decorum::init())
        .manage(AppState {
            cancel_flag: Arc::new(Mutex::new(false)),
        })
        .setup(|app| {
            let window = app.get_webview_window("main").unwrap();

            // --- DECORUM & OVERLAY ---
            // Questo crea la barra del titolo personalizzata che si fonde con lo sfondo
            // e inietta i controlli (X, _, ecc.) direttamente nel frame
            window.create_overlay_titlebar().unwrap();

            // --- FIX RESIZE ---
            // Forza il blocco del ridimensionamento via codice. 
            // Decorum rispetta questo comando eliminando i bordi di resize di Windows.
            window.set_resizable(false).unwrap();

            // Configurazione Tray (Menu)
            let quit_i = MenuItem::with_id(app, "quit", "Esci", true, None::<&str>)?;
            let show_i = MenuItem::with_id(app, "show", "Mostra App", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show_i, &quit_i])?;

            let _tray = TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&menu)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "quit" => { app.exit(0); }
                    "show" => {
                        if let Some(window) = app.get_webview_window("main") {
                            window.show().unwrap();
                            window.set_focus().unwrap();
                        }
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click { .. } = event {
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window("main") {
                            window.show().unwrap();
                            window.set_focus().unwrap();
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