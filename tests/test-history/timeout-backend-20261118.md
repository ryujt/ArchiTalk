# AI wall-time backend contract — 2026-11-18

## Plan before implementation

The confirmed request changes the single per-request default from 300 to 1800
seconds and the hard maximum from 600 to 3600 seconds. Configuration/UI retain
their ten-second minimum; the adapter retains its one-second minimum for bounded
process tests. No role-specific timeout fields are added. Explicit existing TOML
values such as 300 remain authoritative, and a RunSpec retains the setting it
captured before any later settings change.

Owned scope is `src/config.rs`, timeout-related `src/ai.rs` and
`tests/ai_adapter.rs` hunks, `tests/timeout_contract.rs`, and this record. First run
counterexamples against unchanged production code, then share constants between
configuration and adapter validation, and report the applied wall-time seconds
with a fixed diagnostic template. Empty stderr must not add meaningless
`reason=none` / `component=unknown` text; actual bounded diagnostic categories and
the termination-unknown safety guard remain intact.

The new contract tests cover absent/omitted defaults, both validation intervals,
explicit TOML byte preservation, every role's immutable RunSpec snapshot, and
pre-cancelled adapter requests that validate without spawning a CLI or touching a
project. The existing one-second SIGTERM-resistant fake CLI test will emit a
partial assistant response before blocking, proving timeout still rejects partial
success, kills the process group, and reports the actual applied seconds. Existing
cancellation, dropped-future cleanup, and terminal-result checks remain required.

All fixtures are synthetic and use owned temporary directories. No real model,
user DB, live server, shared settings file, installation, or commit is touched.

## Red baseline

- `cargo test --test timeout_contract -- --nocapture`: exit **101**, **2 passed /
  4 failed**. Explicit TOML 300 byte preservation and every role's captured
  RunSpec passed. The default test observed 300 rather than 1800; configuration
  and adapter rejected 1800, and adapter's range diagnostic still said 600.
- `cargo test --test ai_adapter
  timeout_kills_term_resistant_cli_and_does_not_retry -- --nocapture`: exit
  **101**, **0 passed / 1 failed**. The SIGTERM-resistant child emitted a partial
  assistant frame, timed out, and was confirmed dead; the failure was the message
  `AI CLI reached its wall-time limit. [stderr: reason=none, component=unknown]`
  instead of a precise one-second limit with no empty stderr suffix.

Both commands ran before any production source changes.

## Implementation and focused green result

`src/config.rs` now defines the shared constants
`AI_DEFAULT_TIMEOUT_SECONDS = 1800`, `AI_MIN_TIMEOUT_SECONDS = 10`,
`AI_ADAPTER_MIN_TIMEOUT_SECONDS = 1`, and `AI_MAX_TIMEOUT_SECONDS = 3600`.
Defaults and both validators use those constants. The deserialization and load
paths are unchanged, so an explicit 300 is neither converted nor rewritten.

The existing process deadline reports `AI CLI reached its {seconds}-second
wall-time limit.` using the validated RunSpec's applied integer. Failure errors
append the existing bounded, redacted stderr categories only when stderr is
nonempty. The error includes no prompt, response, path, or authentication text.
The child spawn count, no-retry flow, cancellation, process-group TERM/KILL,
three-second cleanup grace, terminal-result requirement, and
`termination_unknown` guard are unchanged.

- `cargo test --test timeout_contract --test ai_adapter -- --nocapture`: exit
  **0**, **20 passed / 0 failed / 1 ignored**. Six timeout contract tests passed;
  fourteen adapter tests passed, including cancellation of the CLI and its
  process group, dropped-future cleanup, bounded I/O failures, missing terminal
  success rejection, diagnostic redaction, and the one-second SIGTERM-resistant
  partial-output timeout. The authenticated real-CLI/model test remained ignored.
- `rustfmt --edition 2024 src/config.rs src/ai.rs tests/timeout_contract.rs
  tests/ai_adapter.rs`: exit **0**.
- `cargo clippy --lib --test timeout_contract --test ai_adapter -- -D warnings`:
  exit **0**.

The 1800/3600-second cases verify finite bounds and cancellation-before-spawn;
they do not wait for those long durations or invoke a model. Root separately owns
App/HTTP/UI compatibility and final integrated checks. No user DB, live server,
shared settings, installation, or real model call was changed by this task.

Independent read-only review found no blocker. It confirmed shared range values,
validation before pre-cancellation, captured timeout seconds, preservation of
actual stderr categories, and unchanged select/termination/drain/unknown-lock
behavior. It did not run additional tests or edit code.
