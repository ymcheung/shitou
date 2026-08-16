use chrono::{DateTime, Duration, Utc};

use crate::accounts::icloud_access_token;
use crate::error::CommandResult;
use crate::mailbox::{get_icloud_worker, SyncedMessageBody};
use crate::models::{CountResult, NotificationList, NotificationSetting};
use crate::state::AppState;

pub(crate) struct NotificationClassification {
    pub kind: &'static str,
    pub code: Option<String>,
    pub expires_at: Option<DateTime<Utc>>,
    pub reason: String,
}

pub(crate) fn classify(
    content: &str,
    received_at: DateTime<Utc>,
) -> Option<NotificationClassification> {
    let normalized = content.to_lowercase();
    const EXCLUDED: &[&str] = &[
        "password changed",
        "your password was changed",
        "recovery email",
        "billing",
        "payment",
        "密碼已變更",
        "變更密碼",
        "復原",
        "恢復",
        "帳單",
        "付款",
    ];
    if EXCLUDED.iter().any(|marker| normalized.contains(marker)) {
        return None;
    }

    const ACCESS: &[&str] = &[
        "one-time code",
        "one time code",
        "otp",
        "login code",
        "sign-in code",
        "signin code",
        "security code",
        "verification code",
        "verification link",
        "magic link",
        "驗證碼",
        "一次性密碼",
        "登入碼",
        "登錄碼",
        "驗證連結",
    ];
    if ACCESS.iter().any(|marker| normalized.contains(marker)) {
        let seconds = duration_seconds(&normalized).unwrap_or(15 * 60);
        return Some(NotificationClassification {
            kind: "access",
            code: access_code(content),
            expires_at: Some(received_at + Duration::seconds(seconds)),
            reason: format!(
                "One-time access · expires after {}",
                format_duration(seconds)
            ),
        });
    }

    const SECURITY: &[&str] = &[
        "new login",
        "new sign-in",
        "new sign in",
        "login attempt",
        "sign-in attempt",
        "signed in",
        "successful login",
        "login successful",
        "new device",
        "新的登入",
        "新登入",
        "登入嘗試",
        "登入成功",
        "新裝置",
        "新設備",
    ];
    SECURITY
        .iter()
        .any(|marker| normalized.contains(marker))
        .then_some(NotificationClassification {
            kind: "security",
            code: None,
            expires_at: None,
            reason: "Login notice · summarized after 1 hour".to_string(),
        })
}

pub(crate) fn looks_relevant(content: &str) -> bool {
    let normalized = content.to_lowercase();
    [
        "code",
        "otp",
        "verify",
        "verification",
        "magic link",
        "login",
        "sign-in",
        "sign in",
        "new device",
        "驗證",
        "登入",
        "登錄",
        "一次性",
        "新裝置",
        "新設備",
    ]
    .iter()
    .any(|marker| normalized.contains(marker))
}

pub(crate) fn duration_seconds(content: &str) -> Option<i64> {
    let normalized = content
        .to_lowercase()
        .replace("分鐘", " minutes ")
        .replace("小時", " hours ");
    let mut durations = Vec::new();
    let bytes = normalized.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if !bytes[index].is_ascii_digit() {
            index += 1;
            continue;
        }
        let start = index;
        while index < bytes.len() && bytes[index].is_ascii_digit() {
            index += 1;
        }
        let Ok(value) = normalized[start..index].parse::<i64>() else {
            continue;
        };
        let unit = normalized[index..].trim_start_matches(|character: char| {
            character.is_whitespace() || matches!(character, '-' | ':' | '：')
        });
        let seconds = if unit.starts_with("minute") || unit.starts_with("min") {
            Some(value * 60)
        } else if unit.starts_with("hour") || unit.starts_with("hr") {
            Some(value * 60 * 60)
        } else {
            None
        };
        if let Some(seconds) = seconds {
            durations.push((start, seconds));
        }
    }
    if durations.is_empty() {
        return None;
    }

    let markers = [
        "invalid",
        "expire",
        "valid for",
        "code",
        "otp",
        "link",
        "驗證碼",
        "連結",
        "失效",
        "過期",
        "到期",
        "有效",
    ]
    .iter()
    .flat_map(|marker| {
        normalized
            .match_indices(marker)
            .map(|(position, _)| position)
    })
    .collect::<Vec<_>>();
    if markers.is_empty() {
        return durations.into_iter().map(|(_, seconds)| seconds).min();
    }
    durations
        .into_iter()
        .min_by_key(|(position, seconds)| {
            (
                markers
                    .iter()
                    .map(|marker| position.abs_diff(*marker))
                    .min()
                    .unwrap_or(usize::MAX),
                *seconds,
            )
        })
        .map(|(_, seconds)| seconds)
}

fn access_code(content: &str) -> Option<String> {
    const MARKERS: &[&str] = &[
        "code",
        "otp",
        "password",
        "驗證碼",
        "一次性密碼",
        "登入碼",
        "登錄碼",
    ];
    let tokens = content
        .split(|character: char| !(character.is_ascii_alphanumeric() || character == '-'))
        .filter(|word| {
            let compact = word.replace('-', "");
            (4..=12).contains(&compact.len())
                && compact
                    .chars()
                    .all(|character| character.is_ascii_alphanumeric())
                && compact.chars().any(|character| character.is_ascii_digit())
        })
        .collect::<Vec<_>>();
    let normalized = content.to_lowercase();
    MARKERS
        .iter()
        .filter_map(|marker| normalized.find(marker))
        .flat_map(|marker| {
            tokens.iter().filter_map(move |token| {
                content
                    .find(token)
                    .map(|position| (position.abs_diff(marker), *token))
            })
        })
        .min_by_key(|(distance, _)| *distance)
        .map(|(_, token)| token.to_string())
        .or_else(|| tokens.first().map(|token| (*token).to_string()))
}

fn format_duration(seconds: i64) -> String {
    if seconds % 3600 == 0 {
        let hours = seconds / 3600;
        format!("{hours} hour{}", if hours == 1 { "" } else { "s" })
    } else {
        let minutes = seconds / 60;
        format!("{minutes} minute{}", if minutes == 1 { "" } else { "s" })
    }
}

pub(crate) fn redact_expired(value: &str, code: Option<&str>) -> String {
    let redacted = code
        .filter(|code| !code.is_empty())
        .map_or_else(|| value.to_string(), |code| value.replace(code, "••••••"));
    redacted
        .split_whitespace()
        .map(|word| {
            if word.starts_with("http://") || word.starts_with("https://") {
                "[expired link]"
            } else {
                word
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[tauri::command]
pub async fn process_notifications(
    state: tauri::State<'_, AppState>,
    summarize_security: bool,
) -> CommandResult<NotificationList> {
    refresh(&state, summarize_security).await
}

async fn refresh(
    state: &tauri::State<'_, AppState>,
    summarize_security: bool,
) -> CommandResult<NotificationList> {
    let now = Utc::now();
    let candidates = {
        let mut mailbox = state.mailbox.lock().expect("database mutex poisoned");
        mailbox.discover_notification_bodies(now, summarize_security)?
    };

    for candidate in candidates {
        let Ok(token) = icloud_access_token(&candidate.account_id) else {
            continue;
        };
        let path = format!(
            "messages/{}",
            urlencoding::encode(&candidate.provider_message_id)
        );
        let Ok(remote) = tauri::async_runtime::spawn_blocking(move || {
            get_icloud_worker::<SyncedMessageBody>(&path, &token)
        })
        .await
        else {
            continue;
        };
        let Ok(remote) = remote else { continue };
        let mut mailbox = state.mailbox.lock().expect("database mutex poisoned");
        let _ = mailbox.cache_message_body(&candidate.message_id, remote);
    }

    let mut mailbox = state.mailbox.lock().expect("database mutex poisoned");
    mailbox.process_notifications(now, summarize_security)?;
    mailbox.notification_list(now, None)
}

#[tauri::command]
pub fn list_notifications(
    state: tauri::State<AppState>,
    account_id: Option<String>,
) -> CommandResult<NotificationList> {
    let mailbox = state.mailbox.lock().expect("database mutex poisoned");
    mailbox.notification_list(Utc::now(), account_id.as_deref())
}

#[tauri::command]
pub fn mark_notifications_seen(
    state: tauri::State<AppState>,
    notification_ids: Vec<String>,
) -> CommandResult<CountResult> {
    let mut mailbox = state.mailbox.lock().expect("database mutex poisoned");
    let count = mailbox.mark_notifications_seen(&notification_ids, Utc::now())?;
    Ok(CountResult { count })
}

#[tauri::command]
pub fn dismiss_notification(
    state: tauri::State<AppState>,
    notification_id: String,
) -> CommandResult<CountResult> {
    let mailbox = state.mailbox.lock().expect("database mutex poisoned");
    let count = mailbox.dismiss_notification(&notification_id, Utc::now())?;
    Ok(CountResult { count })
}

#[tauri::command]
pub fn restore_notification(
    state: tauri::State<AppState>,
    notification_id: String,
) -> CommandResult<CountResult> {
    let mailbox = state.mailbox.lock().expect("database mutex poisoned");
    let count = mailbox.restore_notification(&notification_id, Utc::now())?;
    Ok(CountResult { count })
}

#[tauri::command]
pub fn get_notifications_setting(
    state: tauri::State<AppState>,
) -> CommandResult<NotificationSetting> {
    let mailbox = state.mailbox.lock().expect("database mutex poisoned");
    mailbox.notification_setting()
}

#[tauri::command]
pub fn set_notifications_enabled(
    state: tauri::State<AppState>,
    enabled: bool,
) -> CommandResult<NotificationSetting> {
    let mailbox = state.mailbox.lock().expect("database mutex poisoned");
    mailbox.set_notifications_enabled(enabled)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn classifies_supported_policy_edges() {
        let received = Utc.with_ymd_and_hms(2026, 8, 16, 0, 0, 0).unwrap();
        let access = classify(
            "Account 998877. Verification Code ABCD-1234 expires in 2 hours",
            received,
        )
        .unwrap();
        assert_eq!(access.code.as_deref(), Some("ABCD-1234"));
        assert_eq!(access.expires_at, Some(received + Duration::hours(2)));
        assert_eq!(
            redact_expired("Code ABCD-1234", access.code.as_deref()),
            "Code ••••••"
        );
        assert_eq!(
            classify("login code 123456", received).unwrap().expires_at,
            Some(received + Duration::minutes(15))
        );
        assert_eq!(
            classify(
                "new login detected; if this wasn't you, change your password",
                received
            )
            .unwrap()
            .kind,
            "security"
        );
        assert!(classify("your password was changed", received).is_none());
        assert_eq!(
            classify("Your Successful Login was recorded", received)
                .unwrap()
                .kind,
            "security"
        );
    }
}
