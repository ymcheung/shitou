mod error;
mod mailbox;
mod models;
mod state;

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use error::{AppError, CommandResult};
use mailbox::{Mailbox, SyncedMailbox, SyncedMessageBody};
use models::{
    AuthSession, AuthStartResult, CountResult, Folder, MailAccount, MessageDetail, MessageSummary,
    Provider, Removed, ThemeResult,
};
use reqwest::blocking::Client;
use reqwest::StatusCode;
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

fn app_db_path(app: &AppHandle) -> CommandResult<PathBuf> {
    let mut dir = app
        .path()
        .app_data_dir()
        .map_err(|_| AppError::AppDataDirUnavailable)?;
    std::fs::create_dir_all(&dir).map_err(|_| AppError::AppDataDirUnavailable)?;
    dir.push("mailbox.sqlite3");
    Ok(dir)
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
    let mailbox = state.mailbox.lock().expect("database mutex poisoned");
    let value = mailbox.setting(AUTH_SESSION_SETTING_KEY)?;

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
    let mailbox = state.mailbox.lock().expect("database mutex poisoned");
    mailbox.set_setting(
        AUTH_SESSION_SETTING_KEY,
        &serde_json::to_string(&session).map_err(|error| AppError::Auth(error.to_string()))?,
    )?;
    Ok(session)
}

#[tauri::command]
fn auth_logout(state: tauri::State<AppState>) -> CommandResult<Removed> {
    let mailbox = state.mailbox.lock().expect("database mutex poisoned");
    mailbox.remove_setting(AUTH_SESSION_SETTING_KEY)?;
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
    use super::oauth_query_parameter;
    use crate::mailbox::Mailbox;

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
    let mailbox = state.mailbox.lock().expect("database mutex poisoned");
    if let Err(error) = mailbox.upsert_account(&account) {
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
    let mailbox = state.mailbox.lock().expect("database mutex poisoned");
    if let Err(error) = mailbox.upsert_account(&account) {
        let _ = keyring::Entry::new(SERVICE_NAME, &format!("icloud-access:{}", account.id))?
            .delete_credential();
        return Err(error.into());
    }
    Ok(account)
}

#[tauri::command]
fn account_remove(state: tauri::State<AppState>, account_id: String) -> CommandResult<Removed> {
    let mailbox = state.mailbox.lock().expect("database mutex poisoned");
    mailbox.remove_account(&account_id)?;
    drop(mailbox);

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

#[tauri::command]
async fn sync_account(
    state: tauri::State<'_, AppState>,
    account_id: String,
) -> CommandResult<MailAccount> {
    let provider = {
        let mailbox = state.mailbox.lock().expect("database mutex poisoned");
        mailbox.find_account(&account_id)?.provider
    };
    if matches!(provider, Provider::Icloud) {
        let token = icloud_access_token(&account_id)?;
        let sync = tauri::async_runtime::spawn_blocking(move || {
            get_icloud_worker::<SyncedMailbox>("sync", &token)
        })
        .await
        .map_err(|error| AppError::Network(error.to_string()))??;
        let mut mailbox = state.mailbox.lock().expect("database mutex poisoned");
        mailbox.store_sync(&account_id, sync)?;
        return mailbox.find_account(&account_id);
    }
    let mailbox = state.mailbox.lock().expect("database mutex poisoned");
    mailbox.mark_account_synced(&account_id)
}

#[tauri::command]
async fn sync_all(state: tauri::State<'_, AppState>) -> CommandResult<Vec<MailAccount>> {
    let accounts = {
        let mailbox = state.mailbox.lock().expect("database mutex poisoned");
        mailbox.list_accounts()?
    };
    for account in accounts {
        if matches!(account.provider, Provider::Icloud) {
            let token = icloud_access_token(&account.id)?;
            let sync = tauri::async_runtime::spawn_blocking(move || {
                get_icloud_worker::<SyncedMailbox>("sync", &token)
            })
            .await
            .map_err(|error| AppError::Network(error.to_string()))??;
            let mut mailbox = state.mailbox.lock().expect("database mutex poisoned");
            mailbox.store_sync(&account.id, sync)?;
        }
    }
    let mailbox = state.mailbox.lock().expect("database mutex poisoned");
    mailbox.list_accounts()
}

#[tauri::command]
fn list_accounts(state: tauri::State<AppState>) -> CommandResult<Vec<MailAccount>> {
    let mailbox = state.mailbox.lock().expect("database mutex poisoned");
    mailbox.list_accounts()
}

#[tauri::command]
fn list_folders(state: tauri::State<AppState>, account_id: String) -> CommandResult<Vec<Folder>> {
    let mailbox = state.mailbox.lock().expect("database mutex poisoned");
    mailbox.list_folders(&account_id)
}

#[tauri::command]
fn list_messages(
    state: tauri::State<AppState>,
    folder_id: String,
    query: String,
) -> CommandResult<Vec<MessageSummary>> {
    let mailbox = state.mailbox.lock().expect("database mutex poisoned");
    mailbox.list_messages(&folder_id, &query)
}

#[tauri::command]
fn mark_messages_read(
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
fn mark_messages_unread(
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
fn delete_messages(
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
fn mark_messages_spam(
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
async fn get_message(
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
    mailbox.cache_message_body(&message_id, remote)
}

#[tauri::command]
fn set_theme(state: tauri::State<AppState>, mode: String) -> CommandResult<ThemeResult> {
    if !matches!(mode.as_str(), "system" | "light" | "dark") {
        return Err(AppError::InvalidInput(
            "theme must be system, light, or dark".to_string(),
        ));
    }

    let mailbox = state.mailbox.lock().expect("database mutex poisoned");
    mailbox.set_setting("theme", &mode)?;
    Ok(ThemeResult { mode })
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
            let mailbox = Mailbox::open(db_path, mailbox_encryption_key()?)?;
            app.manage(AppState {
                mailbox: Mutex::new(mailbox),
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
