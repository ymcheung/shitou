# Shitou Mail

Read-only macOS desktop mail app scaffold built with Tauri v2, SvelteKit, and Tailwind CSS.

## What Is Implemented

- Email OTP-only sign-in UI and Tauri command surface for Neon Auth.
- Three-pane read-only mailbox UI with account list, folders, search, offline message reading, attachment metadata, and light/dark/system themes.
- Tauri commands for account connection, local sync, account removal, folder/message reads, and theme persistence.
- Gmail OAuth URL generation constrained to `https://www.googleapis.com/auth/gmail.readonly`.
- Outlook OAuth URL generation constrained to `openid email offline_access Mail.Read`.
- iCloud onboarding through IMAP credentials with app-specific password storage in macOS Keychain.
- Local SQLCipher mailbox schema for accounts, folders, messages, bodies, attachments, sync state, and settings, keyed from macOS Keychain.

## Explicitly Out Of Scope

- No calendar, reminders, contacts, tasks, notifications, compose, send, SMTP, archive, delete, move, label mutation, or read/write mail scopes.
- No mailbox bodies or attachment content are designed to be stored in Neon.

## Development

```bash
npm install --cache .npm-cache
npm run dev
```

Open `http://127.0.0.1:1420/` for browser preview.

For the native macOS shell, install Rust/Cargo and the Tauri prerequisites, then run:

```bash
npm run tauri dev
```

## Configuration

Set these environment variables before connecting real OAuth accounts:

```bash
GMAIL_OAUTH_CLIENT_ID=...
OUTLOOK_OAUTH_CLIENT_ID=...
```

Neon Auth configuration should be completed in Neon with Sign-up and Sign-in with Email enabled. Email OTP is invoked from the app through the Neon SDK rather than selected as a separate console-only sign-in method. Configure Neon Auth's custom SMTP provider with Resend for production email delivery:

```bash
SMTP_HOST=smtp.resend.com
SMTP_PORT=587
SMTP_USERNAME=resend
SMTP_PASSWORD=<RESEND_API_KEY>
SMTP_SENDER_EMAIL=auth@your-domain.example
SMTP_SENDER_NAME="Shitou Mail"
```

## Real Neon Registration

To use Neon for real registration instead of the local demo stub:

1. Create or open the Neon project that owns the app database.
2. Enable Neon Auth for the project and enable Sign-up and Sign-in with Email in Settings → Auth.
3. In Neon Settings → Auth, select Custom SMTP provider and enter the Resend SMTP credentials.
4. If you require email verification during sign-up, enable Verify at Sign-up and select Verification code.
5. Add `NEON_AUTH_BASE_URL` to the desktop runtime environment.
6. `auth_send_email_otp` calls `POST ${NEON_AUTH_BASE_URL}/email-otp/send-verification-otp` with `{ email, type: 'sign-in' }`.
7. `auth_verify_email_otp` calls `POST ${NEON_AUTH_BASE_URL}/sign-in/email-otp` with `{ email, otp }` and uses the returned user identity for the local session.
8. Keep mailbox bodies and attachments in local encrypted storage only; Neon should store account identity/session metadata, not mail content.

## Verification

Current verified checks in this environment:

```bash
npm run build
npm audit --omit=dev --cache .npm-cache
```

Native Tauri/Rust compilation was not run here because `rustc` and `cargo` are not installed on this machine.
