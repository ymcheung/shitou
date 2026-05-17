use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::error::{AppError, CommandResult};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthStartResult {
    pub sent: bool,
    pub email: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthSession {
    pub email: String,
    pub user_id: String,
    pub authenticated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Provider {
    Gmail,
    Outlook,
    Icloud,
}

impl Provider {
    pub fn from_input(value: &str) -> CommandResult<Self> {
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
pub struct MailAccount {
    pub id: String,
    pub provider: Provider,
    pub email: String,
    pub display_name: String,
    pub sync_status: String,
    pub last_synced_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Folder {
    pub id: String,
    pub account_id: String,
    pub name: String,
    pub unread_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Attachment {
    pub id: String,
    pub file_name: String,
    pub mime_type: String,
    pub byte_size: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageSummary {
    pub id: String,
    pub folder_id: String,
    pub account_id: String,
    pub provider_message_id: String,
    pub sender: String,
    pub recipients: Vec<String>,
    pub subject: String,
    pub preview: String,
    pub received_at: DateTime<Utc>,
    pub has_attachments: bool,
    pub is_unread: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageDetail {
    #[serde(flatten)]
    pub summary: MessageSummary,
    pub body_html: String,
    pub body_text: String,
    pub attachments: Vec<Attachment>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderAuthStart {
    pub provider: Provider,
    pub auth_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Removed {
    pub removed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThemeResult {
    pub mode: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CountResult {
    pub count: usize,
}
