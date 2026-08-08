mod error;
mod models;
mod state;

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use chrono::{DateTime, Utc};
use error::{AppError, CommandResult};
use models::{
    Attachment, AuthSession, AuthStartResult, CountResult, Folder, MailAccount, MessageDetail,
    MessageSummary, Provider, Removed, ThemeResult,
};
use reqwest::blocking::Client;
use reqwest::StatusCode;
use rusqlite::{params, Connection, OptionalExtension};
use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use state::AppState;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant};
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::{AppHandle, Emitter, Manager};
use uuid::Uuid;

const SERVICE_NAME: &str = "com.shitou.mail";
const MAILBOX_KEY_ACCOUNT: &str = "local-mailbox-sqlcipher-key";
const AUTH_SESSION_SETTING_KEY: &str = "auth_session";
const SETTINGS_MENU_ID: &str = "settings";
const DEFAULT_NEON_AUTH_BASE_URL: &str =
    "https://ep-floral-heart-a7t3tanr.neonauth.ap-southeast-2.aws.neon.tech/neondb/auth";
const DEFAULT_ICLOUD_CONNECT_URL: &str =
    "https://shitou-icloud-connect.shitou-mail-cloud.workers.dev/icloud/connect";
const NYLAS_CALLBACK_URI: &str = "http://127.0.0.1:8392/callback";
const NYLAS_CALLBACK_ADDRESS: &str = "127.0.0.1:8392";
const NYLAS_CALLBACK_TIMEOUT: Duration = Duration::from_secs(300);
const OUTBOX_SYNC_INTERVAL: Duration = Duration::from_secs(60);

#[derive(Deserialize)]
struct NylasTokenResponse {
    access_token: String,
    refresh_token: String,
    grant_id: String,
    email: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct NylasGrantResponse {
    grant_id: String,
    email: String,
    access_token: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct IcloudFolder {
    id: String,
    name: String,
    unread_count: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct IcloudMessage {
    id: String,
    folder_id: String,
    sender: String,
    recipients: Vec<String>,
    subject: String,
    preview: String,
    received_at: String,
    has_attachments: bool,
    is_unread: bool,
}

#[derive(Deserialize)]
struct IcloudSyncResponse {
    folders: Vec<IcloudFolder>,
    messages: Vec<IcloudMessage>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct IcloudMessageResponse {
    body_html: String,
    body_text: String,
    attachments: Vec<Attachment>,
}

fn neon_auth_url(path: &str) -> CommandResult<String> {
    let base_url = std::env::var("NEON_AUTH_BASE_URL")
        .unwrap_or_else(|_| DEFAULT_NEON_AUTH_BASE_URL.to_string());
    Ok(format!(
        "{}/{}",
        base_url.trim_end_matches('/'),
        path.trim_start_matches('/')
    ))
}

fn response_error_message(service: &str, status: StatusCode, body: &str) -> String {
    let fallback = format!("{service} request failed with HTTP {status}");
    let Ok(value) = serde_json::from_str::<Value>(body) else {
        return if body.trim().is_empty() {
            fallback
        } else {
            format!("{fallback}: {}", body.trim())
        };
    };

    value
        .pointer("/error/message")
        .or_else(|| value.pointer("/message"))
        .or_else(|| value.pointer("/error"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or(fallback)
}

fn post_neon_auth(path: &str, body: Value) -> CommandResult<Value> {
    let url = neon_auth_url(path)?;
    let response = Client::new()
        .post(url)
        .json(&body)
        .send()
        .map_err(|error| AppError::Network(error.to_string()))?;

    let status = response.status();
    let response_text = response
        .text()
        .map_err(|error| AppError::Network(error.to_string()))?;

    if !status.is_success() {
        return Err(AppError::Auth(response_error_message(
            "Neon Auth",
            status,
            &response_text,
        )));
    }

    if response_text.trim().is_empty() {
        return Ok(Value::Null);
    }

    serde_json::from_str(&response_text).map_err(|error| AppError::Auth(error.to_string()))
}

fn auth_response_user(auth_response: &Value, fallback_email: &str) -> AuthSession {
    let user = auth_response
        .pointer("/user")
        .or_else(|| auth_response.pointer("/data/user"));
    let email = user
        .and_then(|value| value.pointer("/email"))
        .and_then(Value::as_str)
        .unwrap_or(fallback_email);
    let user_id = user
        .and_then(|value| value.pointer("/id"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| Uuid::new_v4().to_string());

    AuthSession {
        email: email.to_string(),
        user_id,
        authenticated: true,
    }
}

fn save_auth_session(conn: &Connection, session: &AuthSession) -> CommandResult<()> {
    let value =
        serde_json::to_string(session).map_err(|error| AppError::Auth(error.to_string()))?;
    conn.execute(
        "INSERT INTO local_settings (key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![AUTH_SESSION_SETTING_KEY, value],
    )?;
    Ok(())
}

fn app_db_path(app: &AppHandle) -> CommandResult<PathBuf> {
    let mut dir = app
        .path()
        .app_data_dir()
        .map_err(|_| AppError::AppDataDirUnavailable)?;
    std::fs::create_dir_all(&dir).map_err(|_| AppError::AppDataDirUnavailable)?;
    dir.push("mailbox.sqlite3");
    Ok(dir)
}

fn init_database(path: PathBuf) -> CommandResult<Connection> {
    let conn = Connection::open(path)?;
    let mailbox_key = mailbox_encryption_key()?;
    conn.pragma_update(None, "key", mailbox_key)?;
    conn.execute_batch(
        r#"
        PRAGMA foreign_keys = ON;
        PRAGMA journal_mode = WAL;

        CREATE TABLE IF NOT EXISTS accounts (
          id TEXT PRIMARY KEY,
          provider TEXT NOT NULL CHECK (provider IN ('gmail', 'outlook', 'icloud')),
          email TEXT NOT NULL,
          display_name TEXT NOT NULL,
          sync_status TEXT NOT NULL DEFAULT 'idle',
          last_synced_at TEXT
        );

        CREATE TABLE IF NOT EXISTS folders (
          id TEXT PRIMARY KEY,
          account_id TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
          name TEXT NOT NULL,
          unread_count INTEGER NOT NULL DEFAULT 0
        );

        CREATE TABLE IF NOT EXISTS messages (
          id TEXT PRIMARY KEY,
          folder_id TEXT NOT NULL REFERENCES folders(id) ON DELETE CASCADE,
          account_id TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
          provider_message_id TEXT NOT NULL,
          sender TEXT NOT NULL,
          recipients_json TEXT NOT NULL,
          subject TEXT NOT NULL,
          preview TEXT NOT NULL,
          received_at TEXT NOT NULL,
          has_attachments INTEGER NOT NULL DEFAULT 0,
          is_unread INTEGER NOT NULL DEFAULT 0
        );

        CREATE TABLE IF NOT EXISTS message_bodies (
          message_id TEXT PRIMARY KEY REFERENCES messages(id) ON DELETE CASCADE,
          body_html TEXT NOT NULL,
          body_text TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS attachments (
          id TEXT PRIMARY KEY,
          message_id TEXT NOT NULL REFERENCES messages(id) ON DELETE CASCADE,
          file_name TEXT NOT NULL,
          mime_type TEXT NOT NULL,
          byte_size INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS sync_state (
          account_id TEXT PRIMARY KEY REFERENCES accounts(id) ON DELETE CASCADE,
          cursor TEXT,
          updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS local_settings (
          key TEXT PRIMARY KEY,
          value TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS pending_mail_actions (
          account_id TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
          provider_message_id TEXT NOT NULL,
          action TEXT NOT NULL CHECK (action IN ('trash', 'delete')),
          created_at TEXT NOT NULL,
          PRIMARY KEY (account_id, provider_message_id)
        );
        "#,
    )?;
    remove_demo_seed_data(&conn)?;
    Ok(conn)
}

fn mailbox_encryption_key() -> CommandResult<String> {
    let entry = keyring::Entry::new(SERVICE_NAME, MAILBOX_KEY_ACCOUNT)?;
    match entry.get_password() {
        Ok(key) => Ok(key),
        Err(keyring::Error::NoEntry) => {
            let key = Uuid::new_v4().to_string();
            entry.set_password(&key)?;
            Ok(key)
        }
        Err(error) => Err(AppError::Keychain(error)),
    }
}

fn remove_demo_seed_data(conn: &Connection) -> CommandResult<()> {
    conn.execute(
        "DELETE FROM accounts
         WHERE (id = 'acc-gmail' AND email = 'reader@gmail.com')
            OR (id = 'acc-icloud' AND email = 'reader@icloud.com')",
        [],
    )?;
    Ok(())
}

#[allow(dead_code)]
fn seed_demo_data(conn: &Connection) -> CommandResult<()> {
    let count: i64 = conn.query_row("SELECT COUNT(*) FROM accounts", [], |row| row.get(0))?;
    if count > 0 {
        return Ok(());
    }

    let now = Utc::now();
    conn.execute(
        "INSERT INTO accounts (id, provider, email, display_name, sync_status, last_synced_at) VALUES (?1, 'gmail', ?2, 'Gmail', 'idle', ?3)",
        params!["acc-gmail", "reader@gmail.com", now.to_rfc3339()],
    )?;
    conn.execute(
        "INSERT INTO accounts (id, provider, email, display_name, sync_status, last_synced_at) VALUES (?1, 'icloud', ?2, 'iCloud Mail', 'offline', ?3)",
        params!["acc-icloud", "reader@icloud.com", now.to_rfc3339()],
    )?;
    conn.execute_batch(
        "INSERT INTO folders (id, account_id, name, unread_count) VALUES ('inbox', 'acc-gmail', 'Inbox', 1);
         INSERT INTO folders (id, account_id, name, unread_count) VALUES ('archive', 'acc-gmail', 'Archive', 0);
         INSERT INTO folders (id, account_id, name, unread_count) VALUES ('trash', 'acc-gmail', 'Trash', 0);
         INSERT INTO folders (id, account_id, name, unread_count) VALUES ('spam', 'acc-gmail', 'Spam', 1);
         INSERT INTO folders (id, account_id, name, unread_count) VALUES ('icloud-inbox', 'acc-icloud', 'Inbox', 1);
         INSERT INTO folders (id, account_id, name, unread_count) VALUES ('icloud-trash', 'acc-icloud', 'Trash', 0);
         INSERT INTO folders (id, account_id, name, unread_count) VALUES ('icloud-spam', 'acc-icloud', 'Spam', 0);",
    )?;

    insert_message(
        conn,
        "msg-1",
        "inbox",
        "acc-gmail",
        "gmail:msg-1",
        "Gmail API",
        &["reader@gmail.com"],
        "Read-only scope accepted",
        "This account uses gmail.readonly and stores offline bodies locally on this Mac.",
        true,
        false,
        "<p>This account uses <code>gmail.readonly</code> and stores offline bodies locally on this Mac.</p><div class=\"signature\"><p>Alex Morgan<br>Platform Integrations<br>Gmail API</p></div>",
        "This account uses gmail.readonly and stores offline bodies locally on this Mac.\n\nAlex Morgan\nPlatform Integrations\nGmail API",
    )?;
    insert_message(
        conn,
        "msg-2",
        "icloud-inbox",
        "acc-icloud",
        "imap:uid-301",
        "iCloud Mail",
        &["reader@icloud.com"],
        "IMAP sync cached",
        "iCloud reads over IMAP with an app-specific password stored in Keychain.",
        true,
        false,
        "<p>iCloud reads over IMAP with an app-specific password stored in Keychain. SMTP is not configured in v1.</p><div class=\"signature\"><p>Mina Park<br>Mailbox Operations<br>iCloud Mail</p></div>",
        "iCloud reads over IMAP with an app-specific password stored in Keychain. SMTP is not configured in v1.\n\nMina Park\nMailbox Operations\niCloud Mail",
    )?;
    insert_message(
        conn,
        "msg-3",
        "spam",
        "acc-gmail",
        "gmail:msg-3",
        "Security Notice",
        &["reader@gmail.com"],
        "Untrusted sender quarantined",
        "This demo message appears in the aggregate spam folder.",
        true,
        false,
        "<p>This demo message appears in the aggregate spam folder.</p>",
        "This demo message appears in the aggregate spam folder.",
    )?;
    insert_message(
        conn,
        "msg-4",
        "inbox",
        "acc-gmail",
        "gmail:msg-4",
        "Design Review",
        &["reader@gmail.com"],
        "Signature samples attached",
        "Attached are the signature samples for testing cached attachment metadata.",
        false,
        true,
        "<p>Attached are two signature samples for the demo mailbox.</p><p>The HTML version uses a normal text signature so we can compare it with image-heavy signatures separately.</p><div class=\"signature\"><p>Jordan Lee<br>Design Systems<br>Shitou Mail</p></div>",
        "Attached are two signature samples for the demo mailbox. The HTML version uses a normal text signature so we can compare it with image-heavy signatures separately.\n\nJordan Lee\nDesign Systems\nShitou Mail",
    )?;
    insert_attachment(
        conn,
        "att-2",
        "msg-4",
        "signature-samples.pdf",
        "application/pdf",
        482_176,
    )?;
    insert_attachment(
        conn,
        "att-3",
        "msg-4",
        "brand-footer.png",
        "image/png",
        128_904,
    )?;
    insert_message(
        conn,
        "msg-5",
        "icloud-inbox",
        "acc-icloud",
        "imap:uid-302",
        "Northstar Labs",
        &["reader@icloud.com"],
        "Logo signature rendering check",
        "This message includes images inside the signature block.",
        false,
        false,
        "<p>Please confirm the reader keeps inline signature images visible in the offline body cache.</p><div class=\"signature\"><p><img alt=\"Northstar Labs mark\" width=\"36\" height=\"36\" src=\"data:image/svg+xml,%3Csvg%20xmlns=%22http://www.w3.org/2000/svg%22%20width=%2236%22%20height=%2236%22%20viewBox=%220%200%2036%2036%22%3E%3Crect%20width=%2236%22%20height=%2236%22%20rx=%228%22%20fill=%22%2318181b%22/%3E%3Cpath%20d=%22M18%206l3.2%208.8L30%2018l-8.8%203.2L18%2030l-3.2-8.8L6%2018l8.8-3.2L18%206z%22%20fill=%22%23facc15%22/%3E%3C/svg%3E\"></p><p>Avery Chen<br>Northstar Labs</p><p><img alt=\"Certified offline badge\" width=\"96\" height=\"24\" src=\"data:image/svg+xml,%3Csvg%20xmlns=%22http://www.w3.org/2000/svg%22%20width=%2296%22%20height=%2224%22%20viewBox=%220%200%2096%2024%22%3E%3Crect%20width=%2296%22%20height=%2224%22%20rx=%2212%22%20fill=%22%23ecfeff%22/%3E%3Ctext%20x=%2212%22%20y=%2216%22%20font-family=%22Arial%22%20font-size=%2210%22%20font-weight=%22700%22%20fill=%22%230e7490%22%3EOFFLINE%20READY%3C/text%3E%3C/svg%3E\"></p></div>",
        "Please confirm the reader keeps inline signature images visible in the offline body cache.\n\nAvery Chen\nNorthstar Labs",
    )?;
    Ok(())
}

#[allow(dead_code, clippy::too_many_arguments)]
fn insert_message(
    conn: &Connection,
    id: &str,
    folder_id: &str,
    account_id: &str,
    provider_message_id: &str,
    sender: &str,
    recipients: &[&str],
    subject: &str,
    preview: &str,
    is_unread: bool,
    has_attachments: bool,
    body_html: &str,
    body_text: &str,
) -> CommandResult<()> {
    conn.execute(
        "INSERT INTO messages (id, folder_id, account_id, provider_message_id, sender, recipients_json, subject, preview, received_at, has_attachments, is_unread) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        params![
            id,
            folder_id,
            account_id,
            provider_message_id,
            sender,
            serde_json::to_string(recipients).unwrap_or_else(|_| "[]".to_string()),
            subject,
            preview,
            Utc::now().to_rfc3339(),
            if has_attachments { 1_i64 } else { 0_i64 },
            if is_unread { 1_i64 } else { 0_i64 }
        ],
    )?;
    conn.execute(
        "INSERT INTO message_bodies (message_id, body_html, body_text) VALUES (?1, ?2, ?3)",
        params![id, body_html, body_text],
    )?;
    Ok(())
}

#[allow(dead_code)]
fn insert_attachment(
    conn: &Connection,
    id: &str,
    message_id: &str,
    file_name: &str,
    mime_type: &str,
    byte_size: i64,
) -> CommandResult<()> {
    conn.execute(
        "INSERT INTO attachments (id, message_id, file_name, mime_type, byte_size) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![id, message_id, file_name, mime_type, byte_size],
    )?;
    Ok(())
}

#[tauri::command]
fn auth_send_email_otp(email: String) -> CommandResult<AuthStartResult> {
    if !email.contains('@') {
        return Err(AppError::InvalidInput(
            "enter a valid email address".to_string(),
        ));
    }

    post_neon_auth(
        "email-otp/send-verification-otp",
        json!({ "email": email, "type": "sign-in" }),
    )?;
    Ok(AuthStartResult { sent: true, email })
}

#[tauri::command]
fn auth_current_session(state: tauri::State<AppState>) -> CommandResult<Option<AuthSession>> {
    let conn = state.db.lock().expect("database mutex poisoned");
    let value = conn
        .query_row(
            "SELECT value FROM local_settings WHERE key = ?1",
            params![AUTH_SESSION_SETTING_KEY],
            |row| row.get::<_, String>(0),
        )
        .optional()?;

    value
        .map(|raw| {
            serde_json::from_str::<AuthSession>(&raw)
                .map_err(|error| AppError::Auth(error.to_string()))
        })
        .transpose()
}

#[tauri::command]
fn auth_verify_email_otp(
    state: tauri::State<AppState>,
    email: String,
    otp: String,
) -> CommandResult<AuthSession> {
    if !email.contains('@') {
        return Err(AppError::InvalidInput(
            "enter a valid email address".to_string(),
        ));
    }

    if otp.trim().len() < 6 {
        return Err(AppError::InvalidInput(
            "enter the 6-digit verification code".to_string(),
        ));
    }

    let auth_response = post_neon_auth(
        "sign-in/email-otp",
        json!({ "email": email, "otp": otp.trim() }),
    )?;
    let session = auth_response_user(&auth_response, &email);
    let conn = state.db.lock().expect("database mutex poisoned");
    save_auth_session(&conn, &session)?;
    Ok(session)
}

#[tauri::command]
fn auth_logout(state: tauri::State<AppState>) -> CommandResult<Removed> {
    let conn = state.db.lock().expect("database mutex poisoned");
    conn.execute(
        "DELETE FROM local_settings WHERE key = ?1",
        params![AUTH_SESSION_SETTING_KEY],
    )?;
    Ok(Removed { removed: true })
}

fn nylas_api_uri() -> String {
    std::env::var("NYLAS_API_URI")
        .unwrap_or_else(|_| "https://api.us.nylas.com".to_string())
        .trim_end_matches('/')
        .to_string()
}

fn oauth_query_parameter(target: &str, name: &str) -> Option<String> {
    target.split_once('?')?.1.split('&').find_map(|part| {
        let (key, value) = part.split_once('=')?;
        (key == name).then(|| {
            urlencoding::decode(value)
                .map(|decoded| decoded.into_owned())
                .unwrap_or_else(|_| value.to_string())
        })
    })
}

#[cfg(test)]
mod tests {
    use super::{oauth_query_parameter, queue_message_deletions};
    use rusqlite::Connection;

    #[test]
    fn reads_encoded_oauth_callback_parameters() {
        let target = "/callback?code=one%2Ftwo&state=expected";
        assert_eq!(
            oauth_query_parameter(target, "code").as_deref(),
            Some("one/two")
        );
        assert_eq!(
            oauth_query_parameter(target, "state").as_deref(),
            Some("expected")
        );
        assert_eq!(oauth_query_parameter(target, "error"), None);
    }

    #[test]
    fn queues_remote_delete_and_moves_local_message_first() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE accounts (id TEXT PRIMARY KEY, provider TEXT NOT NULL);
             CREATE TABLE folders (id TEXT PRIMARY KEY, account_id TEXT NOT NULL, name TEXT NOT NULL, unread_count INTEGER NOT NULL);
             CREATE TABLE messages (id TEXT PRIMARY KEY, folder_id TEXT NOT NULL, account_id TEXT NOT NULL, provider_message_id TEXT NOT NULL, is_unread INTEGER NOT NULL);
             CREATE TABLE pending_mail_actions (account_id TEXT NOT NULL, provider_message_id TEXT NOT NULL, action TEXT NOT NULL, created_at TEXT NOT NULL, PRIMARY KEY (account_id, provider_message_id));
             INSERT INTO accounts VALUES ('account-1', 'icloud');
             INSERT INTO folders VALUES ('inbox', 'account-1', 'Inbox', 1);
             INSERT INTO folders VALUES ('trash', 'account-1', 'Deleted Messages', 0);
             INSERT INTO messages VALUES ('local-1', 'inbox', 'account-1', 'remote-1', 1);",
        )
        .unwrap();

        assert_eq!(
            queue_message_deletions(&conn, &["local-1".to_string()]).unwrap(),
            1
        );
        assert_eq!(
            conn.query_row(
                "SELECT folder_id FROM messages WHERE id = 'local-1'",
                [],
                |row| row.get::<_, String>(0),
            )
            .unwrap(),
            "trash"
        );
        assert_eq!(
            conn.query_row(
                "SELECT action FROM pending_mail_actions WHERE provider_message_id = 'remote-1'",
                [],
                |row| row.get::<_, String>(0),
            )
            .unwrap(),
            "trash"
        );
    }
}

fn wait_for_oauth_code(listener: TcpListener, expected_state: &str) -> CommandResult<String> {
    listener
        .set_nonblocking(true)
        .map_err(|error| AppError::Auth(error.to_string()))?;
    let started = Instant::now();

    loop {
        match listener.accept() {
            Ok((mut stream, _)) => {
                stream
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .map_err(|error| AppError::Auth(error.to_string()))?;
                let mut request = [0_u8; 8192];
                let read = stream
                    .read(&mut request)
                    .map_err(|error| AppError::Auth(error.to_string()))?;
                let request = String::from_utf8_lossy(&request[..read]);
                let target = request
                    .lines()
                    .next()
                    .and_then(|line| line.split_whitespace().nth(1))
                    .ok_or_else(|| AppError::Auth("invalid OAuth callback".to_string()))?;

                let result = if !target.starts_with("/callback?") {
                    Err(AppError::Auth("invalid OAuth callback path".to_string()))
                } else if oauth_query_parameter(target, "state").as_deref() != Some(expected_state)
                {
                    Err(AppError::Auth("OAuth state validation failed".to_string()))
                } else if let Some(error) = oauth_query_parameter(target, "error") {
                    Err(AppError::Auth(error))
                } else {
                    oauth_query_parameter(target, "code").ok_or_else(|| {
                        AppError::Auth("OAuth callback omitted its code".to_string())
                    })
                };

                let (status, message) = if result.is_ok() {
                    (
                        "200 OK",
                        "Account connected. You can return to Shitou Mail.",
                    )
                } else {
                    (
                        "400 Bad Request",
                        "Account connection failed. Return to Shitou Mail and try again.",
                    )
                };
                let response = format!(
                    "HTTP/1.1 {status}\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{message}",
                    message.len()
                );
                let _ = stream.write_all(response.as_bytes());
                return result;
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                if started.elapsed() >= NYLAS_CALLBACK_TIMEOUT {
                    return Err(AppError::Auth("account connection timed out".to_string()));
                }
                thread::sleep(Duration::from_millis(100));
            }
            Err(error) => return Err(AppError::Auth(error.to_string())),
        }
    }
}

fn open_system_browser(url: &str) -> CommandResult<()> {
    #[cfg(target_os = "macos")]
    let status = Command::new("open").arg(url).status();
    #[cfg(target_os = "windows")]
    let status = Command::new("cmd").args(["/C", "start", "", url]).status();
    #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
    let status = Command::new("xdg-open").arg(url).status();

    match status {
        Ok(status) if status.success() => Ok(()),
        Ok(_) => Err(AppError::Auth(
            "could not open the system browser".to_string(),
        )),
        Err(error) => Err(AppError::Auth(error.to_string())),
    }
}

fn complete_nylas_oauth(provider: Provider) -> CommandResult<NylasTokenResponse> {
    let client_id = std::env::var("NYLAS_CLIENT_ID")
        .map_err(|_| AppError::MissingEnv("NYLAS_CLIENT_ID".to_string()))?;
    let listener = TcpListener::bind(NYLAS_CALLBACK_ADDRESS).map_err(|error| {
        AppError::Auth(format!(
            "cannot listen for the OAuth callback on {NYLAS_CALLBACK_ADDRESS}: {error}"
        ))
    })?;
    let verifier = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    let state = Uuid::new_v4().simple().to_string();
    let provider_name = match provider {
        Provider::Gmail => "google",
        Provider::Outlook => "microsoft",
        Provider::Icloud => "icloud",
    };
    let scope = match provider {
        Provider::Gmail => Some(
            "https://www.googleapis.com/auth/gmail.readonly https://www.googleapis.com/auth/userinfo.email",
        ),
        Provider::Outlook => Some("openid email offline_access Mail.Read"),
        Provider::Icloud => None,
    };
    let mut auth_url = format!(
        "{}/v3/connect/auth?client_id={}&redirect_uri={}&response_type=code&provider={}&code_challenge_method=S256&code_challenge={}&state={}&access_type=offline",
        nylas_api_uri(),
        urlencoding::encode(&client_id),
        urlencoding::encode(NYLAS_CALLBACK_URI),
        provider_name,
        urlencoding::encode(&challenge),
        urlencoding::encode(&state),
    );
    if let Some(scope) = scope {
        auth_url.push_str("&scope=");
        auth_url.push_str(&urlencoding::encode(scope));
    }

    open_system_browser(&auth_url)?;
    let code = wait_for_oauth_code(listener, &state)?;
    let response = Client::new()
        .post(format!("{}/v3/connect/token", nylas_api_uri()))
        .json(&json!({
            "client_id": client_id,
            "redirect_uri": NYLAS_CALLBACK_URI,
            "grant_type": "authorization_code",
            "code": code,
            "code_verifier": verifier,
        }))
        .send()
        .map_err(|error| AppError::Network(error.to_string()))?;
    let status = response.status();
    let body = response
        .text()
        .map_err(|error| AppError::Network(error.to_string()))?;
    if !status.is_success() {
        return Err(AppError::Auth(response_error_message(
            "Nylas", status, &body,
        )));
    }
    serde_json::from_str(&body).map_err(|error| AppError::Auth(error.to_string()))
}

#[tauri::command]
async fn account_connect_provider(
    state: tauri::State<'_, AppState>,
    provider: String,
) -> CommandResult<MailAccount> {
    let provider = Provider::from_input(&provider)?;
    if matches!(provider, Provider::Icloud) {
        return Err(AppError::UnsupportedProvider(
            "iCloud uses the secure server connector".to_string(),
        ));
    }
    let provider_for_oauth = provider;
    let token =
        tauri::async_runtime::spawn_blocking(move || complete_nylas_oauth(provider_for_oauth))
            .await
            .map_err(|error| AppError::Auth(error.to_string()))??;

    keyring::Entry::new(SERVICE_NAME, &format!("nylas-access:{}", token.grant_id))?
        .set_password(&token.access_token)?;
    if let Err(error) =
        keyring::Entry::new(SERVICE_NAME, &format!("nylas-refresh:{}", token.grant_id))?
            .set_password(&token.refresh_token)
    {
        let _ = keyring::Entry::new(SERVICE_NAME, &format!("nylas-access:{}", token.grant_id))?
            .delete_credential();
        return Err(error.into());
    }

    let display_name = match provider {
        Provider::Gmail => "Gmail",
        Provider::Outlook => "Outlook",
        Provider::Icloud => "iCloud Mail",
    }
    .to_string();
    let account = MailAccount {
        id: token.grant_id,
        provider,
        email: token.email,
        display_name,
        sync_status: "idle".to_string(),
        last_synced_at: None,
    };
    let conn = state.db.lock().expect("database mutex poisoned");
    let inserted = conn.execute(
        "INSERT INTO accounts (id, provider, email, display_name, sync_status)
         VALUES (?1, ?2, ?3, ?4, 'idle')
         ON CONFLICT(id) DO UPDATE SET email = excluded.email, display_name = excluded.display_name, sync_status = 'idle'",
        params![
            account.id,
            account.provider.as_str(),
            account.email,
            account.display_name
        ],
    );
    if let Err(error) = inserted {
        for prefix in ["nylas-access", "nylas-refresh"] {
            if let Ok(entry) =
                keyring::Entry::new(SERVICE_NAME, &format!("{prefix}:{}", account.id))
            {
                let _ = entry.delete_credential();
            }
        }
        return Err(error.into());
    }
    Ok(account)
}

fn complete_nylas_icloud(email: &str, app_password: &str) -> CommandResult<NylasGrantResponse> {
    let url = std::env::var("ICLOUD_CONNECT_URL")
        .unwrap_or_else(|_| DEFAULT_ICLOUD_CONNECT_URL.to_string());
    let response = Client::new()
        .post(url)
        .json(&json!({ "email": email, "appPassword": app_password }))
        .send()
        .map_err(|error| AppError::Network(error.to_string()))?;
    let status = response.status();
    let body = response
        .text()
        .map_err(|error| AppError::Network(error.to_string()))?;
    if !status.is_success() {
        return Err(AppError::Auth(response_error_message(
            "iCloud connector",
            status,
            &body,
        )));
    }
    serde_json::from_str(&body).map_err(|error| AppError::Auth(error.to_string()))
}

fn icloud_worker_url(path: &str) -> String {
    let connect_url = std::env::var("ICLOUD_CONNECT_URL")
        .unwrap_or_else(|_| DEFAULT_ICLOUD_CONNECT_URL.to_string());
    format!(
        "{}/icloud/{}",
        connect_url
            .trim_end_matches('/')
            .trim_end_matches("/icloud/connect"),
        path.trim_start_matches('/')
    )
}

fn icloud_access_token(account_id: &str) -> CommandResult<String> {
    keyring::Entry::new(SERVICE_NAME, &format!("icloud-access:{account_id}"))?
        .get_password()
        .map_err(|error| match error {
            keyring::Error::NoEntry => AppError::Auth(
                "Reconnect this iCloud account once to enable mailbox sync".to_string(),
            ),
            other => other.into(),
        })
}

fn get_icloud_worker<T: DeserializeOwned>(path: &str, access_token: &str) -> CommandResult<T> {
    let response = Client::new()
        .get(icloud_worker_url(path))
        .bearer_auth(access_token)
        .send()
        .map_err(|error| AppError::Network(error.to_string()))?;
    let status = response.status();
    let body = response
        .text()
        .map_err(|error| AppError::Network(error.to_string()))?;
    if !status.is_success() {
        return Err(AppError::Auth(response_error_message(
            "iCloud connector",
            status,
            &body,
        )));
    }
    serde_json::from_str(&body).map_err(|error| AppError::Auth(error.to_string()))
}

fn apply_icloud_message_action(
    message_id: &str,
    action: &str,
    access_token: &str,
) -> CommandResult<()> {
    let hard_delete = if action == "delete" { "?hard=true" } else { "" };
    let response = Client::new()
        .delete(icloud_worker_url(&format!(
            "messages/{}{}",
            urlencoding::encode(message_id),
            hard_delete,
        )))
        .bearer_auth(access_token)
        .send()
        .map_err(|error| AppError::Network(error.to_string()))?;
    let status = response.status();
    let body = response
        .text()
        .map_err(|error| AppError::Network(error.to_string()))?;
    if status.is_success() {
        Ok(())
    } else {
        Err(AppError::Auth(response_error_message(
            "iCloud connector",
            status,
            &body,
        )))
    }
}

fn flush_pending_mail_actions(app: &AppHandle) {
    let state = app.state::<AppState>();
    let Ok(_outbox) = state.outbox.try_lock() else {
        return;
    };
    let pending = {
        let conn = state.db.lock().expect("database mutex poisoned");
        let Ok(mut stmt) = conn.prepare(
            "SELECT account_id, provider_message_id, action
             FROM pending_mail_actions
             ORDER BY created_at",
        ) else {
            return;
        };
        let Ok(rows) = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        }) else {
            return;
        };
        rows.filter_map(Result::ok).collect::<Vec<_>>()
    };

    for (account_id, message_id, action) in pending {
        let result = icloud_access_token(&account_id)
            .and_then(|token| apply_icloud_message_action(&message_id, &action, &token));
        if result.is_ok() {
            let conn = state.db.lock().expect("database mutex poisoned");
            let _ = conn.execute(
                "DELETE FROM pending_mail_actions
                 WHERE account_id = ?1 AND provider_message_id = ?2 AND action = ?3",
                params![account_id, message_id, action],
            );
        }
    }
}

#[tauri::command]
async fn account_connect_icloud(
    state: tauri::State<'_, AppState>,
    email: String,
    app_password: String,
) -> CommandResult<MailAccount> {
    if !email.contains('@') || email.len() > 254 {
        return Err(AppError::InvalidInput(
            "enter a valid iCloud email address".to_string(),
        ));
    }
    if app_password.trim().is_empty() || app_password.len() > 128 {
        return Err(AppError::InvalidInput(
            "enter an Apple app-specific password".to_string(),
        ));
    }

    let grant = tauri::async_runtime::spawn_blocking(move || {
        complete_nylas_icloud(email.trim(), app_password.trim())
    })
    .await
    .map_err(|error| AppError::Auth(error.to_string()))??;
    let account = MailAccount {
        id: grant.grant_id,
        provider: Provider::Icloud,
        email: grant.email,
        display_name: "iCloud Mail".to_string(),
        sync_status: "idle".to_string(),
        last_synced_at: None,
    };
    keyring::Entry::new(SERVICE_NAME, &format!("icloud-access:{}", account.id))?
        .set_password(&grant.access_token)?;
    let conn = state.db.lock().expect("database mutex poisoned");
    let inserted = conn.execute(
        "INSERT INTO accounts (id, provider, email, display_name, sync_status)
         VALUES (?1, 'icloud', ?2, ?3, 'idle')
         ON CONFLICT(id) DO UPDATE SET email = excluded.email, display_name = excluded.display_name, sync_status = 'idle'",
        params![account.id, account.email, account.display_name],
    );
    if let Err(error) = inserted {
        let _ = keyring::Entry::new(SERVICE_NAME, &format!("icloud-access:{}", account.id))?
            .delete_credential();
        return Err(error.into());
    }
    Ok(account)
}

#[tauri::command]
fn account_remove(state: tauri::State<AppState>, account_id: String) -> CommandResult<Removed> {
    let conn = state.db.lock().expect("database mutex poisoned");
    conn.execute("DELETE FROM accounts WHERE id = ?1", params![account_id])?;
    drop(conn);

    for key in [
        format!("nylas-access:{account_id}"),
        format!("nylas-refresh:{account_id}"),
        format!("icloud:{account_id}"),
        format!("icloud-access:{account_id}"),
    ] {
        if let Ok(entry) = keyring::Entry::new(SERVICE_NAME, &key) {
            // ponytail: local removal must still work if macOS refuses stale Keychain cleanup.
            let _ = entry.delete_credential();
        }
    }
    Ok(Removed { removed: true })
}

fn store_icloud_sync(
    conn: &Connection,
    account_id: &str,
    sync: IcloudSyncResponse,
) -> CommandResult<()> {
    if sync.folders.is_empty() {
        return Err(AppError::Auth(
            "iCloud folders are not ready yet; Nylas can take a few minutes after connection"
                .to_string(),
        ));
    }
    let tx = conn.unchecked_transaction()?;
    tx.execute(
        "DELETE FROM folders WHERE account_id = ?1",
        params![account_id],
    )?;
    for folder in &sync.folders {
        tx.execute(
            "INSERT INTO folders (id, account_id, name, unread_count) VALUES (?1, ?2, ?3, ?4)",
            params![
                format!("{account_id}:{}", folder.id),
                account_id,
                folder.name,
                folder.unread_count
            ],
        )?;
    }
    for message in sync.messages {
        if !sync
            .folders
            .iter()
            .any(|folder| folder.id == message.folder_id)
        {
            continue;
        }
        let message_id = format!("{account_id}:{}", message.id);
        tx.execute(
            "INSERT INTO messages (id, folder_id, account_id, provider_message_id, sender, recipients_json, subject, preview, received_at, has_attachments, is_unread) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                message_id,
                format!("{account_id}:{}", message.folder_id),
                account_id,
                message.id,
                message.sender,
                serde_json::to_string(&message.recipients).unwrap_or_else(|_| "[]".to_string()),
                message.subject,
                message.preview,
                message.received_at,
                if message.has_attachments { 1_i64 } else { 0_i64 },
                if message.is_unread { 1_i64 } else { 0_i64 },
            ],
        )?;
        tx.execute(
            "INSERT INTO message_bodies (message_id, body_html, body_text) VALUES (?1, '', '')",
            params![message_id],
        )?;
    }
    apply_pending_mail_actions(&tx)?;
    refresh_folder_unread_counts(&tx)?;
    tx.execute(
        "UPDATE accounts SET sync_status = 'idle', last_synced_at = ?1 WHERE id = ?2",
        params![Utc::now().to_rfc3339(), account_id],
    )?;
    tx.commit()?;
    Ok(())
}

#[tauri::command]
async fn sync_account(
    state: tauri::State<'_, AppState>,
    account_id: String,
) -> CommandResult<MailAccount> {
    let provider = {
        let conn = state.db.lock().expect("database mutex poisoned");
        find_account(&conn, &account_id)?.provider
    };
    if matches!(provider, Provider::Icloud) {
        let token = icloud_access_token(&account_id)?;
        let sync = tauri::async_runtime::spawn_blocking(move || {
            get_icloud_worker::<IcloudSyncResponse>("sync", &token)
        })
        .await
        .map_err(|error| AppError::Network(error.to_string()))??;
        let conn = state.db.lock().expect("database mutex poisoned");
        store_icloud_sync(&conn, &account_id, sync)?;
        return find_account(&conn, &account_id);
    }
    let conn = state.db.lock().expect("database mutex poisoned");
    conn.execute(
        "UPDATE accounts SET sync_status = 'idle', last_synced_at = ?1 WHERE id = ?2",
        params![Utc::now().to_rfc3339(), account_id],
    )?;
    find_account(&conn, &account_id)
}

#[tauri::command]
async fn sync_all(state: tauri::State<'_, AppState>) -> CommandResult<Vec<MailAccount>> {
    let accounts = {
        let conn = state.db.lock().expect("database mutex poisoned");
        list_accounts_from_db(&conn)?
    };
    for account in accounts {
        if matches!(account.provider, Provider::Icloud) {
            let token = icloud_access_token(&account.id)?;
            let sync = tauri::async_runtime::spawn_blocking(move || {
                get_icloud_worker::<IcloudSyncResponse>("sync", &token)
            })
            .await
            .map_err(|error| AppError::Network(error.to_string()))??;
            let conn = state.db.lock().expect("database mutex poisoned");
            store_icloud_sync(&conn, &account.id, sync)?;
        }
    }
    let conn = state.db.lock().expect("database mutex poisoned");
    list_accounts_from_db(&conn)
}

#[tauri::command]
fn list_accounts(state: tauri::State<AppState>) -> CommandResult<Vec<MailAccount>> {
    let conn = state.db.lock().expect("database mutex poisoned");
    list_accounts_from_db(&conn)
}

#[tauri::command]
fn list_folders(state: tauri::State<AppState>, account_id: String) -> CommandResult<Vec<Folder>> {
    let conn = state.db.lock().expect("database mutex poisoned");
    let mut stmt = conn.prepare("SELECT id, account_id, name, unread_count FROM folders WHERE account_id = ?1 ORDER BY name = 'Inbox' DESC, name ASC")?;
    let folders = stmt
        .query_map(params![account_id], |row| {
            Ok(Folder {
                id: row.get(0)?,
                account_id: row.get(1)?,
                name: row.get(2)?,
                unread_count: row.get(3)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(folders)
}

#[tauri::command]
fn list_messages(
    state: tauri::State<AppState>,
    folder_id: String,
    query: String,
) -> CommandResult<Vec<MessageSummary>> {
    let conn = state.db.lock().expect("database mutex poisoned");
    let pattern = format!("%{}%", query);
    let messages = if let Some(folder_names) = aggregate_folder_names(&folder_id) {
        let mut stmt = conn.prepare(&format!(
            "SELECT m.id, m.folder_id, m.account_id, m.provider_message_id, m.sender, m.recipients_json, m.subject, m.preview, m.received_at, m.has_attachments, m.is_unread
             FROM messages m
             JOIN folders f ON f.id = m.folder_id
             WHERE lower(f.name) IN ({folder_names}) AND (?1 = '%%' OR m.sender LIKE ?1 OR m.subject LIKE ?1 OR m.preview LIKE ?1)
             ORDER BY m.received_at DESC",
        ))?;
        let rows = stmt.query_map(params![pattern], row_to_message_summary)?;
        rows.collect::<Result<Vec<_>, _>>()?
    } else {
        let mut stmt = conn.prepare(
            "SELECT id, folder_id, account_id, provider_message_id, sender, recipients_json, subject, preview, received_at, has_attachments, is_unread
             FROM messages
             WHERE folder_id = ?1 AND (?2 = '%%' OR sender LIKE ?2 OR subject LIKE ?2 OR preview LIKE ?2)
             ORDER BY received_at DESC",
        )?;
        let rows = stmt.query_map(params![folder_id, pattern], row_to_message_summary)?;
        rows.collect::<Result<Vec<_>, _>>()?
    };
    Ok(messages)
}

fn aggregate_folder_names(folder_id: &str) -> Option<&'static str> {
    match folder_id {
        "root:inbox" => Some("'inbox'"),
        "root:trash" => Some("'trash', 'deleted messages', '刪除的郵件'"),
        "root:spam" => Some("'spam', 'junk', '垃圾郵件'"),
        _ => None,
    }
}

#[tauri::command]
fn mark_messages_read(
    state: tauri::State<AppState>,
    message_ids: Vec<String>,
) -> CommandResult<CountResult> {
    if message_ids.is_empty() {
        return Ok(CountResult { count: 0 });
    }

    let conn = state.db.lock().expect("database mutex poisoned");
    let tx = conn.unchecked_transaction()?;
    let mut updated = 0;
    {
        let mut stmt =
            tx.prepare("UPDATE messages SET is_unread = 0 WHERE id = ?1 AND is_unread = 1")?;
        for message_id in &message_ids {
            updated += stmt.execute(params![message_id])?;
        }
    }
    refresh_folder_unread_counts(&tx)?;
    tx.commit()?;
    Ok(CountResult { count: updated })
}

#[tauri::command]
fn mark_messages_unread(
    state: tauri::State<AppState>,
    message_ids: Vec<String>,
) -> CommandResult<CountResult> {
    if message_ids.is_empty() {
        return Ok(CountResult { count: 0 });
    }

    let conn = state.db.lock().expect("database mutex poisoned");
    let tx = conn.unchecked_transaction()?;
    let mut updated = 0;
    {
        let mut stmt =
            tx.prepare("UPDATE messages SET is_unread = 1 WHERE id = ?1 AND is_unread = 0")?;
        for message_id in &message_ids {
            updated += stmt.execute(params![message_id])?;
        }
    }
    refresh_folder_unread_counts(&tx)?;
    tx.commit()?;
    Ok(CountResult { count: updated })
}

fn apply_pending_mail_actions(conn: &Connection) -> CommandResult<()> {
    conn.execute(
        "UPDATE messages
         SET folder_id = (
           SELECT trash.id FROM folders trash
           WHERE trash.account_id = messages.account_id
             AND lower(trash.name) IN ('trash', 'deleted messages', '刪除的郵件')
           LIMIT 1
         )
         WHERE EXISTS (
           SELECT 1 FROM pending_mail_actions pending
           WHERE pending.account_id = messages.account_id
             AND pending.provider_message_id = messages.provider_message_id
             AND pending.action = 'trash'
         )
           AND EXISTS (
             SELECT 1 FROM folders trash
             WHERE trash.account_id = messages.account_id
               AND lower(trash.name) IN ('trash', 'deleted messages', '刪除的郵件')
           )",
        [],
    )?;
    conn.execute(
        "DELETE FROM messages
         WHERE EXISTS (
           SELECT 1 FROM pending_mail_actions pending
           WHERE pending.account_id = messages.account_id
             AND pending.provider_message_id = messages.provider_message_id
             AND pending.action = 'delete'
         )",
        [],
    )?;
    Ok(())
}

fn queue_message_deletions(conn: &Connection, message_ids: &[String]) -> CommandResult<usize> {
    let tx = conn.unchecked_transaction()?;
    let mut changed = 0;
    for message_id in message_ids {
        let target = tx
            .query_row(
                "SELECT m.account_id, m.provider_message_id, a.provider,
                        lower(f.name) IN ('trash', 'deleted messages', '刪除的郵件')
                 FROM messages m
                 JOIN accounts a ON a.id = m.account_id
                 JOIN folders f ON f.id = m.folder_id
                 WHERE m.id = ?1",
                params![message_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, bool>(3)?,
                    ))
                },
            )
            .optional()?;
        let Some((account_id, provider_message_id, provider, in_trash)) = target else {
            continue;
        };
        if provider == "icloud" {
            tx.execute(
                "INSERT INTO pending_mail_actions
                   (account_id, provider_message_id, action, created_at)
                 VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(account_id, provider_message_id)
                 DO UPDATE SET action = excluded.action, created_at = excluded.created_at",
                params![
                    account_id,
                    provider_message_id,
                    if in_trash { "delete" } else { "trash" },
                    Utc::now().to_rfc3339(),
                ],
            )?;
        }
        changed += if in_trash {
            tx.execute("DELETE FROM messages WHERE id = ?1", params![message_id])?
        } else {
            tx.execute(
                "UPDATE messages
                 SET folder_id = (
                   SELECT id FROM folders
                   WHERE account_id = ?1
                     AND lower(name) IN ('trash', 'deleted messages', '刪除的郵件')
                   LIMIT 1
                 )
                 WHERE id = ?2
                   AND EXISTS (
                     SELECT 1 FROM folders
                     WHERE account_id = ?1
                       AND lower(name) IN ('trash', 'deleted messages', '刪除的郵件')
                   )",
                params![account_id, message_id],
            )?
        };
    }
    refresh_folder_unread_counts(&tx)?;
    tx.commit()?;
    Ok(changed)
}

#[tauri::command]
fn delete_messages(
    app: AppHandle,
    state: tauri::State<'_, AppState>,
    message_ids: Vec<String>,
) -> CommandResult<CountResult> {
    if message_ids.is_empty() {
        return Ok(CountResult { count: 0 });
    }

    let conn = state.db.lock().expect("database mutex poisoned");
    let changed = queue_message_deletions(&conn, &message_ids)?;
    drop(conn);
    thread::spawn(move || flush_pending_mail_actions(&app));
    Ok(CountResult { count: changed })
}

#[tauri::command]
fn mark_messages_spam(
    state: tauri::State<AppState>,
    message_ids: Vec<String>,
) -> CommandResult<CountResult> {
    if message_ids.is_empty() {
        return Ok(CountResult { count: 0 });
    }

    let conn = state.db.lock().expect("database mutex poisoned");
    let tx = conn.unchecked_transaction()?;
    let mut changed = 0;
    {
        let mut stmt = tx.prepare(
            "UPDATE messages
             SET folder_id = (
               SELECT spam.id
               FROM folders spam
               WHERE spam.account_id = messages.account_id AND lower(spam.name) IN ('spam', 'junk')
               LIMIT 1
             )
             WHERE id = ?1
               AND EXISTS (
                 SELECT 1
                 FROM folders spam
                 WHERE spam.account_id = messages.account_id AND lower(spam.name) IN ('spam', 'junk')
               )
               AND folder_id <> (
                 SELECT spam.id
                 FROM folders spam
                 WHERE spam.account_id = messages.account_id AND lower(spam.name) IN ('spam', 'junk')
                 LIMIT 1
               )",
        )?;
        for message_id in &message_ids {
            changed += stmt.execute(params![message_id])?;
        }
    }
    refresh_folder_unread_counts(&tx)?;
    tx.commit()?;
    Ok(CountResult { count: changed })
}

fn refresh_folder_unread_counts(conn: &Connection) -> CommandResult<()> {
    conn.execute(
        "UPDATE folders
         SET unread_count = (
           SELECT COUNT(*)
           FROM messages
           WHERE messages.folder_id = folders.id AND messages.is_unread = 1
         )",
        [],
    )?;
    Ok(())
}

fn get_message_from_db(conn: &Connection, message_id: &str) -> CommandResult<MessageDetail> {
    let summary = conn.query_row(
        "SELECT id, folder_id, account_id, provider_message_id, sender, recipients_json, subject, preview, received_at, has_attachments, is_unread FROM messages WHERE id = ?1",
        params![message_id],
        row_to_message_summary,
    )?;
    let (body_html, body_text): (String, String) = conn.query_row(
        "SELECT body_html, body_text FROM message_bodies WHERE message_id = ?1",
        params![summary.id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    let mut stmt = conn.prepare("SELECT id, file_name, mime_type, byte_size FROM attachments WHERE message_id = ?1 ORDER BY file_name ASC")?;
    let attachments = stmt
        .query_map(params![summary.id], |row| {
            Ok(Attachment {
                id: row.get(0)?,
                file_name: row.get(1)?,
                mime_type: row.get(2)?,
                byte_size: row.get(3)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(MessageDetail {
        summary,
        body_html,
        body_text,
        attachments,
    })
}

#[tauri::command]
async fn get_message(
    state: tauri::State<'_, AppState>,
    message_id: String,
) -> CommandResult<MessageDetail> {
    let detail = {
        let conn = state.db.lock().expect("database mutex poisoned");
        get_message_from_db(&conn, &message_id)?
    };
    if !detail.body_html.is_empty() || !detail.body_text.is_empty() {
        return Ok(detail);
    }

    let provider = {
        let conn = state.db.lock().expect("database mutex poisoned");
        find_account(&conn, &detail.summary.account_id)?.provider
    };
    if !matches!(provider, Provider::Icloud) {
        return Ok(detail);
    }

    let token = icloud_access_token(&detail.summary.account_id)?;
    let path = format!(
        "messages/{}",
        urlencoding::encode(&detail.summary.provider_message_id)
    );
    let remote = tauri::async_runtime::spawn_blocking(move || {
        get_icloud_worker::<IcloudMessageResponse>(&path, &token)
    })
    .await
    .map_err(|error| AppError::Network(error.to_string()))??;

    let conn = state.db.lock().expect("database mutex poisoned");
    let tx = conn.unchecked_transaction()?;
    tx.execute(
        "UPDATE message_bodies SET body_html = ?1, body_text = ?2 WHERE message_id = ?3",
        params![remote.body_html, remote.body_text, message_id],
    )?;
    tx.execute(
        "DELETE FROM attachments WHERE message_id = ?1",
        params![message_id],
    )?;
    for attachment in remote.attachments {
        tx.execute(
            "INSERT INTO attachments (id, message_id, file_name, mime_type, byte_size) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![format!("{message_id}:{}", attachment.id), message_id, attachment.file_name, attachment.mime_type, attachment.byte_size],
        )?;
    }
    tx.commit()?;
    get_message_from_db(&conn, &message_id)
}

#[tauri::command]
fn set_theme(state: tauri::State<AppState>, mode: String) -> CommandResult<ThemeResult> {
    if !matches!(mode.as_str(), "system" | "light" | "dark") {
        return Err(AppError::InvalidInput(
            "theme must be system, light, or dark".to_string(),
        ));
    }

    let conn = state.db.lock().expect("database mutex poisoned");
    conn.execute(
        "INSERT INTO local_settings (key, value) VALUES ('theme', ?1) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![mode],
    )?;
    Ok(ThemeResult { mode })
}

fn list_accounts_from_db(conn: &Connection) -> CommandResult<Vec<MailAccount>> {
    let mut stmt = conn.prepare("SELECT id, provider, email, display_name, sync_status, last_synced_at FROM accounts ORDER BY provider, email")?;
    let accounts = stmt
        .query_map([], |row| {
            let provider: String = row.get(1)?;
            let last_synced_at: Option<String> = row.get(5)?;
            Ok(MailAccount {
                id: row.get(0)?,
                provider: Provider::from_input(&provider)
                    .map_err(|_| rusqlite::Error::InvalidQuery)?,
                email: row.get(2)?,
                display_name: row.get(3)?,
                sync_status: row.get(4)?,
                last_synced_at: last_synced_at
                    .and_then(|value| DateTime::parse_from_rfc3339(&value).ok())
                    .map(|value| value.with_timezone(&Utc)),
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(accounts)
}

fn find_account(conn: &Connection, account_id: &str) -> CommandResult<MailAccount> {
    conn.query_row(
        "SELECT id, provider, email, display_name, sync_status, last_synced_at FROM accounts WHERE id = ?1",
        params![account_id],
        |row| {
            let provider: String = row.get(1)?;
            let last_synced_at: Option<String> = row.get(5)?;
            Ok(MailAccount {
                id: row.get(0)?,
                provider: Provider::from_input(&provider).map_err(|_| rusqlite::Error::InvalidQuery)?,
                email: row.get(2)?,
                display_name: row.get(3)?,
                sync_status: row.get(4)?,
                last_synced_at: last_synced_at
                    .and_then(|value| DateTime::parse_from_rfc3339(&value).ok())
                    .map(|value| value.with_timezone(&Utc)),
            })
        },
    )
    .map_err(AppError::from)
}

fn row_to_message_summary(row: &rusqlite::Row<'_>) -> rusqlite::Result<MessageSummary> {
    let recipients_json: String = row.get(5)?;
    let received_at: String = row.get(8)?;
    Ok(MessageSummary {
        id: row.get(0)?,
        folder_id: row.get(1)?,
        account_id: row.get(2)?,
        provider_message_id: row.get(3)?,
        sender: row.get(4)?,
        sender_avatar_url: None,
        recipients: serde_json::from_str(&recipients_json).unwrap_or_default(),
        subject: row.get(6)?,
        preview: row.get(7)?,
        received_at: DateTime::parse_from_rfc3339(&received_at)
            .map(|value| value.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now()),
        has_attachments: row.get::<_, i64>(9)? == 1,
        is_unread: row.get::<_, i64>(10)? == 1,
    })
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
            let db_path = app_db_path(&app.handle())?;
            let db = init_database(db_path)?;
            app.manage(AppState {
                db: Mutex::new(db),
                outbox: Mutex::new(()),
            });
            let app_handle = app.handle().clone();
            thread::spawn(move || loop {
                flush_pending_mail_actions(&app_handle);
                thread::sleep(OUTBOX_SYNC_INTERVAL);
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            auth_send_email_otp,
            auth_current_session,
            auth_verify_email_otp,
            auth_logout,
            account_connect_provider,
            account_connect_icloud,
            account_remove,
            sync_account,
            sync_all,
            list_accounts,
            list_folders,
            list_messages,
            get_message,
            mark_messages_read,
            mark_messages_unread,
            delete_messages,
            mark_messages_spam,
            set_theme
        ])
        .run(context)
        .expect("error while running Shitou Mail");
}
