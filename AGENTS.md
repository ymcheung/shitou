# Project Guidance

## Code Organization

- Organize code by product function or domain responsibility, not by generic technical buckets.
- Avoid catch-all folders such as `components`, `helpers`, `utils`, or `services` that collect unrelated code.
- Prefer folders that answer what part of the product the code belongs to, such as `auth`, `accounts`, `mailbox`, `messages`, `settings`, `layout`, `tauri`, or `shared`.
- Keep UI, constants, local helpers, and feature-specific services close to the function that owns them.
- Use `shared` only for types or primitives that are genuinely reused across multiple domains.
- Keep route files thin: they should compose screens, own high-level state, and wire domain callbacks rather than contain full feature implementations.
- For backend code, keep app bootstrap thin and split cross-cutting models, state, errors, commands, and repositories into function-oriented modules.
