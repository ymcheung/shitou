use reqwest::blocking::Client;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::error::{response_error_message, AppError, CommandResult};
use crate::models::{AuthSession, AuthStartResult, Removed};
use crate::state::AppState;

const AUTH_SESSION_SETTING_KEY: &str = "auth_session";
const DEFAULT_NEON_AUTH_BASE_URL: &str =
    "https://ep-floral-heart-a7t3tanr.neonauth.ap-southeast-2.aws.neon.tech/neondb/auth";

fn neon_auth_url(path: &str) -> CommandResult<String> {
    let base_url = std::env::var("NEON_AUTH_BASE_URL")
        .unwrap_or_else(|_| DEFAULT_NEON_AUTH_BASE_URL.to_string());
    Ok(format!(
        "{}/{}",
        base_url.trim_end_matches('/'),
        path.trim_start_matches('/')
    ))
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

#[tauri::command]
pub fn auth_send_email_otp(email: String) -> CommandResult<AuthStartResult> {
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
pub fn auth_current_session(state: tauri::State<AppState>) -> CommandResult<Option<AuthSession>> {
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
pub fn auth_verify_email_otp(
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
pub fn auth_logout(state: tauri::State<AppState>) -> CommandResult<Removed> {
    let mailbox = state.mailbox.lock().expect("database mutex poisoned");
    mailbox.remove_setting(AUTH_SESSION_SETTING_KEY)?;
    Ok(Removed { removed: true })
}
