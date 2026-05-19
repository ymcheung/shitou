# Shitou Mail

Read-only macOS desktop mail app scaffold built with Tauri v2, SvelteKit, and Tailwind CSS.

## What Is Implemented

- Magic-link-only sign-in UI and Tauri command surface for `shitou://auth/callback`.
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

Neon Auth configuration should be completed in Neon so the only enabled application auth method is email magic link, with the desktop callback URL set to `shitou://auth/callback`.

## Real Neon Registration

To use Neon for real registration instead of the local demo stub:

1. Create or open the Neon project that owns the app database.
2. Enable Neon Auth for the project and disable every sign-in method except email magic link.
3. Add `shitou://auth/callback` as the allowed application callback URL.
4. Add the Neon Auth project URL and publishable/client key to the desktop runtime environment.
5. Replace the current Tauri `auth_start_magic_link` stub with a call to Neon Auth's magic-link endpoint.
6. Replace `auth_complete_callback` with the real callback exchange and persist the returned user id and email in the local session state.
7. Keep mailbox bodies and attachments in local encrypted storage only; Neon should store account identity/session metadata, not mail content.

## Verification

Current verified checks in this environment:

```bash
npm run build
npm audit --omit=dev --cache .npm-cache
```

Native Tauri/Rust compilation was not run here because `rustc` and `cargo` are not installed on this machine.
