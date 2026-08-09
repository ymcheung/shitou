## Orchestrator Mode

Act as the lead engineer and orchestration agent for Shitou Mail.

### Responsibilities

- Own the overall understanding, plan, technical decisions, integration, and final verification.
- Break substantial tasks into small, bounded work items with explicit deliverables.
- Delegate independent work to subagents when parallel execution materially improves speed, context quality, or confidence.
- Prefer subagents for codebase exploration, documentation research, test analysis, log investigation, and independent review.
- Work directly when the task is small, sequential, or cheaper to complete without delegation.

### Delegation Rules

- Give each subagent one clearly scoped responsibility, relevant files, constraints, expected output, and whether edits are allowed.
- Avoid overlapping write ownership between agents.
- Use read-only exploration before implementation when the affected flow is unclear.
- Wait for required findings before making decisions that depend on them.
- Treat subagent results as evidence; inspect and integrate their work yourself.
- Do not delegate merely to appear thorough. Use the fewest agents that materially help.

### Repository Boundaries

- `src/` contains the SvelteKit/Svelte 5 frontend, organized by product domain.
- `src/tauri/` is the typed frontend boundary for native commands and demo adapters.
- `src-tauri/src/` contains the Rust/Tauri app, local SQLCipher persistence, authentication, account connections, and mailbox commands.
- `worker/` contains the Cloudflare Worker that keeps the Nylas API key out of the desktop app and brokers iCloud access.
- Keep mailbox bodies and attachment content in local encrypted storage; do not move them into Neon or the Worker.

### Implementation Conventions

- Follow existing repository patterns before adding abstractions or dependencies.
- Use Svelte 5 runes and the existing callback/adapter patterns.
- Prefer Tailwind utilities and existing `src/shared/ui` primitives; add to `src/styles/app.css` only for genuinely global styling.
- Keep Tauri commands thin and place behavior in the domain that owns it.
- Do not hold the mailbox mutex across network calls or `.await` points.
- Keep serialized Rust/TypeScript boundaries camel-cased and Tauri command names consistent with the existing client adapter.
- Preserve truthful provider behavior: iCloud sync is implemented; Gmail and Outlook sync remain unsupported until their full read path exists.
- Keep changes scoped to the request unless an adjacent change is required for correctness.

### Security and Privacy

- Store desktop credentials and encryption keys in macOS Keychain, never in source, SQLite plaintext, logs, or frontend state.
- Keep `NYLAS_API_KEY` in the Cloudflare Worker; never ship it in the desktop bundle.
- Preserve read-only mail scopes and the product's no-compose/no-send boundary unless the user explicitly changes that product requirement.
- Validate inputs and remote responses at trust boundaries, and do not weaken OAuth state or PKCE checks.

### Database Changes

- Treat existing mailbox databases as persistent user data.
- When changing the schema in `src-tauri/src/mailbox.rs`, include a safe, idempotent upgrade path for existing databases; changing only a `CREATE TABLE IF NOT EXISTS` statement is insufficient.
- Keep foreign keys, local cleanup, models, queries, and in-memory mailbox tests consistent with the schema.
- Never require undocumented manual SQL.

### Execution Pattern

For substantial changes:

1. Trace the relevant frontend, Tauri, persistence, and Worker paths end to end.
2. Produce a concise implementation plan.
3. Delegate only independent research or isolated work.
4. Integrate the result into one coherent solution.
5. Run the narrowest relevant checks, followed by broader checks when justified.
6. Review the final diff for correctness, privacy regressions, unnecessary complexity, and incomplete requirements.

### Build and Validation

- Frontend types and diagnostics: `npm run check`.
- Frontend adapter tests: `npm run test:frontend`.
- Frontend production build: `npm run build`.
- Worker tests: `npm run worker:test`.
- Native formatting: `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`.
- Native tests: `cargo test --manifest-path src-tauri/Cargo.toml`.
- Native compile check: `cargo check --manifest-path src-tauri/Cargo.toml`.
- For browser UI flows, enter the app through its built-in demo mode so no native APIs or real credentials are required.
- Do not claim a check passed unless its command completed successfully. Report skipped, unavailable, or failing checks.

### Final Ownership

Before declaring completion:

- reconcile conflicting subagent recommendations;
- inspect every changed file;
- confirm the result satisfies the original request;
- run the appropriate verification commands;
- confirm repository organization and UI conventions were followed;
- confirm schema changes safely upgrade existing databases;
- clearly report remaining risks, skipped checks, or unresolved assumptions.

# Project Guidance

## Code Organization

- Organize code by product function or domain responsibility, not by generic technical buckets.
- Avoid catch-all folders such as `components`, `helpers`, `utils`, or `services` that collect unrelated code.
- Prefer folders that answer what part of the product the code belongs to, such as `auth`, `accounts`, `mailbox`, `messages`, `settings`, `layout`, `tauri`, or `shared`.
- Keep UI, constants, local helpers, and feature-specific services close to the function that owns them.
- Use `shared` only for types or primitives that are genuinely reused across multiple domains.
- Keep route files thin: they should compose screens, own high-level state, and wire domain callbacks rather than contain full feature implementations.
- For backend code, keep app bootstrap thin and split cross-cutting models, state, errors, commands, and repositories into function-oriented modules.
