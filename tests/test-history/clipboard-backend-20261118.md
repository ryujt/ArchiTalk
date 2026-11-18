# Native clipboard fallback — backend (2026-11-18)

## Planned scope

- Goal: authenticated local `POST /api/v1/clipboard` copies UTF-8 text through macOS `pbcopy` when browser copying is unavailable, with no transcript or clipboard persistence.
- Boundary: new native writer, application lifecycle guard, existing HTTP Host/Origin/Bearer authorization. Browser behavior is verified by the root agent.
- Environment: isolated temporary projects; injected temporary native helper scripts. Tests never invoke the real system clipboard, OpenAI, or an existing user server.
- Risks: shell injection, stale workspace writes, unfinished children, oversized requests, misleading success, content in workspace history.
- Required checks: helper stdin preserves Unicode/newlines and shell-looking text; failure/timeout/cancellation; authorization and JSON/decoded byte bounds; reset/shutdown prevent or cancel work; no workspace state or receipt changes.
- Limitations: production adapter targets the supported macOS release platform. A real desktop clipboard write is intentionally excluded to preserve the user's clipboard.

## Runs

- `clipboard-backend-format-01`: initial `cargo fmt --all -- --check` found formatting differences in the new Rust code; applied the formatter to owned Rust files.
- `clipboard-backend-01`: `cargo test --lib clipboard::tests -- --nocapture` — 5 passed, 1 failed. The timeout test selected between helper readiness and the timeout future, so simultaneous readiness could report a completed timeout before reading its PID. The writer returned the expected fixed timeout error; this was a test-oracle race. The timeout case now checks the helper PID after the operation returns; cancellation still waits for a running PID before cancelling. Production and test timeouts are unchanged.
- `clipboard-backend-02`: `cargo test --lib clipboard::tests -- --nocapture` — 6 passed. The timeout and cancellation assertions confirm the direct helper PID has exited and been reaped; HTTP guards and no-history checks passed.
- `clipboard-backend-03`: `cargo test --lib clipboard::tests -- --nocapture` — 6 passed after explicit UTF-8 locale and status-code refinements. Local `man pbcopy` confirms its stdin encoding follows locale, so the adapter fixes `LC_ALL=en_US.UTF-8` without changing the parent process environment.
- `clipboard-backend-consumers-01`: `cargo test --test viewer_backend` — 7 passed. Existing viewer/transcription authorization, secret settings, file read boundaries, reset and shutdown consumers remain valid.
- `clipboard-backend-static-01`: `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, and `git diff --check` passed on the frozen backend source.

## Result and limits

- Endpoint success is emitted only after the injected native helper has accepted stdin and exited successfully. Unknown fields, malformed JSON, NULs, decoded text above 2 MiB and wire body above 3 MiB never start it.
- Tests observed helper termination and reaping for timeout, cancellation, workspace deletion and shutdown; no project history, receipts or transcript files were added by copying.
- The real `/usr/bin/pbcopy` adapter was inspected, not executed. Its desktop pasteboard integration still requires a user's normal macOS login session. Browser fallback, user messaging and direct retry are covered separately by the root agent.
