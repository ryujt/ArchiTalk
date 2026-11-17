# ArchiTalk implementation verification

Baseline: `1052929`, clean `main` before implementation. Owner: integration agent.
Previous records: `20261117-tobe-validation.md` and `ui-20261117-static.md` establish design-only checks; no prior product tests exist. Their pending CLI/process/filesystem/browser checks are included in this implementation's risk matrix.

## Scope

- Required: Rust build, authenticated loopback HTTP, persistent sessions/revisions/idempotency, interview/draft/review/export with real adapters, settings/model/effort, safe immutable files, cancellation/restart behavior, UI protocol checks.
- Evidence: unit/contract tests plus actual local SQLite/filesystem/process and installed Claude/Codex smoke executions. Fixtures prove application rules, not remote model compatibility.
- Agent ownership: root domain/controller/http/runtime; AI agent subprocess adapters; storage agent DB/files; UI agent browser assets. No explicit model override; inherited models are used.
- Every subsequent execution is recorded below before running, then finalized with results. AI/storage/UI module runs have separate owner records linked by the index.

## I01 — dependency/build setup

- Status: complete. Exact fetch start was not measured; the previously written 20:57 task estimate is withdrawn. Setup ran after the observed 20:53:41 KST baseline.
- Command: `cargo fetch`, followed by `cargo check` once implementation modules exist.
- Expected: dependencies resolve; no external AI call for this step. Build failure is kept and followed by corrective runs.
- Result: `cargo fetch` exit 0, 236 locked dependencies. First `cargo check` started **2026-11-17 21:01:39 KST**, exit 101, four errors: root treated file gateway's `Result<ExportResult>` as an unwrapped result. Corrected the consumer with a failure-preserving conversion; second `cargo check` exit 0 (1.01 seconds). This is compile evidence only.

## I02 — controller/domain/HTTP workflow tests

- Status: prepared before execution. Exact command timestamps/results will be appended.
- Previous: I01's type-contract error fixed; module agents' real SQLite/filesystem/CLI spikes are recorded separately. Tests will call the real controller/router/store and subprocess adapter with an explicit fixture executable; it does not prove model quality or real remote output.
- Required assertions: token/Host/Origin, persistent state, one active run, idempotency before revision, settings trust/next-run snapshot, truthful unknowns, draft/critic/export, immutable saved copies, interrupted-process recovery gate, unsafe file/context rejection. A separate explicitly requested local CLI workflow will cover the real model boundary.
- First run: **2026-11-17 21:04:20 KST**, `cargo test --test workflow -- --nocapture`, exit 0, 3/3 passed. Actual router/controller/SQLite/filesystem and fixture CLI process were used. Subsequent domain/export/diagnostic hardening requires a new final run.

## I03 — real installed CLI, real TCP product workflow

- Status: prepared before execution.
- Command: `cargo test --test real_workflow -- --ignored --nocapture --test-threads=1` (explicit real model usage, normal `cargo test` skips this test).
- Scope: actual compiled `architalk --no-open` in isolated temporary project/config → loopback authenticated HTTP → real Claude/Codex conversation → design → independent critique → preview → immutable Markdown/manifest → restart and persisted state. SSE replay crosses a real TCP connection. Credentials are handled only by installed CLIs; tests do not print local auth tokens or copy credentials.
- Expectation: both backends complete the full path; failures and compatibility limits remain visible. Agent CLI-only smoke is additional evidence and cannot replace this test.
- I03 first execution: **2026-11-17 21:09:00 KST** start. Claude conversation completed, but the subsequent design run stayed `running` with no child CLI visible and no draft commit for over six minutes. Test had not passed; interrupted only the owned server/test process (PIDs 10454/10445) to avoid uninformative waiting and unintended next-backend calls. Server stderr had been suppressed by the test, so the cause is not yet established. This observability defect will be fixed before a focused rerun; no model result content or auth token is logged.
- Independent diagnosis: storage reviewer found AI adapter's error branches consumed a failed stdout/stderr/stdin JoinHandle but then polled it again during cleanup. Tokio panics on consumed output. That explains terminated child + uncommitted `running` state; SQLite 3/4 MiB actual commit/reopen passed (ST-11). AI owner is fixing and reproducing through the public adapter. Root added stage-only tracing, worker JoinError observation→persistent failure/termination gate, and design preservation when preparing critique fails. The exact real-model event that triggered the parser error still needs the instrumented rerun.

## I04 — real browser UI journey

- Status: prepared before execution. CUA inventory confirms the browser host is unavailable but native Google Chrome is available; attempt the native app with an isolated QA workspace.
- Scope: compiled local HTTP server and real browser DOM/layout, explicit fixture CLI only for repeatable UI behavior. Real model evidence is I03; this browser fixture is not counted as real AI.
- Expected: no fake content in product assets, welcome/context→conversation→draft/critique→save and settings model/effort→restart view. Token stays in local origin, removed from URL; no unrelated tabs/files changed. Exact actions/results and environment limits will be appended.
- CUA environment: browser host unavailable; native `getApp("Google Chrome")` returned `cgWindowNotFound`. No desktop browser scenario executed. The owned QA server was shut down (only its listener PID) after this attempt. A repository-owned headless Playwright test with isolated browser/project/process is being implemented separately; it does not connect to existing user tabs.
- Visual artifact check: integration owner viewed desktop review, mobile welcome (390px), and scrolled mobile settings screenshots from the browser test's isolated artifact folder. Main conversation/document panels and settings fields are legible, no visible overlaps in those views. Script/HTML attack text is displayed as text in the review/source panel. These screenshots are from the fixture-backed product path, not a real model output. Behavioral/viewport/network results are owned by the browser execution record.
- Final software browser evidence: [B01–B05](implementation-web-browser.md) preserves two failures (incorrect native-dialog focus assumption, then a real stale-snapshot race), their diagnosis and fixes. Final **desktop/mobile 2/2 passed**, Node policy/protocol/state **21/21 passed**; actual 390/768/1365px views, setting changes frozen for next run, reload recovery, disk file equality, no external requests/page errors. Initial desktop CUA failure remains a separate environment result; headless tests used only owned browser contexts/processes.

## I05 — integrated regressions after independent criticism

- Prepared before execution. Command: `cargo test --test workflow -- --nocapture`.
- Changes: revoke approval when a major finding is reopened; reject stale export preview; reserve the global process slot before help diagnostics; preserve completed design if the larger critique context cannot be prepared. AI consumed-handle panic and domain source/supersede/priority fixes are separately regression-tested by their owner and independently reviewed.
- Expected: all router/controller tests pass with the real SQLite/filesystem/subprocess fixture; no real model usage in this run.
- Actual: **2026-11-17 21:21:38 KST** start, compile 2.58s + tests 0.85s, exit 0, **6/6 passed**. Independent reviewer confirmed approval revocation, shared diagnostic reservation, empty-session recovery, source amendments and question priority changes. `cargo fmt` formatted the final integrated Rust files before this run.

## I06 — instrumented real Claude rerun and final checks

- Prepared before execution, after AI owner reproduced/fixed consumed JoinHandle panic and root added terminal supervision. Domain candidate schema v2 is now frozen and source-tested.
- Command: `cargo test --test real_workflow real_claude -- --ignored --nocapture --test-threads=1`.
- Required: conversation/design/critique all terminal, real HTTP/SSE/file/restart path; stage-only diagnostic logs, no model body/auth dumps. On failure, narrow to observed parser/contract boundary rather than repeat blindly.
- Static companion: `cargo clippy --all-targets -- -D warnings`; failures will be corrected and recorded separately.
- Actual I06: **2026-11-17 21:22:39 KST**, real Claude now failed promptly in 6.90s as a terminal error `invalid_cli_output / Unsupported Claude system event`, with confirmed process termination and committed error state. The prior infinite pending condition is closed. Adapter owner is identifying the exact harmless/unsafe event subtype using header-only inspection; unsupported events are not blindly ignored.
- Clippy first execution: exit 101, three lint failures (needless return in AI helper, Option unwrap after variant check in root recovery branch, derivable Settings default). They are implementation style issues; corrections precede the final lint run.

## I07 — assembled product suite and lint

- Prepared before execution. Commands: `cargo fmt`, `cargo test --locked`, `cargo clippy --all-targets -- -D warnings`.
- Scope: integration fixes plus configuration precedence (initial DB snapshot follows user config, explicit saved web override persists), all source-policy/process/files/HTTP tests in one crate. Real model tests remain opt-in and are tracked by I03/I06/follow-up records.
- Previous: targeted I05 6/6 passed; source/AI/files/UI agents recorded their own passing suites and independent criticism. Recent full-CLI parser metadata issue remains unresolved until the actual event profile is verified.
- Actual: start **2026-11-17 21:26:46 KST**. `cargo test --locked`: **65 top-level Rust tests passed, 3 opt-in real tests ignored**, including process groups, actual SQLite/filesystem/races, schema guards, source corrections and 7 controller/router regressions. Real-model ignored tests are not counted as executed here. Full test compile 4.24s; `cargo clippy --all-targets -- -D warnings` exit 0 (1.75s). No failures in this assembled run. Subsequent pinned `thinking_tokens` metadata handling is covered by the AI owner's incremental tests and the next actual CLI workflow.

## I08 — final real Claude/Codex workflow after protocol diagnosis

- Prepared before execution. Command: `cargo test --test real_workflow -- --ignored --nocapture --test-threads=1`.
- Observed root cause: Claude emits nonterminal `system/thinking_tokens` while reasoning. AI owner confirmed metadata header values and the official SDK schema without logging prompt/response content. Only that initialized, two-u64 metadata shape is accepted. AI unit suite is now 11/11; unknown/pre-init/malformed/post-terminal cases still fail closed.
- Required evidence: both real backends through actual compiled server, conversation/design/independent critique, TCP SSE, exact output file/manifest and restart. This is the final supported adapter/schema combination, not an API or mock replacement.
- Actual: **2026-11-17 21:28:57 KST** command start, **2/2 passed**, test duration **284.23s**. Claude: conversation/design/critic completed, CLI 2.1.326, requested default model+low effort, reported actual `claude-fable-5-2`, saved **28,840-byte** document with 5 review findings. Codex: all three roles completed, CLI 0.169.0, requested default model+low effort, actual model not supplied by CLI (`null`), saved **14,059-byte** document with no review findings. Both verified actual file/manifest, TCP SSE replay, clean server restart, persisted session/export and no automatic model rerun. These are draft outputs; test success is not a claim that the architecture is semantically perfect or user-approved.
- Final Rust lint/format companion: `cargo clippy --all-targets -- -D warnings && cargo fmt --check` exit 0 after the metadata-profile change (clippy 1.70s).

## I09 — release artifact and final repository checks

- Prepared before execution. Build: `cargo build --release --locked`, then release `--version`, `--help`, and `--config config.example.toml doctor` (help only, no model calls).
- Final source checks: staged diff whitespace, local docs/source links, lockfile/manifest consistency, no checked-in secrets/test outputs or private `.architalk` state. Node/browser final source results are in the browser owner record; no duplicate model calls are needed.
- Release build: **2026-11-17 21:36:06 KST**, `cargo build --release --locked` exit 0, optimized build **19.56s**. Release smoke **21:37:15 KST**: version `architalk 0.1.0`, help/options and example TOML `doctor` all exit 0. Both installed CLI versions compatible with termination confirmed; no model call in this smoke.
- Documentation/static: 16 Markdown files, 123 local links/anchors and package JSON/lock parsing passed; no broken reference, unclosed fence, machine-specific user path or literal capability token. This reference count is before this final result annotation.
- Staged review: `git diff --cached --check` exit 0; 45 intended source/config/lock/docs/test files only. No binaries, node_modules, browser captures, private SQLite data or authentication files are staged. The release binary remains a local ignored build artifact. No further model or full regression rerun is needed after documentation-only finalization.

## Final scope and independent review

- Implemented required path: Rust CLI+included web assets, local Claude/Codex, role model/effort settings, adaptive source-backed conversation/corrections, draft/critique/approval gates, SQLite/idempotency/revisions/events, immutable exports/restart/termination recovery.
- Independent mandatory findings were handled: process task double-poll panic, unsupported harmless CLI progress metadata, root/state lock and FIFO safety, source supersession/history/unknowns and risk priority, diagnostic concurrency/no-session recovery, approval revocation/preview freshness, UI stale snapshots. Writer/critic ownership was separated and final changed scopes were re-reviewed.
- No claim: all architectures are correct, arbitrary CLI versions/managed cloud policies are attested, Windows/Linux/network-filesystem support, parent SIGKILL/power-loss cleanup, real disk-full, all browsers/assistive technologies or long-duration load. These are not silently marked passed. v1 support and design differences are in `docs/implementation.md` and README.
