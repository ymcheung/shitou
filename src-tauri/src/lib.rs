mod accounts;
mod auth;
mod error;
mod mailbox;
mod models;
mod state;

use error::{AppError, CommandResult};
use mailbox::Mailbox;
use state::AppState;
use std::path::PathBuf;
use std::sync::Mutex;
use std::thread;
use std::time::Duration;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::{AppHandle, Emitter, Manager};
const SETTINGS_MENU_ID: &str = "settings";
const OUTBOX_SYNC_INTERVAL: Duration = Duration::from_secs(60);

fn app_db_path(app: &AppHandle) -> CommandResult<PathBuf> {
    let mut dir = app
        .path()
        .app_data_dir()
        .map_err(|_| AppError::AppDataDirUnavailable)?;
    std::fs::create_dir_all(&dir).map_err(|_| AppError::AppDataDirUnavailable)?;
    dir.push("mailbox.sqlite3");
    Ok(dir)
}

fn app_menu<R: tauri::Runtime>(app_handle: &AppHandle<R>) -> tauri::Result<Menu<R>> {
    let pkg_info = app_handle.package_info();
    let config = app_handle.config();
    let about_metadata = tauri::menu::AboutMetadata {
        name: Some(pkg_info.name.clone()),
        version: Some(pkg_info.version.to_string()),
        copyright: config.bundle.copyright.clone(),
        authors: config
            .bundle
            .publisher
            .clone()
            .map(|publisher| vec![publisher]),
        ..Default::default()
    };

    Menu::with_items(
        app_handle,
        &[
            #[cfg(target_os = "macos")]
            &Submenu::with_items(
                app_handle,
                pkg_info.name.clone(),
                true,
                &[
                    &PredefinedMenuItem::about(app_handle, None, Some(about_metadata.clone()))?,
                    &PredefinedMenuItem::separator(app_handle)?,
                    &MenuItem::with_id(
                        app_handle,
                        SETTINGS_MENU_ID,
                        "Settings...",
                        true,
                        Some("CmdOrCtrl+,"),
                    )?,
                    &PredefinedMenuItem::separator(app_handle)?,
                    &PredefinedMenuItem::services(app_handle, None)?,
                    &PredefinedMenuItem::separator(app_handle)?,
                    &PredefinedMenuItem::hide(app_handle, None)?,
                    &PredefinedMenuItem::hide_others(app_handle, None)?,
                    &PredefinedMenuItem::separator(app_handle)?,
                    &PredefinedMenuItem::quit(app_handle, None)?,
                ],
            )?,
            #[cfg(not(any(
                target_os = "linux",
                target_os = "dragonfly",
                target_os = "freebsd",
                target_os = "netbsd",
                target_os = "openbsd"
            )))]
            &Submenu::with_items(
                app_handle,
                "File",
                true,
                &[
                    #[cfg(not(target_os = "macos"))]
                    &MenuItem::with_id(
                        app_handle,
                        SETTINGS_MENU_ID,
                        "Settings...",
                        true,
                        Some("CmdOrCtrl+,"),
                    )?,
                    &PredefinedMenuItem::close_window(app_handle, None)?,
                    #[cfg(not(target_os = "macos"))]
                    &PredefinedMenuItem::quit(app_handle, None)?,
                ],
            )?,
            &Submenu::with_items(
                app_handle,
                "Edit",
                true,
                &[
                    &PredefinedMenuItem::undo(app_handle, None)?,
                    &PredefinedMenuItem::redo(app_handle, None)?,
                    &PredefinedMenuItem::separator(app_handle)?,
                    &PredefinedMenuItem::cut(app_handle, None)?,
                    &PredefinedMenuItem::copy(app_handle, None)?,
                    &PredefinedMenuItem::paste(app_handle, None)?,
                    &PredefinedMenuItem::select_all(app_handle, None)?,
                ],
            )?,
            #[cfg(target_os = "macos")]
            &Submenu::with_items(
                app_handle,
                "View",
                true,
                &[&PredefinedMenuItem::fullscreen(app_handle, None)?],
            )?,
            &Submenu::with_items(
                app_handle,
                "Window",
                true,
                &[
                    &PredefinedMenuItem::minimize(app_handle, None)?,
                    &PredefinedMenuItem::maximize(app_handle, None)?,
                    #[cfg(target_os = "macos")]
                    &PredefinedMenuItem::separator(app_handle)?,
                    &PredefinedMenuItem::close_window(app_handle, None)?,
                ],
            )?,
            &Submenu::with_items(
                app_handle,
                "Help",
                true,
                &[
                    #[cfg(not(target_os = "macos"))]
                    &PredefinedMenuItem::about(app_handle, None, Some(about_metadata))?,
                ],
            )?,
        ],
    )
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let mut context = tauri::generate_context!();
    context.set_default_window_icon(Some(tauri::include_image!("icons/icon.png")));

    tauri::Builder::default()
        .menu(app_menu)
        .on_menu_event(|app, event| {
            if event.id() == SETTINGS_MENU_ID {
                let _ = app.emit("open-settings", "general");
            }
        })
        .setup(|app| {
            let db_path = app_db_path(app.handle())?;
            let mailbox = Mailbox::open_app(db_path)?;
            app.manage(AppState {
                mailbox: Mutex::new(mailbox),
                outbox: Mutex::new(()),
            });
            let app_handle = app.handle().clone();
            thread::spawn(move || loop {
                mailbox::flush_pending_mail_actions(&app_handle);
                thread::sleep(OUTBOX_SYNC_INTERVAL);
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            auth::auth_send_email_otp,
            auth::auth_current_session,
            auth::auth_verify_email_otp,
            auth::auth_logout,
            accounts::account_connect_provider,
            accounts::account_connect_icloud,
            accounts::account_remove,
            mailbox::sync_account,
            mailbox::sync_all,
            mailbox::list_accounts,
            mailbox::list_folders,
            mailbox::list_messages,
            mailbox::get_message,
            mailbox::mark_messages_read,
            mailbox::mark_messages_unread,
            mailbox::delete_messages,
            mailbox::mark_messages_spam,
            mailbox::set_theme
        ])
        .run(context)
        .expect("error while running Shitou Mail");
}
