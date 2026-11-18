# Speech timestamp backend — 2026-11-18

## ASB01 plan, before source changes

Scope: transcription settings/model profile, outgoing audio multipart contract and returned real speech segments. The implementation owner changes only `src/transcription.rs`, `tests/viewer_backend.rs` and this history. Default new settings to `whisper-1`; retain explicitly saved models. The supported timing profile requests `verbose_json` plus `timestamp_granularities[]=segment`; all other models request `json` and remain untimed.

Required checks: exact multipart field names/values; real seconds preserved including overlap/out-of-order segments; missing/null/empty segments explicitly untimed; malformed types, nonfinite/negative/reversed/excessive timestamps, NUL text and more than 4096 segments rejected with the fixed public error. Maintain the existing 1 MiB response bound, audio-only 20 MiB upload, timeout, no retry/redirect, single slot and reset/shutdown contracts. Settings remain write-only/private and do not overwrite an existing explicit model during key-only changes.

Environment: repository Rust toolchain, temporary projects/configuration and bound loopback fake upstreams only. No real API/model/key, actual user configuration, microphone or persistent audio. Prior `viewer-backend-20261118.md` recorded a passing transport/secret/reset baseline; this work changes its prior text-only response contract. No long-duration/load run is needed for the bounded one-request format change.

ASB01 RED command: `cargo test --lib transcription::tests --test viewer_backend -- --nocapture`. Added assertions compile against serialized DTOs before implementation. Result: **3 passed, 5 expected failures** in the transport unit group (0.05 s): old `gpt-transcribe` default, absent `segments`/`timing`, and accepting malformed segment data. Cargo stopped after the failing library group, so the HTTP group requires a separate RED run. No product source has changed yet.

ASB02 RED command: `cargo test --test viewer_backend -- --nocapture`. Result: **5 passed, 2 expected failures** (0.45 s), both absent `timing_supported` assertions. Existing viewer, missing-key, secret hardening and reset/shutdown checks passed. Source implementation starts only after these RED results.

## ASB03 implementation and focused GREEN, planned

Added an exact known timing profile, bounded real segment DTO and explicit timing indicator. Multipart fields branch by selected model, and only the known Whisper profile parses segments. Malformed timing fails the whole response; other model profiles never claim timing.

ASB03 commands: `rustfmt --edition 2024 src/transcription.rs tests/viewer_backend.rs` succeeded; `cargo test --lib transcription::tests --test viewer_backend -- --nocapture` passed **8 transport tests** (0.05 s). The name filter also filtered out all seven HTTP tests; this is not HTTP verification. Add the accepted 4096-segment/safe-millisecond boundary fixture and run transport/HTTP separately.

ASB04 results:

- `cargo test --lib transcription::tests -- --nocapture`: **9 passed** (0.06 s), including exact format/granularity fields across four profiles, real segment preservation, untimed fallback, malformed timing rejection, count/time boundary acceptance and existing transport bounds/cancellation/no-retry behavior.
- `env -u OPENAI_API_KEY cargo test --test viewer_backend -- --nocapture`: **7 passed** (0.50 s), including explicit-model preservation through a key-only update and reopen, settings capability, private/write-only secrets, missing-key preflight and upload reset/shutdown/single-slot behavior. Environment key fallback was disabled for this run.
- `rustfmt --edition 2024 --check src/transcription.rs tests/viewer_backend.rs`: **exit 0**.
- `env -u OPENAI_API_KEY cargo clippy --lib --test viewer_backend -- -D warnings`: **exit 0**.
- Final owned-file `git diff --check`: **exit 0**. Source/test SHA-256 freeze identifiers: `8ef4c4998994ec324eccbd2575159f3af906a42a3b30c73fb155ef837593b7fa` and `54507b7d176dba94b85867a5793a4e8b22c7027d33f69432462ba3786f82c0d5` respectively.

Independent read-only critic review found no blocking issue in the model-profile branch, typed whole-response rejection, bounded time conversion, fallback, NUL handling and saved-model restart coverage. The only later change was the valid boundary acceptance fixture; no product source changed after the passing focused run.

## Frozen contract and remaining integration

Backend files are frozen for root integration. Response always contains `{text,model,segments,timing}`; segment times are seconds. Only exact `whisper-1` advertises `timing_supported:true` and emits parsed real segments. Empty/missing/null segments and every other model profile return `segments:[],timing:"none"`. A malformed present Whisper segment rejects the response with `transcription_response_invalid`; upstream content is never included in that error. Segment order/overlap and text are preserved for the frontend to combine with recorded events.

No actual user configuration or key was changed, audio persisted or real API/model/hardware invoked. Browser timeline/export integration belongs to the frontend owner and is not claimed by this focused backend verification. No dependency, version, install or commit changes were made by this owner.
