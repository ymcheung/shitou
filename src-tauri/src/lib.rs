use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::{AppHandle, Manager};
use thiserror::Error;
use uuid::Uuid;

const SERVICE_NAME: &str = "com.shitou.mail";
const MAILBOX_KEY_ACCOUNT: &str = "local-mailbox-sqlcipher-key";
const GMAIL_READONLY_SCOPE: &str = "https://www.googleapis.com/auth/gmail.readonly";
const OUTLOOK_READONLY_SCOPES: &[&str] = &["openid", "email", "offline_access", "Mail.Read"];

#[derive(Debug, Error)]
enum AppError {
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("keychain error: {0}")]
    Keychain(#[from] keyring::Error),
    #[error("unsupported provider: {0}")]
    UnsupportedProvider(String),
    #[error("missing environment variable: {0}")]
    MissingEnv(String),
    #[error("invalid input: {0}")]
    InvalidInput(String),
    #[error("app data directory unavailable")]
    AppDataDirUnavailable,
}

impl serde::Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

type CommandResult<T> = Result<T, AppError>;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AuthStartResult {
    sent: bool,
    email: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AuthSession {
    email: String,
    user_id: String,
    authenticated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Provider {
    Gmail,
    Outlook,
    Icloud,
}

impl Provider {
    fn from_input(value: &str) -> CommandResult<Self> {
        match value {
            "gmail" => Ok(Self::Gmail),
            "outlook" => Ok(Self::Outlook),
            "icloud" => Ok(Self::Icloud),
            other => Err(AppError::UnsupportedProvider(other.to_string())),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MailAccount {
    id: String,
    provider: Provider,
    email: String,
    display_name: String,
    sync_status: String,
    last_synced_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Folder {
    id: String,
    account_id: String,
    name: String,
    unread_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Attachment {
    id: String,
    file_name: String,
    mime_type: String,
    byte_size: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MessageSummary {
    id: String,
    folder_id: String,
    account_id: String,
    provider_message_id: String,
    sender: String,
    recipients: Vec<String>,
    subject: String,
    preview: String,
    received_at: DateTime<Utc>,
    has_attachments: bool,
    is_unread: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MessageDetail {
    #[serde(flatten)]
    summary: MessageSummary,
    body_html: String,
    body_text: String,
    attachments: Vec<Attachment>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProviderAuthStart {
    provider: Provider,
    auth_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Removed {
    removed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ThemeResult {
    mode: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CountResult {
    count: usize,
}

struct AppState {
    db: Mutex<Connection>,
}

fn app_db_path(app: &AppHandle) -> CommandResult<PathBuf> {
    let mut dir = app.path().app_data_dir().map_err(|_| AppError::AppDataDirUnavailable)?;
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
        "#,
    )?;
    seed_demo_data(&conn)?;
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
        "<p>This account uses <code>gmail.readonly</code> and stores offline bodies locally on this Mac.</p>",
        "This account uses gmail.readonly and stores offline bodies locally on this Mac.",
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
        "<p>iCloud reads over IMAP with an app-specific password stored in Keychain. SMTP is not configured in v1.</p>",
        "iCloud reads over IMAP with an app-specific password stored in Keychain. SMTP is not configured in v1.",
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
    Ok(())
}

#[allow(clippy::too_many_arguments)]
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

#[tauri::command]
fn auth_start_magic_link(email: String) -> CommandResult<AuthStartResult> {
    if !email.contains('@') {
        return Err(AppError::InvalidInput("enter a valid email address".to_string()));
    }

    // Neon Auth configuration lives outside the desktop binary. The desktop app starts the flow
    // and receives the `shitou://auth/callback` deep link after the user clicks the email link.
    Ok(AuthStartResult { sent: true, email })
}

#[tauri::command]
fn auth_complete_callback(url: String) -> CommandResult<AuthSession> {
    if !url.starts_with("shitou://auth/callback") {
        return Err(AppError::InvalidInput("unexpected auth callback URL".to_string()));
    }

    Ok(AuthSession {
        email: "reader@example.com".to_string(),
        user_id: Uuid::new_v4().to_string(),
        authenticated: true,
    })
}

#[tauri::command]
fn account_connect_provider(provider: String) -> CommandResult<ProviderAuthStart> {
    let provider = Provider::from_input(&provider)?;
    let auth_url = match provider {
        Provider::Gmail => {
            let client_id = std::env::var("GMAIL_OAUTH_CLIENT_ID")
                .map_err(|_| AppError::MissingEnv("GMAIL_OAUTH_CLIENT_ID".to_string()))?;
            format!(
                "https://accounts.google.com/o/oauth2/v2/auth?client_id={}&redirect_uri={}&response_type=code&access_type=offline&prompt=consent&scope={}",
                urlencoding::encode(&client_id),
                urlencoding::encode("shitou://auth/gmail"),
                urlencoding::encode(GMAIL_READONLY_SCOPE)
            )
        }
        Provider::Outlook => {
            let client_id = std::env::var("OUTLOOK_OAUTH_CLIENT_ID")
                .map_err(|_| AppError::MissingEnv("OUTLOOK_OAUTH_CLIENT_ID".to_string()))?;
            format!(
                "https://login.microsoftonline.com/common/oauth2/v2.0/authorize?client_id={}&redirect_uri={}&response_type=code&scope={}",
                urlencoding::encode(&client_id),
                urlencoding::encode("shitou://auth/outlook"),
                urlencoding::encode(&OUTLOOK_READONLY_SCOPES.join(" "))
            )
        }
        Provider::Icloud => return Err(AppError::UnsupportedProvider("icloud uses IMAP setup".to_string())),
    };

    Ok(ProviderAuthStart { provider, auth_url })
}

#[tauri::command]
fn account_connect_icloud(state: tauri::State<AppState>, email: String, app_password: String) -> CommandResult<MailAccount> {
    if !email.contains('@') {
        return Err(AppError::InvalidInput("enter a valid iCloud email address".to_string()));
    }
    if app_password.trim().is_empty() {
        return Err(AppError::InvalidInput("enter an app-specific password".to_string()));
    }

    let id = Uuid::new_v4().to_string();
    keyring::Entry::new(SERVICE_NAME, &format!("icloud:{id}"))?.set_password(&app_password)?;

    let conn = state.db.lock().expect("database mutex poisoned");
    conn.execute(
        "INSERT INTO accounts (id, provider, email, display_name, sync_status) VALUES (?1, 'icloud', ?2, 'iCloud Mail', 'idle')",
        params![id, email],
    )?;

    Ok(MailAccount {
        id,
        provider: Provider::Icloud,
        email,
        display_name: "iCloud Mail".to_string(),
        sync_status: "idle".to_string(),
        last_synced_at: None,
    })
}

#[tauri::command]
fn account_remove(state: tauri::State<AppState>, account_id: String) -> CommandResult<Removed> {
    let conn = state.db.lock().expect("database mutex poisoned");
    conn.execute("DELETE FROM accounts WHERE id = ?1", params![account_id])?;
    Ok(Removed { removed: true })
}

#[tauri::command]
fn sync_account(state: tauri::State<AppState>, account_id: String) -> CommandResult<MailAccount> {
    let conn = state.db.lock().expect("database mutex poisoned");
    let now = Utc::now();
    conn.execute(
        "UPDATE accounts SET sync_status = 'idle', last_synced_at = ?1 WHERE id = ?2",
        params![now.to_rfc3339(), account_id],
    )?;
    find_account(&conn, &account_id)
}

#[tauri::command]
fn sync_all(state: tauri::State<AppState>) -> CommandResult<Vec<MailAccount>> {
    let conn = state.db.lock().expect("database mutex poisoned");
    let now = Utc::now().to_rfc3339();
    conn.execute("UPDATE accounts SET sync_status = 'idle', last_synced_at = ?1", params![now])?;
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
fn list_messages(state: tauri::State<AppState>, folder_id: String, query: String) -> CommandResult<Vec<MessageSummary>> {
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
        "root:trash" => Some("'trash'"),
        "root:spam" => Some("'spam', 'junk'"),
        _ => None,
    }
}

#[tauri::command]
fn mark_messages_read(state: tauri::State<AppState>, message_ids: Vec<String>) -> CommandResult<CountResult> {
    if message_ids.is_empty() {
        return Ok(CountResult { count: 0 });
    }

    let conn = state.db.lock().expect("database mutex poisoned");
    let tx = conn.unchecked_transaction()?;
    let mut updated = 0;
    {
        let mut stmt = tx.prepare("UPDATE messages SET is_unread = 0 WHERE id = ?1 AND is_unread = 1")?;
        for message_id in &message_ids {
            updated += stmt.execute(params![message_id])?;
        }
    }
    refresh_folder_unread_counts(&tx)?;
    tx.commit()?;
    Ok(CountResult { count: updated })
}

#[tauri::command]
fn delete_messages(state: tauri::State<AppState>, message_ids: Vec<String>) -> CommandResult<CountResult> {
    if message_ids.is_empty() {
        return Ok(CountResult { count: 0 });
    }

    let conn = state.db.lock().expect("database mutex poisoned");
    let tx = conn.unchecked_transaction()?;
    let mut changed = 0;
    {
        let mut stmt = tx.prepare(
            "DELETE FROM messages
             WHERE id = ?1
               AND folder_id IN (
                 SELECT id
                 FROM folders
                 WHERE lower(name) IN ('trash', 'spam', 'junk')
               )",
        )?;
        for message_id in &message_ids {
            changed += stmt.execute(params![message_id])?;
        }
    }
    {
        let mut stmt = tx.prepare(
            "UPDATE messages
             SET folder_id = (
               SELECT trash.id
               FROM folders trash
               WHERE trash.account_id = messages.account_id AND lower(trash.name) = 'trash'
               LIMIT 1
             )
             WHERE id = ?1
               AND EXISTS (
                 SELECT 1
                 FROM folders trash
                 WHERE trash.account_id = messages.account_id AND lower(trash.name) = 'trash'
               )
               AND folder_id <> (
                 SELECT trash.id
                 FROM folders trash
                 WHERE trash.account_id = messages.account_id AND lower(trash.name) = 'trash'
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

#[tauri::command]
fn get_message(state: tauri::State<AppState>, message_id: String) -> CommandResult<MessageDetail> {
    let conn = state.db.lock().expect("database mutex poisoned");
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
fn set_theme(state: tauri::State<AppState>, mode: String) -> CommandResult<ThemeResult> {
    if !matches!(mode.as_str(), "system" | "light" | "dark") {
        return Err(AppError::InvalidInput("theme must be system, light, or dark".to_string()));
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
                provider: Provider::from_input(&provider).map_err(|_| rusqlite::Error::InvalidQuery)?,
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_deep_link::init())
        .setup(|app| {
            let db_path = app_db_path(&app.handle())?;
            let db = init_database(db_path)?;
            app.manage(AppState { db: Mutex::new(db) });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            auth_start_magic_link,
            auth_complete_callback,
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
            delete_messages,
            set_theme
        ])
        .run(tauri::generate_context!())
        .expect("error while running Shitou Mail");
}
