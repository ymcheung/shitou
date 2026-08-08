use std::path::PathBuf;

use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use serde::Deserialize;

use crate::error::{AppError, CommandResult};
use crate::models::{Attachment, Folder, MailAccount, MessageDetail, MessageSummary, Provider};

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

pub struct Mailbox {
    conn: Connection,
}

impl Mailbox {
    pub fn open(path: PathBuf, key: String) -> CommandResult<Self> {
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
            "#,
        )?;
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
            .query_map([], |row| account_from_row(row))?
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

    pub fn mark_account_synced(&self, account_id: &str) -> CommandResult<MailAccount> {
        self.conn.execute(
            "UPDATE accounts SET sync_status = 'idle', last_synced_at = ?1 WHERE id = ?2",
            params![Utc::now().to_rfc3339(), account_id],
        )?;
        self.find_account(account_id)
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
            let mut stmt = self.conn.prepare(&format!(
                "SELECT m.id, m.folder_id, m.account_id, m.provider_message_id, m.sender, m.recipients_json, m.subject, m.preview, m.received_at, m.has_attachments, m.is_unread
                 FROM messages m JOIN folders f ON f.id = m.folder_id
                 WHERE lower(f.name) IN ({folder_names}) AND (?1 = '%%' OR m.sender LIKE ?1 OR m.subject LIKE ?1 OR m.preview LIKE ?1)
                 ORDER BY m.received_at DESC",
            ))?;
            let rows = stmt.query_map(params![pattern], message_summary_from_row)?;
            rows.collect::<Result<Vec<_>, _>>()?
        } else {
            let mut stmt = self.conn.prepare(
                "SELECT id, folder_id, account_id, provider_message_id, sender, recipients_json, subject, preview, received_at, has_attachments, is_unread
                 FROM messages WHERE folder_id = ?1 AND (?2 = '%%' OR sender LIKE ?2 OR subject LIKE ?2 OR preview LIKE ?2)
                 ORDER BY received_at DESC",
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
        let summary = self.conn.query_row(
            "SELECT id, folder_id, account_id, provider_message_id, sender, recipients_json, subject, preview, received_at, has_attachments, is_unread FROM messages WHERE id = ?1",
            params![message_id],
            message_summary_from_row,
        )?;
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
