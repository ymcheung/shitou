# Shitou Mail

Read-only macOS desktop mail app scaffold built with Tauri v2, SvelteKit, and Tailwind CSS.

## What Is Implemented

- Email OTP-only sign-in UI and Tauri command surface for Neon Auth.
- Three-pane read-only mailbox UI with account list, folders, search, offline message reading, attachment metadata, and light/dark/system themes.
- Local in-app summaries for temporary iCloud access mail and login notices; no system notification permission is used.
- Tauri commands for account connection, local sync, account removal, folder/message reads, and theme persistence.
- Nylas desktop OAuth with PKCE for Gmail, Outlook, and iCloud account connections.
- Read-only Gmail (`gmail.readonly`) and Outlook (`Mail.Read`) authorization scopes.
- Nylas access and refresh tokens stored in macOS Keychain; no API key ships in the app.
- Local SQLCipher mailbox schema for accounts, folders, messages, bodies, attachments, sync state, and settings, keyed from macOS Keychain.

## Explicitly Out Of Scope

- No calendar, reminders, contacts, tasks, system notifications, compose, send, SMTP, archive, delete, move, label mutation, or read/write mail scopes.
- No mailbox bodies or attachment content are designed to be stored in Neon.

## Development

```bash
npm install --cache .npm-cache
npm run dev
```

Open `http://127.0.0.1:1420/` for browser preview.
Email OTP is intentionally unavailable in browser preview; use demo mode or
run the native shell for real authentication.

For the native macOS shell, install Rust/Cargo and the Tauri prerequisites, then run:

```bash
npm run tauri dev
```

## Configuration

In the Nylas Dashboard, register `http://127.0.0.1:8392/callback` as a
`desktop` callback URI for Google and Microsoft, and configure the Google,
Microsoft, and iCloud connectors. Then set the public Nylas application client
ID before running the desktop app:

```bash
NYLAS_CLIENT_ID=...
# Optional for EU applications:
NYLAS_API_URI=https://api.eu.nylas.com
```

iCloud users must first create an app-specific password in
[Apple Account settings](https://account.apple.com/account/manage). They enter
their iCloud Mail address and that password in Shitou Mail, which sends it once
through the Cloudflare Worker to Nylas and never stores it locally.

## Cloudflare Worker for iCloud

The Worker in `worker/` keeps `NYLAS_API_KEY` out of the desktop app and creates
read-only iCloud grants through Nylas Custom Authentication.

1. Create a free [Cloudflare account](https://dash.cloudflare.com/sign-up).
2. Create or copy an API key for the same Nylas application as the iCloud
   connector.
3. Authenticate Wrangler, save the API key as a Worker secret, and deploy:

```bash
pnpm exec wrangler login
pnpm run worker:test
pnpm exec wrangler secret put NYLAS_API_KEY --config worker/wrangler.jsonc
pnpm run worker:deploy
```

The deployed endpoint is
`https://shitou-icloud-connect.shitou-mail-cloud.workers.dev/icloud/connect`
and is included as the app default. Start Tauri with the Nylas client ID:

```bash
NYLAS_CLIENT_ID="..." npm run tauri dev
```

For local Worker development, copy `worker/.dev.vars.example` to
`worker/.dev.vars`, enter the Nylas API key, and run `pnpm run worker:dev`.
Because app sign-in is currently skipped, apply a Cloudflare rate-limit rule to
`/icloud/connect` before production use.

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
5. The app includes this project's public Neon Auth URL. Set
   `NEON_AUTH_BASE_URL` only when targeting a different Neon project.
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
