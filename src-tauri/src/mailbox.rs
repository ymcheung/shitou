use std::path::PathBuf;
use std::thread;

use chrono::{DateTime, Duration, Utc};
use reqwest::blocking::Client;
use rusqlite::{params, Connection, OptionalExtension};
use serde::de::DeserializeOwned;
use serde::Deserialize;
use tauri::{AppHandle, Manager};

use crate::accounts::icloud_access_token;
use crate::error::{response_error_message, AppError, CommandResult};
use crate::models::{
    Attachment, CountResult, Folder, MailAccount, MessageDetail, MessageSummary, NotificationList,
    NotificationSetting, NotificationSummary, Provider, ThemeResult,
};
use crate::notifications::{classify, duration_seconds, looks_relevant, redact_expired};
use crate::state::AppState;

const SERVICE_NAME: &str = "com.shitou.mail";
const MAILBOX_KEY_ACCOUNT: &str = "local-mailbox-sqlcipher-key";
const DEFAULT_ICLOUD_CONNECT_URL: &str =
    "https://shitou-icloud-connect.shitou-mail-cloud.workers.dev/icloud/connect";

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncedFolder {
    pub id: String,
    pub name: String,
    pub unread_count: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncedMessage {
    pub id: String,
    pub folder_id: String,
    pub sender: String,
    pub recipients: Vec<String>,
    pub subject: String,
    pub preview: String,
    pub received_at: String,
    pub has_attachments: bool,
    pub is_unread: bool,
}

#[derive(Deserialize)]
pub struct SyncedMailbox {
    pub folders: Vec<SyncedFolder>,
    pub messages: Vec<SyncedMessage>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncedMessageBody {
    pub body_html: String,
    pub body_text: String,
    pub attachments: Vec<Attachment>,
}

pub struct NotificationBodyCandidate {
    pub message_id: String,
    pub account_id: String,
    pub provider_message_id: String,
}

pub struct Mailbox {
    conn: Connection,
}

impl Mailbox {
    pub fn open_app(path: PathBuf) -> CommandResult<Self> {
        let entry = keyring::Entry::new(SERVICE_NAME, MAILBOX_KEY_ACCOUNT)?;
        let key = match entry.get_password() {
            Ok(key) => key,
            Err(keyring::Error::NoEntry) => {
                let key = uuid::Uuid::new_v4().to_string();
                entry.set_password(&key)?;
                key
            }
            Err(error) => return Err(AppError::Keychain(error)),
        };
        Self::open(path, key)
    }

    fn open(path: PathBuf, key: String) -> CommandResult<Self> {
        let mailbox = Self {
            conn: Connection::open(path)?,
        };
        mailbox.conn.pragma_update(None, "key", key)?;
        mailbox.initialize_schema()?;
        mailbox.remove_demo_seed_data()?;
        Ok(mailbox)
    }

    #[cfg(test)]
    pub fn in_memory() -> CommandResult<Self> {
        let mailbox = Self {
            conn: Connection::open_in_memory()?,
        };
        mailbox.initialize_schema()?;
        Ok(mailbox)
    }

    fn initialize_schema(&self) -> CommandResult<()> {
        self.conn.execute_batch(
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

            UPDATE accounts
            SET sync_status = 'unsupported', last_synced_at = NULL
            WHERE provider IN ('gmail', 'outlook');
            "#,
        )?;
        self.migrate_schema()?;
        Ok(())
    }

    fn migrate_schema(&self) -> CommandResult<()> {
        let version = self
            .conn
            .query_row("PRAGMA user_version", [], |row| row.get::<_, i64>(0))?;
        if version < 1 {
            self.conn.execute_batch(
                r#"
                CREATE TABLE IF NOT EXISTS mail_notifications (
                  account_id TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
                  provider_message_id TEXT NOT NULL,
                  message_id TEXT NOT NULL,
                  sender TEXT NOT NULL,
                  subject TEXT NOT NULL,
                  preview TEXT NOT NULL,
                  received_at TEXT NOT NULL,
                  body_html TEXT NOT NULL DEFAULT '',
                  body_text TEXT NOT NULL DEFAULT '',
                  is_unread INTEGER NOT NULL DEFAULT 0,
                  kind TEXT NOT NULL CHECK (kind IN ('access', 'security')),
                  code TEXT,
                  expires_at TEXT,
                  summarized_at TEXT,
                  hidden_at TEXT,
                  seen_at TEXT,
                  dismissed_at TEXT,
                  restored INTEGER NOT NULL DEFAULT 0,
                  reason TEXT NOT NULL,
                  PRIMARY KEY (account_id, provider_message_id)
                );
                PRAGMA user_version = 1;
                "#,
            )?;
        }
        Ok(())
    }

    fn remove_demo_seed_data(&self) -> CommandResult<()> {
        self.conn.execute(
            "DELETE FROM accounts
             WHERE (id = 'acc-gmail' AND email = 'reader@gmail.com')
                OR (id = 'acc-icloud' AND email = 'reader@icloud.com')",
            [],
        )?;
        Ok(())
    }

    pub fn setting(&self, key: &str) -> CommandResult<Option<String>> {
        Ok(self
            .conn
            .query_row(
                "SELECT value FROM local_settings WHERE key = ?1",
                params![key],
                |row| row.get(0),
            )
            .optional()?)
    }

    pub fn set_setting(&self, key: &str, value: &str) -> CommandResult<()> {
        self.conn.execute(
            "INSERT INTO local_settings (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    pub fn remove_setting(&self, key: &str) -> CommandResult<()> {
        self.conn
            .execute("DELETE FROM local_settings WHERE key = ?1", params![key])?;
        Ok(())
    }

    pub fn upsert_account(&self, account: &MailAccount) -> CommandResult<()> {
        self.conn.execute(
            "INSERT INTO accounts (id, provider, email, display_name, sync_status)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(id) DO UPDATE SET email = excluded.email, display_name = excluded.display_name, sync_status = excluded.sync_status",
            params![
                account.id,
                account.provider.as_str(),
                account.email,
                account.display_name,
                account.sync_status,
            ],
        )?;
        Ok(())
    }

    pub fn remove_account(&self, account_id: &str) -> CommandResult<()> {
        self.conn
            .execute("DELETE FROM accounts WHERE id = ?1", params![account_id])?;
        Ok(())
    }

    pub fn list_accounts(&self) -> CommandResult<Vec<MailAccount>> {
        let mut stmt = self.conn.prepare("SELECT id, provider, email, display_name, sync_status, last_synced_at FROM accounts ORDER BY provider, email")?;
        let accounts = stmt
            .query_map([], account_from_row)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(accounts)
    }

    pub fn find_account(&self, account_id: &str) -> CommandResult<MailAccount> {
        self.conn
            .query_row(
                "SELECT id, provider, email, display_name, sync_status, last_synced_at FROM accounts WHERE id = ?1",
                params![account_id],
                account_from_row,
            )
            .map_err(AppError::from)
    }

    pub fn store_sync(&mut self, account_id: &str, sync: SyncedMailbox) -> CommandResult<()> {
        if sync.folders.is_empty() {
            return Err(AppError::Auth(
                "iCloud folders are not ready yet; Nylas can take a few minutes after connection"
                    .to_string(),
            ));
        }
        let tx = self.conn.transaction()?;
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
                    i64::from(message.has_attachments),
                    i64::from(message.is_unread),
                ],
            )?;
            tx.execute(
                "INSERT INTO message_bodies (message_id, body_html, body_text) VALUES (?1, '', '')",
                params![message_id],
            )?;
        }
        restore_visible_notification_messages(&tx)?;
        apply_pending_mail_actions(&tx)?;
        refresh_folder_unread_counts(&tx)?;
        tx.execute(
            "UPDATE accounts SET sync_status = 'idle', last_synced_at = ?1 WHERE id = ?2",
            params![Utc::now().to_rfc3339(), account_id],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn list_folders(&self, account_id: &str) -> CommandResult<Vec<Folder>> {
        let mut stmt = self.conn.prepare("SELECT id, account_id, name, unread_count FROM folders WHERE account_id = ?1 ORDER BY name = 'Inbox' DESC, name ASC")?;
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

    pub fn list_messages(
        &self,
        folder_id: &str,
        query: &str,
    ) -> CommandResult<Vec<MessageSummary>> {
        let pattern = format!("%{query}%");
        let messages = if let Some(folder_names) = aggregate_folder_names(folder_id) {
            let hidden_filter = if folder_id == "root:inbox" {
                "AND NOT EXISTS (
                   SELECT 1 FROM mail_notifications n
                   WHERE n.account_id = m.account_id
                     AND n.provider_message_id = m.provider_message_id
                     AND n.hidden_at IS NOT NULL
                     AND n.restored = 0
                 )"
            } else {
                ""
            };
            let mut stmt = self.conn.prepare(&format!(
                "SELECT m.id, m.folder_id, m.account_id, m.provider_message_id, m.sender, m.recipients_json, m.subject, m.preview, m.received_at, m.has_attachments, m.is_unread
                 FROM messages m JOIN folders f ON f.id = m.folder_id
                 WHERE lower(f.name) IN ({folder_names})
                   {hidden_filter}
                   AND (?1 = '%%' OR m.sender LIKE ?1 OR m.subject LIKE ?1 OR m.preview LIKE ?1)
                 ORDER BY m.received_at DESC",
            ))?;
            let rows = stmt.query_map(params![pattern], message_summary_from_row)?;
            rows.collect::<Result<Vec<_>, _>>()?
        } else {
            let mut stmt = self.conn.prepare(
                "SELECT m.id, m.folder_id, m.account_id, m.provider_message_id, m.sender, m.recipients_json, m.subject, m.preview, m.received_at, m.has_attachments, m.is_unread
                 FROM messages m JOIN folders f ON f.id = m.folder_id
                 WHERE m.folder_id = ?1
                   AND (lower(f.name) <> 'inbox' OR NOT EXISTS (
                     SELECT 1 FROM mail_notifications n
                     WHERE n.account_id = m.account_id
                       AND n.provider_message_id = m.provider_message_id
                       AND n.hidden_at IS NOT NULL
                       AND n.restored = 0
                   ))
                   AND (?2 = '%%' OR m.sender LIKE ?2 OR m.subject LIKE ?2 OR m.preview LIKE ?2)
                 ORDER BY m.received_at DESC",
            )?;
            let rows = stmt.query_map(params![folder_id, pattern], message_summary_from_row)?;
            rows.collect::<Result<Vec<_>, _>>()?
        };
        Ok(messages)
    }

    pub fn mark_messages_read(
        &mut self,
        message_ids: &[String],
        unread: bool,
    ) -> CommandResult<usize> {
        if message_ids.is_empty() {
            return Ok(0);
        }
        let tx = self.conn.transaction()?;
        let mut updated = 0;
        {
            let mut stmt = tx.prepare(if unread {
                "UPDATE messages SET is_unread = 1 WHERE id = ?1 AND is_unread = 0"
            } else {
                "UPDATE messages SET is_unread = 0 WHERE id = ?1 AND is_unread = 1"
            })?;
            for message_id in message_ids {
                updated += stmt.execute(params![message_id])?;
            }
        }
        refresh_folder_unread_counts(&tx)?;
        tx.commit()?;
        Ok(updated)
    }

    pub fn mark_messages_spam(&mut self, message_ids: &[String]) -> CommandResult<usize> {
        if message_ids.is_empty() {
            return Ok(0);
        }
        let tx = self.conn.transaction()?;
        let mut changed = 0;
        {
            let mut stmt = tx.prepare(
                "UPDATE messages SET folder_id = (
                   SELECT spam.id FROM folders spam
                   WHERE spam.account_id = messages.account_id AND lower(spam.name) IN ('spam', 'junk') LIMIT 1
                 )
                 WHERE id = ?1
                   AND EXISTS (SELECT 1 FROM folders spam WHERE spam.account_id = messages.account_id AND lower(spam.name) IN ('spam', 'junk'))
                   AND folder_id <> (SELECT spam.id FROM folders spam WHERE spam.account_id = messages.account_id AND lower(spam.name) IN ('spam', 'junk') LIMIT 1)",
            )?;
            for message_id in message_ids {
                changed += stmt.execute(params![message_id])?;
            }
        }
        refresh_folder_unread_counts(&tx)?;
        tx.commit()?;
        Ok(changed)
    }

    pub fn get_message(&self, message_id: &str) -> CommandResult<MessageDetail> {
        let summary = self
            .conn
            .query_row(
                "SELECT id, folder_id, account_id, provider_message_id, sender, recipients_json, subject, preview, received_at, has_attachments, is_unread FROM messages WHERE id = ?1",
                params![message_id],
                message_summary_from_row,
            )
            .optional()?;
        let Some(summary) = summary else {
            return self.get_notification_message(message_id);
        };
        let (body_html, body_text) = self.conn.query_row(
            "SELECT body_html, body_text FROM message_bodies WHERE message_id = ?1",
            params![summary.id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        let mut stmt = self.conn.prepare("SELECT id, file_name, mime_type, byte_size FROM attachments WHERE message_id = ?1 ORDER BY file_name ASC")?;
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

    pub fn cache_message_body(
        &mut self,
        message_id: &str,
        body: SyncedMessageBody,
    ) -> CommandResult<MessageDetail> {
        let tx = self.conn.transaction()?;
        tx.execute(
            "UPDATE message_bodies SET body_html = ?1, body_text = ?2 WHERE message_id = ?3",
            params![body.body_html, body.body_text, message_id],
        )?;
        tx.execute(
            "UPDATE mail_notifications SET body_html = ?1, body_text = ?2 WHERE message_id = ?3",
            params![body.body_html, body.body_text, message_id],
        )?;
        tx.execute(
            "DELETE FROM attachments WHERE message_id = ?1",
            params![message_id],
        )?;
        for attachment in body.attachments {
            tx.execute(
                "INSERT INTO attachments (id, message_id, file_name, mime_type, byte_size) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![format!("{message_id}:{}", attachment.id), message_id, attachment.file_name, attachment.mime_type, attachment.byte_size],
            )?;
        }
        tx.commit()?;
        self.get_message(message_id)
    }

    fn get_notification_message(&self, message_id: &str) -> CommandResult<MessageDetail> {
        self.conn
            .query_row(
                "SELECT message_id, account_id, provider_message_id, sender, subject, preview,
                        received_at, body_html, body_text, is_unread
                 FROM mail_notifications WHERE message_id = ?1",
                params![message_id],
                |row| {
                    let received_at: String = row.get(6)?;
                    Ok(MessageDetail {
                        summary: MessageSummary {
                            id: row.get(0)?,
                            folder_id: "root:notifications".to_string(),
                            account_id: row.get(1)?,
                            provider_message_id: row.get(2)?,
                            sender: row.get(3)?,
                            sender_avatar_url: None,
                            recipients: vec![],
                            subject: row.get(4)?,
                            preview: row.get(5)?,
                            received_at: DateTime::parse_from_rfc3339(&received_at)
                                .map(|value| value.with_timezone(&Utc))
                                .unwrap_or_else(|_| Utc::now()),
                            has_attachments: false,
                            is_unread: row.get::<_, i64>(9)? == 1,
                        },
                        body_html: row.get(7)?,
                        body_text: row.get(8)?,
                        attachments: vec![],
                    })
                },
            )
            .map_err(AppError::from)
    }

    fn cache_notification_body(
        &self,
        message_id: &str,
        body: SyncedMessageBody,
    ) -> CommandResult<MessageDetail> {
        self.conn.execute(
            "UPDATE mail_notifications SET body_html = ?1, body_text = ?2 WHERE message_id = ?3",
            params![body.body_html, body.body_text, message_id],
        )?;
        self.get_notification_message(message_id)
    }

    pub fn notifications_enabled(&self) -> CommandResult<bool> {
        Ok(self.setting("notifications_enabled")?.as_deref() != Some("false"))
    }

    pub fn process_notifications(
        &mut self,
        now: DateTime<Utc>,
        summarize_security: bool,
    ) -> CommandResult<Vec<NotificationBodyCandidate>> {
        self.process_notifications_inner(now, summarize_security, false)
    }

    pub fn discover_notification_bodies(
        &mut self,
        now: DateTime<Utc>,
        summarize_security: bool,
    ) -> CommandResult<Vec<NotificationBodyCandidate>> {
        self.process_notifications_inner(now, summarize_security, true)
    }

    fn process_notifications_inner(
        &mut self,
        now: DateTime<Utc>,
        summarize_security: bool,
        defer_missing_body: bool,
    ) -> CommandResult<Vec<NotificationBodyCandidate>> {
        if !self.notifications_enabled()? {
            return Ok(vec![]);
        }

        let cutoff = (now - Duration::days(7)).to_rfc3339();
        let mut stmt = self.conn.prepare(
            "SELECT m.id, m.account_id, m.provider_message_id, m.sender, m.subject, m.preview,
                    m.received_at, b.body_html, b.body_text, m.is_unread
             FROM messages m
             JOIN folders f ON f.id = m.folder_id
             JOIN accounts a ON a.id = m.account_id
             JOIN message_bodies b ON b.message_id = m.id
             WHERE lower(f.name) = 'inbox'
               AND a.provider = 'icloud'
               AND julianday(m.received_at) >= julianday(?1)
               AND NOT EXISTS (
                 SELECT 1 FROM mail_notifications n
                 WHERE n.account_id = m.account_id
                   AND n.provider_message_id = m.provider_message_id
               )",
        )?;
        let candidates = stmt
            .query_map(params![cutoff], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, String>(7)?,
                    row.get::<_, String>(8)?,
                    row.get::<_, i64>(9)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        drop(stmt);

        let tx = self.conn.transaction()?;
        let mut body_candidates = Vec::new();
        for (
            message_id,
            account_id,
            provider_message_id,
            sender,
            subject,
            preview,
            received,
            body_html,
            body_text,
            is_unread,
        ) in candidates
        {
            let Ok(received_at) =
                DateTime::parse_from_rfc3339(&received).map(|value| value.with_timezone(&Utc))
            else {
                continue;
            };
            let summary_content = format!("{sender} {subject} {preview}");
            let summary_classification = classify(&summary_content, received_at);
            let needs_body = match &summary_classification {
                Some(classification) => {
                    classification.kind == "access" && duration_seconds(&summary_content).is_none()
                }
                None => true,
            };
            if body_html.is_empty()
                && body_text.is_empty()
                && looks_relevant(&summary_content)
                && needs_body
                && defer_missing_body
            {
                body_candidates.push(NotificationBodyCandidate {
                    message_id,
                    account_id,
                    provider_message_id,
                });
                continue;
            }
            let content = format!("{summary_content} {body_text}");
            let Some(classification) = classify(&content, received_at) else {
                continue;
            };
            let (summarized_at, hidden_at) = if classification.kind == "access" {
                (
                    Some(now.to_rfc3339()),
                    classification
                        .expires_at
                        .filter(|expires_at| *expires_at <= now)
                        .map(|_| now.to_rfc3339()),
                )
            } else if summarize_security && received_at + Duration::hours(1) <= now {
                (Some(now.to_rfc3339()), Some(now.to_rfc3339()))
            } else {
                (None, None)
            };
            tx.execute(
                "INSERT INTO mail_notifications
                   (account_id, provider_message_id, message_id, sender, subject, preview,
                    received_at, body_html, body_text, is_unread, kind, code, expires_at,
                    summarized_at, hidden_at, reason)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
                params![
                    account_id,
                    provider_message_id,
                    message_id,
                    sender,
                    subject,
                    preview,
                    received,
                    body_html,
                    body_text,
                    is_unread,
                    classification.kind,
                    classification.code,
                    classification.expires_at.map(|value| value.to_rfc3339()),
                    summarized_at,
                    hidden_at,
                    classification.reason,
                ],
            )?;
        }
        tx.execute(
            "UPDATE mail_notifications
             SET hidden_at = ?1
             WHERE kind = 'access'
               AND restored = 0
               AND hidden_at IS NULL
               AND expires_at IS NOT NULL
               AND julianday(expires_at) <= julianday(?1)",
            params![now.to_rfc3339()],
        )?;
        if summarize_security {
            tx.execute(
                "UPDATE mail_notifications
                 SET summarized_at = ?1, hidden_at = ?1
                 WHERE kind = 'security'
                   AND restored = 0
                   AND hidden_at IS NULL
                   AND julianday(received_at) <= julianday(?2)",
                params![now.to_rfc3339(), (now - Duration::hours(1)).to_rfc3339()],
            )?;
        }
        refresh_folder_unread_counts(&tx)?;
        tx.commit()?;
        Ok(body_candidates)
    }

    pub fn list_notifications(
        &self,
        now: DateTime<Utc>,
        account_id: Option<&str>,
    ) -> CommandResult<Vec<NotificationSummary>> {
        let mut stmt = self.conn.prepare(
            "SELECT n.account_id || ':' || n.provider_message_id, n.message_id, n.account_id,
                    a.email, n.sender, n.subject, n.preview, n.kind, n.code, n.received_at,
                    n.expires_at, n.reason, n.seen_at
             FROM mail_notifications n
             JOIN accounts a ON a.id = n.account_id
             WHERE n.dismissed_at IS NULL
               AND n.summarized_at IS NOT NULL
               AND (?1 IS NULL OR n.account_id = ?1)
               AND ((n.kind = 'access' AND julianday(n.expires_at) > julianday(?2))
                    OR (n.kind = 'access' AND julianday(n.expires_at) > julianday(?3))
                    OR (n.kind = 'security' AND julianday(n.summarized_at) > julianday(?4)))
             ORDER BY COALESCE(n.expires_at, n.summarized_at) DESC, n.message_id ASC",
        )?;
        let now_text = now.to_rfc3339();
        let access_cutoff = (now - Duration::hours(24)).to_rfc3339();
        let security_cutoff = (now - Duration::days(7)).to_rfc3339();
        let notifications = stmt
            .query_map(
                params![account_id, now_text, access_cutoff, security_cutoff],
                |row| {
                    let kind: String = row.get(7)?;
                    let expires_at = row
                        .get::<_, Option<String>>(10)?
                        .and_then(|value| DateTime::parse_from_rfc3339(&value).ok())
                        .map(|value| value.with_timezone(&Utc));
                    let expired = expires_at.is_some_and(|value| value <= now);
                    let received_at: String = row.get(9)?;
                    let code = row.get::<_, Option<String>>(8)?;
                    let subject = row.get::<_, String>(5)?;
                    let preview = row.get::<_, String>(6)?;
                    Ok(NotificationSummary {
                        id: row.get(0)?,
                        message_id: row.get(1)?,
                        account_id: row.get(2)?,
                        account_email: row.get(3)?,
                        sender: row.get(4)?,
                        subject: if expired {
                            redact_expired(&subject, code.as_deref())
                        } else {
                            subject
                        },
                        preview: if expired {
                            redact_expired(&preview, code.as_deref())
                        } else {
                            preview
                        },
                        status: if kind == "access" {
                            if expired {
                                "expired"
                            } else {
                                "valid"
                            }
                        } else {
                            "security"
                        }
                        .to_string(),
                        kind,
                        code: if expired { None } else { code },
                        received_at: DateTime::parse_from_rfc3339(&received_at)
                            .map(|value| value.with_timezone(&Utc))
                            .unwrap_or(now),
                        expires_at,
                        reason: row.get(11)?,
                        is_seen: row.get::<_, Option<String>>(12)?.is_some(),
                    })
                },
            )?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(notifications)
    }

    pub fn dismiss_notification(
        &self,
        notification_id: &str,
        now: DateTime<Utc>,
    ) -> CommandResult<usize> {
        Ok(self.conn.execute(
            "UPDATE mail_notifications SET dismissed_at = ?1 WHERE message_id = ?2",
            params![now.to_rfc3339(), notification_id],
        )?)
    }

    pub fn restore_notification(
        &self,
        notification_id: &str,
        now: DateTime<Utc>,
    ) -> CommandResult<usize> {
        let changed = self.conn.execute(
            "UPDATE mail_notifications
             SET hidden_at = NULL, dismissed_at = ?1, restored = 1
             WHERE message_id = ?2",
            params![now.to_rfc3339(), notification_id],
        )?;
        restore_visible_notification_messages(&self.conn)?;
        refresh_folder_unread_counts(&self.conn)?;
        Ok(changed)
    }

    pub fn notification_list(
        &self,
        now: DateTime<Utc>,
        account_id: Option<&str>,
    ) -> CommandResult<NotificationList> {
        let items = self.list_notifications(now, account_id)?;
        let unseen_count = items.iter().filter(|item| !item.is_seen).count();
        let next_expiry_at = self
            .conn
            .query_row(
                "SELECT MIN(expires_at) FROM mail_notifications
                 WHERE kind = 'access'
                   AND restored = 0
                   AND hidden_at IS NULL
                   AND julianday(expires_at) > julianday(?1)",
                params![now.to_rfc3339()],
                |row| row.get::<_, Option<String>>(0),
            )?
            .and_then(|value| DateTime::parse_from_rfc3339(&value).ok())
            .map(|value| value.with_timezone(&Utc));
        Ok(NotificationList {
            items,
            unseen_count,
            next_expiry_at,
            enabled: self.notifications_enabled()?,
        })
    }

    pub fn mark_notifications_seen(
        &mut self,
        notification_ids: &[String],
        now: DateTime<Utc>,
    ) -> CommandResult<usize> {
        let tx = self.conn.transaction()?;
        let mut changed = 0;
        for notification_id in notification_ids {
            changed += tx.execute(
                "UPDATE mail_notifications
                 SET seen_at = ?1
                 WHERE message_id = ?2 AND seen_at IS NULL",
                params![now.to_rfc3339(), notification_id],
            )?;
        }
        tx.commit()?;
        Ok(changed)
    }

    pub fn notification_setting(&self) -> CommandResult<NotificationSetting> {
        Ok(NotificationSetting {
            enabled: self.notifications_enabled()?,
        })
    }

    pub fn set_notifications_enabled(&self, enabled: bool) -> CommandResult<NotificationSetting> {
        self.set_setting(
            "notifications_enabled",
            if enabled { "true" } else { "false" },
        )?;
        if !enabled {
            self.conn
                .execute("UPDATE mail_notifications SET hidden_at = NULL", [])?;
            restore_visible_notification_messages(&self.conn)?;
            refresh_folder_unread_counts(&self.conn)?;
        }
        Ok(NotificationSetting { enabled })
    }

    pub fn pending_actions(&self) -> CommandResult<Vec<(String, String, String)>> {
        let mut stmt = self.conn.prepare(
            "SELECT account_id, provider_message_id, action FROM pending_mail_actions ORDER BY created_at",
        )?;
        let actions = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(actions)
    }

    pub fn complete_pending_action(
        &self,
        account_id: &str,
        message_id: &str,
        action: &str,
    ) -> CommandResult<()> {
        self.conn.execute(
            "DELETE FROM pending_mail_actions WHERE account_id = ?1 AND provider_message_id = ?2 AND action = ?3",
            params![account_id, message_id, action],
        )?;
        Ok(())
    }

    #[cfg(test)]
    pub fn seed_deletion_example(&self) -> CommandResult<()> {
        self.conn.execute_batch(
            "INSERT INTO accounts VALUES ('account-1', 'icloud', 'reader@icloud.com', 'iCloud Mail', 'idle', NULL);
             INSERT INTO folders VALUES ('inbox', 'account-1', 'Inbox', 1);
             INSERT INTO folders VALUES ('trash', 'account-1', 'Deleted Messages', 0);
             INSERT INTO messages VALUES ('local-1', 'inbox', 'account-1', 'remote-1', 'Sender', '[]', 'Subject', 'Preview', '2026-01-01T00:00:00Z', 0, 1);
             INSERT INTO message_bodies VALUES ('local-1', '', '');",
        )?;
        Ok(())
    }

    pub fn delete_messages(&mut self, message_ids: &[String]) -> CommandResult<usize> {
        let tx = self.conn.transaction()?;
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

    #[cfg(test)]
    pub fn message_folder(&self, message_id: &str) -> CommandResult<Option<String>> {
        Ok(self
            .conn
            .query_row(
                "SELECT folder_id FROM messages WHERE id = ?1",
                params![message_id],
                |row| row.get(0),
            )
            .optional()?)
    }

    #[cfg(test)]
    pub fn pending_action(&self, provider_message_id: &str) -> CommandResult<Option<String>> {
        Ok(self
            .conn
            .query_row(
                "SELECT action FROM pending_mail_actions WHERE provider_message_id = ?1",
                params![provider_message_id],
                |row| row.get(0),
            )
            .optional()?)
    }
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

pub(crate) fn get_icloud_worker<T: DeserializeOwned>(
    path: &str,
    access_token: &str,
) -> CommandResult<T> {
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

pub(crate) fn flush_pending_mail_actions(app: &AppHandle) {
    let state = app.state::<AppState>();
    let Ok(_outbox) = state.outbox.try_lock() else {
        return;
    };
    let pending = {
        let mailbox = state.mailbox.lock().expect("database mutex poisoned");
        let Ok(actions) = mailbox.pending_actions() else {
            return;
        };
        actions
    };

    for (account_id, message_id, action) in pending {
        let result = icloud_access_token(&account_id)
            .and_then(|token| apply_icloud_message_action(&message_id, &action, &token));
        if result.is_ok() {
            let mailbox = state.mailbox.lock().expect("database mutex poisoned");
            let _ = mailbox.complete_pending_action(&account_id, &message_id, &action);
        }
    }
}

#[tauri::command]
pub async fn sync_account(
    state: tauri::State<'_, AppState>,
    account_id: String,
) -> CommandResult<MailAccount> {
    let provider = {
        let mailbox = state.mailbox.lock().expect("database mutex poisoned");
        mailbox.find_account(&account_id)?.provider
    };
    ensure_sync_supported(provider)?;
    let token = icloud_access_token(&account_id)?;
    let sync = tauri::async_runtime::spawn_blocking(move || {
        get_icloud_worker::<SyncedMailbox>("sync", &token)
    })
    .await
    .map_err(|error| AppError::Network(error.to_string()))??;
    let mut mailbox = state.mailbox.lock().expect("database mutex poisoned");
    mailbox.store_sync(&account_id, sync)?;
    mailbox.find_account(&account_id)
}

fn ensure_sync_supported(provider: Provider) -> CommandResult<()> {
    match provider {
        Provider::Icloud => Ok(()),
        other => Err(AppError::UnsupportedProvider(format!(
            "{} mail sync is not implemented",
            other.as_str()
        ))),
    }
}

#[tauri::command]
pub async fn sync_all(state: tauri::State<'_, AppState>) -> CommandResult<Vec<MailAccount>> {
    let accounts = {
        let mailbox = state.mailbox.lock().expect("database mutex poisoned");
        mailbox.list_accounts()?
    };
    let mut first_error = None;
    for account in accounts {
        if matches!(account.provider, Provider::Icloud) {
            let result = match icloud_access_token(&account.id) {
                Ok(token) => tauri::async_runtime::spawn_blocking(move || {
                    get_icloud_worker::<SyncedMailbox>("sync", &token)
                })
                .await
                .map_err(|error| AppError::Network(error.to_string()))
                .and_then(|result| result),
                Err(error) => Err(error),
            };
            match result {
                Ok(sync) => {
                    let mut mailbox = state.mailbox.lock().expect("database mutex poisoned");
                    mailbox.store_sync(&account.id, sync)?;
                }
                Err(error) if first_error.is_none() => first_error = Some(error),
                Err(_) => {}
            }
        }
    }
    if let Some(error) = first_error {
        return Err(error);
    }
    let mailbox = state.mailbox.lock().expect("database mutex poisoned");
    mailbox.list_accounts()
}

#[tauri::command]
pub fn list_accounts(state: tauri::State<AppState>) -> CommandResult<Vec<MailAccount>> {
    let mailbox = state.mailbox.lock().expect("database mutex poisoned");
    mailbox.list_accounts()
}

#[tauri::command]
pub fn list_folders(
    state: tauri::State<AppState>,
    account_id: String,
) -> CommandResult<Vec<Folder>> {
    let mailbox = state.mailbox.lock().expect("database mutex poisoned");
    mailbox.list_folders(&account_id)
}

#[tauri::command]
pub fn list_messages(
    state: tauri::State<AppState>,
    folder_id: String,
    query: String,
) -> CommandResult<Vec<MessageSummary>> {
    let mailbox = state.mailbox.lock().expect("database mutex poisoned");
    mailbox.list_messages(&folder_id, &query)
}

#[tauri::command]
pub fn mark_messages_read(
    state: tauri::State<AppState>,
    message_ids: Vec<String>,
) -> CommandResult<CountResult> {
    if message_ids.is_empty() {
        return Ok(CountResult { count: 0 });
    }

    let mut mailbox = state.mailbox.lock().expect("database mutex poisoned");
    let updated = mailbox.mark_messages_read(&message_ids, false)?;
    Ok(CountResult { count: updated })
}

#[tauri::command]
pub fn mark_messages_unread(
    state: tauri::State<AppState>,
    message_ids: Vec<String>,
) -> CommandResult<CountResult> {
    if message_ids.is_empty() {
        return Ok(CountResult { count: 0 });
    }

    let mut mailbox = state.mailbox.lock().expect("database mutex poisoned");
    let updated = mailbox.mark_messages_read(&message_ids, true)?;
    Ok(CountResult { count: updated })
}

#[tauri::command]
pub fn delete_messages(
    app: AppHandle,
    state: tauri::State<'_, AppState>,
    message_ids: Vec<String>,
) -> CommandResult<CountResult> {
    if message_ids.is_empty() {
        return Ok(CountResult { count: 0 });
    }

    let mut mailbox = state.mailbox.lock().expect("database mutex poisoned");
    let changed = mailbox.delete_messages(&message_ids)?;
    drop(mailbox);
    thread::spawn(move || flush_pending_mail_actions(&app));
    Ok(CountResult { count: changed })
}

#[tauri::command]
pub fn mark_messages_spam(
    state: tauri::State<AppState>,
    message_ids: Vec<String>,
) -> CommandResult<CountResult> {
    if message_ids.is_empty() {
        return Ok(CountResult { count: 0 });
    }

    let mut mailbox = state.mailbox.lock().expect("database mutex poisoned");
    let changed = mailbox.mark_messages_spam(&message_ids)?;
    Ok(CountResult { count: changed })
}

#[tauri::command]
pub async fn get_message(
    state: tauri::State<'_, AppState>,
    message_id: String,
) -> CommandResult<MessageDetail> {
    let detail = {
        let mailbox = state.mailbox.lock().expect("database mutex poisoned");
        mailbox.get_message(&message_id)?
    };
    if !detail.body_html.is_empty() || !detail.body_text.is_empty() {
        return Ok(detail);
    }

    let provider = {
        let mailbox = state.mailbox.lock().expect("database mutex poisoned");
        mailbox.find_account(&detail.summary.account_id)?.provider
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
        get_icloud_worker::<SyncedMessageBody>(&path, &token)
    })
    .await
    .map_err(|error| AppError::Network(error.to_string()))??;

    let mut mailbox = state.mailbox.lock().expect("database mutex poisoned");
    if detail.summary.folder_id == "root:notifications" {
        mailbox.cache_notification_body(&message_id, remote)
    } else {
        mailbox.cache_message_body(&message_id, remote)
    }
}

#[tauri::command]
pub fn set_theme(state: tauri::State<AppState>, mode: String) -> CommandResult<ThemeResult> {
    if !matches!(mode.as_str(), "system" | "light" | "dark") {
        return Err(AppError::InvalidInput(
            "theme must be system, light, or dark".to_string(),
        ));
    }

    let mailbox = state.mailbox.lock().expect("database mutex poisoned");
    mailbox.set_setting("theme", &mode)?;
    Ok(ThemeResult { mode })
}

fn refresh_folder_unread_counts(conn: &Connection) -> CommandResult<()> {
    conn.execute(
        "UPDATE folders
         SET unread_count = (
           SELECT COUNT(*)
           FROM messages
           WHERE messages.folder_id = folders.id
             AND messages.is_unread = 1
             AND (lower(folders.name) <> 'inbox' OR NOT EXISTS (
               SELECT 1 FROM mail_notifications n
               WHERE n.account_id = messages.account_id
                 AND n.provider_message_id = messages.provider_message_id
                 AND n.hidden_at IS NOT NULL
                 AND n.restored = 0
             ))
         )",
        [],
    )?;
    Ok(())
}

fn aggregate_folder_names(folder_id: &str) -> Option<&'static str> {
    match folder_id {
        "root:inbox" => Some("'inbox'"),
        "root:trash" => Some("'trash', 'deleted messages', '刪除的郵件'"),
        "root:spam" => Some("'spam', 'junk', '垃圾郵件'"),
        _ => None,
    }
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

fn restore_visible_notification_messages(conn: &Connection) -> CommandResult<()> {
    conn.execute(
        "INSERT OR IGNORE INTO messages
           (id, folder_id, account_id, provider_message_id, sender, recipients_json,
            subject, preview, received_at, has_attachments, is_unread)
         SELECT n.message_id,
                (SELECT f.id FROM folders f
                 WHERE f.account_id = n.account_id AND lower(f.name) = 'inbox' LIMIT 1),
                n.account_id, n.provider_message_id, n.sender, '[]', n.subject, n.preview,
                n.received_at, 0, n.is_unread
         FROM mail_notifications n
         WHERE n.hidden_at IS NULL
           AND (n.sender <> '' OR n.subject <> '' OR n.preview <> '')
           AND EXISTS (SELECT 1 FROM folders f
                       WHERE f.account_id = n.account_id AND lower(f.name) = 'inbox')",
        [],
    )?;
    conn.execute(
        "INSERT OR IGNORE INTO message_bodies (message_id, body_html, body_text)
         SELECT n.message_id, n.body_html, n.body_text
         FROM mail_notifications n
         WHERE EXISTS (SELECT 1 FROM messages m WHERE m.id = n.message_id)",
        [],
    )?;
    Ok(())
}

fn account_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<MailAccount> {
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
}

fn message_summary_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<MessageSummary> {
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

#[cfg(test)]
mod tests {
    use super::{
        ensure_sync_supported, Mailbox, SyncedFolder, SyncedMailbox, SyncedMessage,
        SyncedMessageBody,
    };
    use crate::models::{MailAccount, Provider};
    use chrono::{Duration, TimeZone, Utc};
    use rusqlite::Connection;

    #[test]
    fn rejects_provider_sync_without_an_implementation() {
        let error = ensure_sync_supported(Provider::Gmail).unwrap_err();
        assert_eq!(
            error.to_string(),
            "unsupported provider: gmail mail sync is not implemented"
        );
    }

    #[test]
    fn existing_mailbox_upgrades_idempotently_for_notifications() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE accounts (
               id TEXT PRIMARY KEY,
               provider TEXT NOT NULL,
               email TEXT NOT NULL,
               display_name TEXT NOT NULL,
               sync_status TEXT NOT NULL DEFAULT 'idle',
               last_synced_at TEXT
             );",
        )
        .unwrap();
        let mailbox = Mailbox { conn };

        mailbox.initialize_schema().unwrap();
        mailbox.initialize_schema().unwrap();

        let state = mailbox
            .notification_list(Utc.with_ymd_and_hms(2026, 8, 16, 0, 0, 0).unwrap(), None)
            .unwrap();
        assert!(state.enabled);
        assert!(state.items.is_empty());
    }

    #[test]
    fn queues_remote_delete_and_moves_local_message_first() {
        let mut mailbox = Mailbox::in_memory().unwrap();
        mailbox.seed_deletion_example().unwrap();

        assert_eq!(
            mailbox.delete_messages(&["local-1".to_string()]).unwrap(),
            1
        );
        assert_eq!(
            mailbox.message_folder("local-1").unwrap().as_deref(),
            Some("trash")
        );
        assert_eq!(
            mailbox.pending_action("remote-1").unwrap().as_deref(),
            Some("trash")
        );
    }

    #[test]
    fn access_mail_leaves_inbox_at_expiry_without_changing_provider_unread_state() {
        let mut mailbox = Mailbox::in_memory().unwrap();
        mailbox
            .upsert_account(&MailAccount {
                id: "account-1".to_string(),
                provider: Provider::Icloud,
                email: "reader@icloud.com".to_string(),
                display_name: "iCloud Mail".to_string(),
                sync_status: "idle".to_string(),
                last_synced_at: None,
            })
            .unwrap();
        mailbox
            .store_sync(
                "account-1",
                SyncedMailbox {
                    folders: vec![SyncedFolder {
                        id: "inbox".to_string(),
                        name: "Inbox".to_string(),
                        unread_count: 1,
                    }],
                    messages: vec![SyncedMessage {
                        id: "otp-1".to_string(),
                        folder_id: "inbox".to_string(),
                        sender: "Example <security@example.com>".to_string(),
                        recipients: vec!["reader@icloud.com".to_string()],
                        subject: "Your login code is 123456".to_string(),
                        preview: "The session lasts 24 hours. This code is invalid in 5 minutes."
                            .to_string(),
                        received_at: "2026-08-16T00:00:00Z".to_string(),
                        has_attachments: false,
                        is_unread: true,
                    }],
                },
            )
            .unwrap();
        mailbox
            .cache_message_body(
                "account-1:otp-1",
                SyncedMessageBody {
                    body_html: String::new(),
                    body_text: "The session lasts 24 hours. Your login code is 123456. This code is invalid in 5 minutes."
                        .to_string(),
                    attachments: vec![],
                },
            )
            .unwrap();

        let before_expiry = Utc.with_ymd_and_hms(2026, 8, 16, 0, 4, 59).unwrap();
        mailbox.process_notifications(before_expiry, false).unwrap();
        assert_eq!(mailbox.list_messages("root:inbox", "").unwrap().len(), 1);
        let notifications = mailbox.list_notifications(before_expiry, None).unwrap();
        assert_eq!(notifications.len(), 1);
        assert_eq!(notifications[0].code.as_deref(), Some("123456"));
        assert_eq!(notifications[0].status, "valid");
        assert_eq!(
            mailbox
                .notification_list(before_expiry, None)
                .unwrap()
                .next_expiry_at,
            Some(Utc.with_ymd_and_hms(2026, 8, 16, 0, 5, 0).unwrap())
        );
        assert_eq!(
            mailbox
                .notification_list(before_expiry, None)
                .unwrap()
                .unseen_count,
            1
        );
        mailbox
            .mark_notifications_seen(&["account-1:otp-1".to_string()], before_expiry)
            .unwrap();
        assert_eq!(
            mailbox
                .notification_list(before_expiry, None)
                .unwrap()
                .unseen_count,
            0
        );

        let at_expiry = Utc.with_ymd_and_hms(2026, 8, 16, 0, 5, 0).unwrap();
        mailbox.process_notifications(at_expiry, false).unwrap();
        assert!(mailbox.list_messages("root:inbox", "").unwrap().is_empty());
        let expired = mailbox.list_notifications(at_expiry, None).unwrap();
        assert_eq!(expired[0].code, None);
        assert_eq!(expired[0].status, "expired");
        assert!(!expired[0].subject.contains("123456"));
        assert!(
            mailbox
                .get_message("account-1:otp-1")
                .unwrap()
                .summary
                .is_unread
        );
        mailbox
            .conn
            .execute(
                "INSERT INTO folders (id, account_id, name, unread_count)
                 VALUES ('account-1:trash', 'account-1', 'Trash', 0)",
                [],
            )
            .unwrap();
        mailbox
            .conn
            .execute(
                "UPDATE messages SET folder_id = 'account-1:trash' WHERE id = 'account-1:otp-1'",
                [],
            )
            .unwrap();
        assert_eq!(mailbox.list_messages("root:trash", "").unwrap().len(), 1);
        mailbox
            .conn
            .execute(
                "UPDATE messages SET folder_id = 'account-1:inbox' WHERE id = 'account-1:otp-1'",
                [],
            )
            .unwrap();
        assert!(!mailbox.set_notifications_enabled(false).unwrap().enabled);
        assert_eq!(mailbox.list_messages("root:inbox", "").unwrap().len(), 1);
        assert!(mailbox.set_notifications_enabled(true).unwrap().enabled);
        let after_retention = at_expiry + Duration::hours(25);
        mailbox
            .process_notifications(after_retention, false)
            .unwrap();
        assert!(mailbox
            .list_notifications(after_retention, None)
            .unwrap()
            .is_empty());
        assert!(mailbox
            .get_message("account-1:otp-1")
            .unwrap()
            .body_text
            .contains("123456"));
        mailbox
            .store_sync(
                "account-1",
                SyncedMailbox {
                    folders: vec![SyncedFolder {
                        id: "inbox".to_string(),
                        name: "Inbox".to_string(),
                        unread_count: 1,
                    }],
                    messages: vec![SyncedMessage {
                        id: "otp-1".to_string(),
                        folder_id: "inbox".to_string(),
                        sender: "Example".to_string(),
                        recipients: vec![],
                        subject: "Your login code is 123456".to_string(),
                        preview: "This code is invalid in 5 minutes.".to_string(),
                        received_at: "2026-08-16T00:00:00Z".to_string(),
                        has_attachments: false,
                        is_unread: true,
                    }],
                },
            )
            .unwrap();
        mailbox
            .process_notifications(after_retention, false)
            .unwrap();
        assert!(mailbox.list_messages("root:inbox", "").unwrap().is_empty());
        mailbox
            .store_sync(
                "account-1",
                SyncedMailbox {
                    folders: vec![SyncedFolder {
                        id: "inbox".to_string(),
                        name: "Inbox".to_string(),
                        unread_count: 0,
                    }],
                    messages: vec![],
                },
            )
            .unwrap();
        assert!(!mailbox.set_notifications_enabled(false).unwrap().enabled);
        assert_eq!(mailbox.list_messages("root:inbox", "").unwrap().len(), 1);
    }

    #[test]
    fn login_notice_waits_for_an_eligible_three_hour_summary_pass() {
        let mut mailbox = Mailbox::in_memory().unwrap();
        mailbox
            .upsert_account(&MailAccount {
                id: "account-1".to_string(),
                provider: Provider::Icloud,
                email: "reader@icloud.com".to_string(),
                display_name: "iCloud Mail".to_string(),
                sync_status: "idle".to_string(),
                last_synced_at: None,
            })
            .unwrap();
        mailbox
            .store_sync(
                "account-1",
                SyncedMailbox {
                    folders: vec![SyncedFolder {
                        id: "inbox".to_string(),
                        name: "Inbox".to_string(),
                        unread_count: 1,
                    }],
                    messages: vec![SyncedMessage {
                        id: "login-1".to_string(),
                        folder_id: "inbox".to_string(),
                        sender: "Example <security@example.com>".to_string(),
                        recipients: vec!["reader@icloud.com".to_string()],
                        subject: "New login detected".to_string(),
                        preview: "A new device signed in to your account.".to_string(),
                        received_at: "2026-08-16T00:00:00Z".to_string(),
                        has_attachments: false,
                        is_unread: true,
                    }],
                },
            )
            .unwrap();

        let after_one_hour = Utc.with_ymd_and_hms(2026, 8, 16, 1, 0, 0).unwrap();
        mailbox
            .process_notifications(after_one_hour, false)
            .unwrap();
        assert_eq!(mailbox.list_messages("root:inbox", "").unwrap().len(), 1);
        assert!(mailbox
            .list_notifications(after_one_hour, None)
            .unwrap()
            .is_empty());

        mailbox.process_notifications(after_one_hour, true).unwrap();
        assert!(mailbox.list_messages("root:inbox", "").unwrap().is_empty());
        let notifications = mailbox.list_notifications(after_one_hour, None).unwrap();
        assert_eq!(notifications.len(), 1);
        assert_eq!(notifications[0].status, "security");
        let after_retention = after_one_hour + Duration::days(8);
        mailbox
            .process_notifications(after_retention, true)
            .unwrap();
        assert!(mailbox
            .list_notifications(after_retention, None)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn dismissed_access_mail_still_expires_and_restore_is_permanent() {
        let mut mailbox = Mailbox::in_memory().unwrap();
        mailbox
            .upsert_account(&MailAccount {
                id: "account-1".to_string(),
                provider: Provider::Icloud,
                email: "reader@icloud.com".to_string(),
                display_name: "iCloud Mail".to_string(),
                sync_status: "idle".to_string(),
                last_synced_at: None,
            })
            .unwrap();
        mailbox
            .store_sync(
                "account-1",
                SyncedMailbox {
                    folders: vec![SyncedFolder {
                        id: "inbox".to_string(),
                        name: "Inbox".to_string(),
                        unread_count: 1,
                    }],
                    messages: vec![SyncedMessage {
                        id: "otp-1".to_string(),
                        folder_id: "inbox".to_string(),
                        sender: "Example".to_string(),
                        recipients: vec!["reader@icloud.com".to_string()],
                        subject: "驗證碼 654321".to_string(),
                        preview: "此驗證碼將在 5 分鐘後失效。".to_string(),
                        received_at: "2026-08-16T00:00:00Z".to_string(),
                        has_attachments: false,
                        is_unread: true,
                    }],
                },
            )
            .unwrap();
        mailbox
            .cache_message_body(
                "account-1:otp-1",
                SyncedMessageBody {
                    body_html: String::new(),
                    body_text: "驗證碼 654321。此驗證碼將在 5 分鐘後失效。".to_string(),
                    attachments: vec![],
                },
            )
            .unwrap();

        let before_expiry = Utc.with_ymd_and_hms(2026, 8, 16, 0, 4, 0).unwrap();
        mailbox.process_notifications(before_expiry, false).unwrap();
        mailbox
            .dismiss_notification("account-1:otp-1", before_expiry)
            .unwrap();
        assert!(mailbox
            .list_notifications(before_expiry, None)
            .unwrap()
            .is_empty());

        let at_expiry = Utc.with_ymd_and_hms(2026, 8, 16, 0, 5, 0).unwrap();
        mailbox.process_notifications(at_expiry, false).unwrap();
        assert!(mailbox.list_messages("root:inbox", "").unwrap().is_empty());

        mailbox
            .restore_notification("account-1:otp-1", at_expiry)
            .unwrap();
        mailbox
            .process_notifications(at_expiry + Duration::hours(1), false)
            .unwrap();
        assert_eq!(mailbox.list_messages("root:inbox", "").unwrap().len(), 1);

        mailbox
            .store_sync(
                "account-1",
                SyncedMailbox {
                    folders: vec![SyncedFolder {
                        id: "inbox".to_string(),
                        name: "Inbox".to_string(),
                        unread_count: 0,
                    }],
                    messages: vec![],
                },
            )
            .unwrap();
        assert_eq!(mailbox.list_messages("root:inbox", "").unwrap().len(), 1);
        assert!(mailbox
            .get_message("account-1:otp-1")
            .unwrap()
            .body_text
            .contains("654321"));

        let after_retention = at_expiry + Duration::hours(25);
        mailbox
            .process_notifications(after_retention, false)
            .unwrap();
        let tombstone: (i64, String) = mailbox
            .conn
            .query_row(
                "SELECT restored, body_text FROM mail_notifications
                 WHERE account_id = 'account-1' AND provider_message_id = 'otp-1'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(
            tombstone,
            (1, "驗證碼 654321。此驗證碼將在 5 分鐘後失效。".to_string())
        );
        mailbox
            .store_sync(
                "account-1",
                SyncedMailbox {
                    folders: vec![SyncedFolder {
                        id: "inbox".to_string(),
                        name: "Inbox".to_string(),
                        unread_count: 1,
                    }],
                    messages: vec![SyncedMessage {
                        id: "otp-1".to_string(),
                        folder_id: "inbox".to_string(),
                        sender: "Example".to_string(),
                        recipients: vec![],
                        subject: "驗證碼 654321".to_string(),
                        preview: "此驗證碼將在 5 分鐘後失效。".to_string(),
                        received_at: "2026-08-16T00:00:00Z".to_string(),
                        has_attachments: false,
                        is_unread: true,
                    }],
                },
            )
            .unwrap();
        mailbox
            .process_notifications(after_retention, false)
            .unwrap();
        assert_eq!(mailbox.list_messages("root:inbox", "").unwrap().len(), 1);
    }
}
