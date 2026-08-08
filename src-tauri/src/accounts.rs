use std::io::{Read, Write};
use std::net::TcpListener;
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use reqwest::blocking::Client;
use serde::Deserialize;
use serde_json::json;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::error::{response_error_message, AppError, CommandResult};
use crate::models::{MailAccount, Provider, Removed};
use crate::state::AppState;

const SERVICE_NAME: &str = "com.shitou.mail";
const DEFAULT_ICLOUD_CONNECT_URL: &str =
    "https://shitou-icloud-connect.shitou-mail-cloud.workers.dev/icloud/connect";
const NYLAS_CALLBACK_URI: &str = "http://127.0.0.1:8392/callback";
const NYLAS_CALLBACK_ADDRESS: &str = "127.0.0.1:8392";
const NYLAS_CALLBACK_TIMEOUT: Duration = Duration::from_secs(300);

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
pub async fn account_connect_provider(
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
        sync_status: "unsupported".to_string(),
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
        return Err(error);
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

pub(crate) fn icloud_access_token(account_id: &str) -> CommandResult<String> {
    keyring::Entry::new(SERVICE_NAME, &format!("icloud-access:{account_id}"))?
        .get_password()
        .map_err(|error| match error {
            keyring::Error::NoEntry => AppError::Auth(
                "Reconnect this iCloud account once to enable mailbox sync".to_string(),
            ),
            other => other.into(),
        })
}

#[tauri::command]
pub async fn account_connect_icloud(
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
        return Err(error);
    }
    Ok(account)
}

#[tauri::command]
pub fn account_remove(state: tauri::State<AppState>, account_id: String) -> CommandResult<Removed> {
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

#[cfg(test)]
mod tests {
    use super::oauth_query_parameter;

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
}
