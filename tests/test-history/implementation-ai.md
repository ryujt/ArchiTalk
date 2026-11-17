# AI adapter implementation verification

Scope: `src/ai.rs`, CLI probes, adapter tests. Initial baseline: `main` at `1052929`, no product Rust implementation. Previous index only contains design/UI checks and no real model call evidence. Model calls authorized for this implementation task; no installation, login changes, or separate purchase.

## AI-P0-20261117-205432 — CLI capability discovery

Started: 2026-11-17 20:54:32 KST. Ended: 20:54:44 KST. Local macOS, Claude Code 2.1.326, Codex CLI 0.169.0. Commands: `claude --version`, `codex --version`, both root helps, `codex exec --help`, `codex features list`.

Result: partial verification. Both installed CLIs expose required noninteractive JSONL entry points and isolation options. Claude supports `--restricted`, `--safe-mode`, empty `--tools`, strict MCP and explicit settings; effort help lists low/medium/high/xhigh/max. Codex supports exec-only `--ignore-user-config`, `--ignore-rules`, ephemeral, read-only, JSONL. `codex --ignore-user-config features list` failed with exit 2 because this option belongs to `exec`; no model was invoked. Feature listing shows shell_tool/unified_exec/hooks/plugins/apps and other capabilities that must be disabled in the adapter. Discovery was performed before its history entry was created; subsequent tests create entries first. No credentials or user configuration contents were collected.

## AI-P1-20261117-2055 — OS boundary and minimal real responses

Started: pending (entry created before execution). Criteria: a child launched through the same OS profile cannot create a file inside canonical project root; actual CLIs run outside project, no tools/MCP/hooks, report minimal terminal success JSONL; output captured only in temporary evidence files. Follow-up: implement exact observed protocol and rerun through product entry point. Status: running.

AI-P1 result: passed local external spike, 2026-11-17 20:55:20–20:55:29 KST. Command: `python3 examples/cli-probe/probe.py --real`. OS sandbox `touch` returned 1 and target did not exist. Both real CLIs returned exit 0 and `ARCHITALK_OK`: Claude `result/success` with `tools=[]`, `mcp_servers=[]`, `permissionMode=dontAsk`, model `claude-fable-5-2`; Codex final agent_message plus `turn.completed`. Claude reports total_cost_usd 0.0492178; Codex does not report monetary cost. Codex reports a pre-turn diagnostic item that Code Mode is unavailable because code-mode host was disabled, and explicitly fails that capability closed, then completes the text turn. This diagnostic is not a terminal failed turn. Probe cwd was a private OS temp directory outside project. Raw temporary evidence contains only this synthetic prompt/response and operational metadata; no auth files were opened/copied. The probe proves project writes blocked, not general filesystem read isolation or abrupt-parent-death cleanup. Product-path rerun follows.

## AI-P2-20261117-implementation — product adapter verification

Started: pending; entry created before execution. Baseline: adapter under construction following AI-P1 exact flags. Expected: parser rejects malformed/truncated/oversized/tool output; subprocess termination handles cancellation/timeout/descendants; argument construction retains boundaries; actual product API returns both real responses. Status: not yet run.

AI-P2a build observation: `cargo check --lib` started 2026-11-17 20:59:00 KST, completed with exit 101 (observed 21:02 KST). AI module type-check produced no AI-specific diagnostic; build was blocked by concurrently not-yet-written app/http modules and a storage Result<&()> mismatch. This is blocked integration, not an AI behavior pass. Follow-up: rerun after owning modules are ready; no other owner's file changed.

AI-P2a timestamp correction: the preceding observation's “21:02 KST” was a transcription error. Actual tool clock on observation was 2026-11-17 21:00:33 KST; command execution began 20:59:00 KST. Exit 101 and diagnostics remain unchanged.

## AI-P2b-20261117-isolated — same-source isolated adapter tests

Entry created before execution. Precondition: root crate HTTP module is still being assembled by another owner. Run a temporary Cargo harness whose lib includes the product `src/ai.rs` and whose integration target is the unchanged `tests/ai_adapter.rs`; no adapter copy or product fallback. Expected unit/parser and actual OS process fault-injection checks pass. Real model test remains ignored in this run. Start/end and actual results follow.

AI-P2b result: failed, 2026-11-17 21:00:56–21:01:04 KST, exit 101. Same-source isolated harness: 8 parser/argument/bounds tests passed; integration 5 passed, 1 failed, 1 real-model test intentionally ignored. Cancellation correctly returned `cancelled` but conservatively marked `termination_unknown=true` for a parent+sleep group; expected confirmed process termination failed. Timeout of a SIGTERM-resistant process, dropped future, project-write deny, outside cwd, stdin boundary, and missing terminal checks passed. Investigate process-group liveness after termination (possible adopted zombie) and preserve conservative semantics; do not weaken the check without observing why. Harness resolved cached uuid 1.29.2, whereas root lock currently pins 1.26.1; final root-crate rerun remains required.

## AI-P2c-20261117-cancel-diagnostic

Entry created before rerun. Add test-only PID/PPID/PGID/state diagnostics for a failed cancellation assertion, then rerun exactly that product integration test via the existing same-source temporary harness. No product behavior changed. Expected: identify whether process execution or pipe/reap state remains.

AI-P2c result: passed one focused rerun (exit 0, one case, 0.44 s), but original intermittent failure is unresolved. No PID diagnostics appeared because termination was confirmed. Added product warning fields (`pid`, `group_stopped`, `drains_clean`) and error suffix distinguishing group vs pipe cleanup; these fields contain no prompt/output. Next bounded repeat reproduces original parallel suite before claiming a fix.

## AI-P2d-20261117-cancel-reproduction

Entry created before execution. Same-source harness; run full unit/integration suite with default parallel scheduling after diagnostic-only changes. Expected: either reproduced phase evidence or a bounded series showing whether cancellation liveness depends on concurrency. No real model call.

AI-P2d result: passed 8 unit and 6 integration checks; real model case ignored. Same-source harness exit 0, unit 0.01 s and integration 4.31 s. The original AI-P2b conservative unknown-termination observation remains preserved; its group-vs-pipe cause was not reproduced by this rerun.

Independent root review: requested model must not populate resolved model (fixed; Codex returns None when CLI does not report it). Diagnostic child termination uncertainty must survive adapter boundaries (Diagnostic now has defaulted `termination_unknown`, forwarded by generate; application owner handles persistent gating). Managed configuration can outrank ordinary flags: [Codex managed configuration](https://learn.chatgpt.com/docs/enterprise/managed-configuration) and [Claude managed settings](https://code.claude.com/docs/en/managed-settings). Local policy and MDM paths now cause fail-closed diagnostics; Claude remote cache accepts only the observed non-executable channelsEnabled flag. Cloud-delivered policy changes cannot be fully attested by help and remain a deployment trust boundary; global home/read isolation is not claimed.

## AI-P3-20261117-root-product

Entry created before execution. Root crate now assembles. Commands: `cargo test --lib ai::tests -- --nocapture`, `cargo test --test ai_adapter -- --nocapture`, then explicit ignored real installed CLI case. Verify same production adapter with root Cargo.lock. Start/end and actual outcomes follow. No automatic real-call retry.

AI-P3 initial root results (2026-11-17 21:05 KST): 8 adapter unit tests and 6 integration tests passed using root Cargo.lock; one real test ignored. Independent storage-owner Critic AI-C1/C2/C3 accepted before final rerun: sandbox probe termination_unknown now preserved; Claude remote cache opened nofollow/nonblocking, regular-file/length checked, bounded read; Codex requires one turn.started before agent/final events. Added requested-vs-resolved, authentication, diagnostic/probe uncertainty, and out-of-order event regressions. New checks are distinct from the above initial 8+6 pass.

## AI-P4-20261117-final-adapter

Entry created before execution. Final adapter source after independent findings; root crate unit and integration suite, then explicit ignored real installed CLI smoke. No automatic real model retry. Expected terminal success plus final message from both backends; original project OS write denied; no user model value represented as resolved model without CLI evidence. Actual timestamps/results follow.

## AI-CRITIC-STORAGE-20261117 — independent read-only review

2026-11-17 21:07–21:09 KST. Reviewed `src/storage.rs`, `src/files.rs`, and their existing integration scenarios independently from their author. No changes made in those files. Findings sent to owner/root: S-C1 important contract counterexample — renaming active `.architalk` then recreating a regular state directory can create a second lock inode for the same root; root-directory inode locking should be considered and reproduced. S-C2 recommended — routed SQLite `openat` should include O_NONBLOCK before regular-file metadata validation, to avoid a replacement FIFO blocking in read-only open paths. Existing fd-relative nofollow publication, identity checks, immutable outputs, journal hardlink/symlink checks, and SQLite transaction ownership were otherwise coherent in the reviewed scope. Owner's resolution/retest belongs to storage history; this review does not claim unexecuted adversarial scenarios passed.

AI-P4 intermediate: root adapter unit tests passed 10/10. Root integration failed 1/7 executed (6 passed, real test ignored): requested/resolved Codex test was blocked by managed_policy_present. A normal macOS Codex application preference plist shares the MDM domain name; rejecting every ordinary user preference file is overbroad. Fix the detector to inspect presence of the two documented policy payload keys rather than treating all app preferences as enforced policy. Do not remove checks for actual policy sources. Original failure preserved; follow-up rerun follows.

## AI-P5-20261117-policy-detector-fix

Entry created before execution. Actual ordinary `com.openai.codex.plist` contains neither documented managed payload key. Detector now uses CoreFoundation preference lookup for only `config_toml_base64`/`requirements_toml_base64`, including effective MDM overrides; system config and explicit managed preference locations stay fail-closed. No preference or login file was modified. Re-run root adapter unit/integration checks and explicitly invoke real test only if those pass.

AI-P5a: command `cargo test --lib ai::tests --test ai_adapter -- --nocapture` exited 0; 10 unit tests passed. The `ai::tests` name filter also applied to integration target, so integration ran 0/8; this is not an integration pass. Explicit unfiltered integration command follows.

AI-P5b result: `cargo test --test ai_adapter -- --nocapture`, started 2026-11-17 21:09:40 KST, root crate exit 0, 7 passed + 1 explicitly ignored real-call test, integration runtime 4.35 s. Includes requested-model vs actual-model regression, cancellation/process group, TERM-resistant timeout, dropped future, outside cwd/project-write deny, and missing terminal success. All final adapter unit cases also passed (10/10). Next command explicitly invokes the paid smoke once for both installed CLIs.

## AI-P6-20261117-real-product-adapters

Entry created before execution. Command: `cargo test --test ai_adapter real_installed_clis_return_terminal_response -- --ignored --nocapture`; synthetic exact-reply prompt only, real installed/authenticated Claude 2.1.326 and Codex 0.169.0, root Cargo.lock, product generate public API, no model/effort override. Expected exact ARCHITALK_OK after valid terminal success. Calls are authorized for implementation and will not be automatically retried.

AI-P6 result: passed actual environment, command started 2026-11-17 21:10:07 KST, test runtime 9.06 s, exit 0, one test containing two actual model calls. Public `generate` returned exact ARCHITALK_OK from installed Claude (`2.1.326 (Claude Code)`, resolved_model=`claude-fable-5-2`) and Codex (`codex-cli 0.169.0`, resolved_model=None because JSONL does not expose it). No model fallback, automatic retry, login mutation, or installation. This verifies final product adapter source, root dependency lock, OS project-write probe, stdin/JSONL, terminal validation and bounded drains. It does not verify arbitrary model/effort combinations or enterprise/cloud policy overrides.

Final independent review closure: storage owner accepted S-C1/S-C2, added project-directory inode flock before state creation and O_NONBLOCK before SQLite regular-file validation. ST-09 reproduced FIFO blocking under a bounded child; ST-10 confirms prompt termination and 26/26 storage/files checks. Reviewed final call sites/ownership again: Store retains locked root and exposes ProjectFiles from the same pinned inode; no required residual finding in that scope. This reviewer did not rerun the owner's full suite.

Final adapter status: 10 unit + 7 local fault-injection integration cases pass; explicit actual-model smoke passes both installed backends. Earlier failures and zero-case filtered run remain above. One early cancellation run conservatively returned termination_unknown; focused and two subsequent complete root/same-source reruns confirmed cleanup. Its transient group/pipe cause was not reproduced; permanent phase diagnostics and controller's manual-confirmation gate preserve safe handling if it recurs. No claim of global filesystem read isolation, hostile executable containment, arbitrary detached-process cleanup, or abrupt parent SIGKILL cleanup. macOS project-write sandbox is verified; other platforms fail closed. Named Codex profiles are explicitly unsupported. Requested model/effort snapshots remain separate from CLI-reported actual model; effort support depends on chosen model. Raw real-call evidence was synthetic and stored only under private OS temporary directories; no credentials copied. Further real calls are intentionally opt-in.

Final static checks observed 2026-11-17 21:11:08 KST: `rustfmt --edition 2024 --check src/ai.rs tests/ai_adapter.rs` and `python3 -m py_compile examples/cli-probe/probe.py` both exit 0. Generated Python cache removed. Implementation/test files are ready for root integration; no additional code changes after successful real-product smoke.

## AI-P7-20261117-consumed-handle-regression

Entry created before execution. Independent storage Critic found a definite error path: stdout/stderr/writer task output is consumed on Err, but cleanup decides completion from parsed-value presence and re-awaits that same JoinHandle. Tokio explicitly panics on consumed output. This can kill the worker after process cleanup while leaving the application run running. Root authorized reopening frozen AI module for this required fix. Add public generate fault injection for malformed stdout, aggregate stderr overflow, and early stdin close; first run against unchanged adapter must show a panic, then track task completion independently of task success and re-run. No real model call.

AI-P7 baseline result: public malformed-stdout case reproduced exact Tokio panic “JoinHandle polled after completion”, test exit 101 (0 passed, 1 failed). This was a product bug, not CLI authentication or model behavior. Added stdout_done/stderr_done flags independently from successful result storage, and set writer completion before inspecting its result. Cleanup awaits only tasks whose output has not already been consumed.

## AI-P8-20261117-consumed-handle-fix

Entry created before verification. Re-run the grouped public fault case (malformed stdout, >16MiB stderr, closed stdin/1MiB prompt), then full adapter suite. Expect AiError returned without worker panic and process cleanup confirmed. Root separately monitors any unexpected worker panic into a persistent terminal recovery gate.

AI-P8 focused result: exit 0; public generate fault group passed all three injected errors without panic (1.11 s). Malformed stdout returns invalid_cli_output; >16MiB stderr returns output_limit; early stdin close with 1MiB input returns cli_io_error. Every case confirms process cleanup. No real model call. Full adapter/unit regression follows before refreeze.

## AI-P9-20261117-final-error-path-suite

Entry created before execution. Command `cargo test --lib --test ai_adapter -- --nocapture`; execute all local parser/subprocess cases after consumed-handle fix, real model case stays ignored because root is validating the final full product path.

AI-P9 final result: start 2026-11-17 21:19:49 KST, root Cargo.lock suite exit 0; unit 10/10 and local integration 8/8 passed, actual model test intentionally ignored (previous real adapters passed, root now runs final full workflow). Integration runtime 4.35 s. This includes all three consumed-task error cases. Adapter code refrozen after this required independent finding.

Independent storage Critic final closure: read-only source review confirms stdout/stderr/writer consumed flags are set before success/error inspection, both select and cleanup use those flags, and failures return before successful-value unwraps. Thus success/error/JoinError cannot re-poll the same consumed task. Public fault cases check absence of tokio::spawn JoinError as well as expected error code/cleanup. No required finding remains in this fix's reviewed scope; dynamic results are AI-P8/P9.

## AI-P10-20261117-real-system-header-discovery

Entry created before execution. Root final real workflow now correctly fails fast instead of leaving running state, exposing a previously unsupported Claude system event in the first conversation. Capture exactly one real CLI call using synthetic domain-v2 intro+README, effort low, production restriction flags/environment, outside cwd and OS write denial. Only sanitized type/subtype and init tool/MCP/permission metadata may be printed; no prompt, response text, auth material, or raw CLI outputs retained in this header-only mode. Do not loosen unknown-event checks before identifying the actual subtype.

AI-P10 result: real single capture exited 0 with terminal result/success, 2026-11-17 21:24:10–21:25:04 KST. Header-only mode printed no prompt/response/auth material, but emitted hundreds of redundant token-event headers and the tool truncated the middle where system subtypes appeared. No raw stream was retained, so the needed exact subtype cannot honestly be recovered from that result. Corrected the probe to retain only unique sanitized system headers, permission/tool/MCP counts, and a metadata-only JSON file; stop the next owned process as soon as a new system subtype is identified. This is a capture correction, not a guessed parser compatibility expansion.

## AI-P11-20261117-system-only-capture

Entry created before execution. Same synthetic prompt and restrictions, one short real call, unique system headers only, stop on first non-init/status system event. Preserve no raw model text or authentication material. Use exact observed subtype and primary official semantics before implementing a pinned parser rule.

AI-P11 result: exact new event identified, 2026-11-17 21:26:36–21:26:40 KST. Headers: system/init (tools_count=0, mcp_count=0, permission_mode=dontAsk), system/status, system/thinking_tokens. Owned probe deliberately stopped at that event (exit 143); no response text or raw stream retained. [Official SDKThinkingTokensMessage reference](https://code.claude.com/docs/en/agent-sdk/typescript#sdkthinkingtokensmessage) identifies it as reasoning progress estimates, with numeric estimated_tokens and estimated_tokens_delta. This is not permission or tool execution. Root informed before allowing it.

## AI-P12-20261117-thinking-metadata-profile

Entry created before execution. Pinned Claude 2.1.326 parser now accepts only system/thinking_tokens after init, with both nonnegative integer token fields. It yields no delta or final message and does not expose reasoning content. Unknown system subtypes still fail, but error includes only a sanitized ASCII identifier (alphanumeric/underscore, <=64), otherwise 'unknown'. Regression checks telemetry cannot establish success, malformed numeric shape/pre-init/post-terminal fail, and unknown subtypes remain rejected without leaking unsafe subtype text. Also removed clippy's needless return in group_alive. Root's concurrently started generic suite may predate this change, so this dedicated epoch verifies final AI source separately.

AI-P12 unit result: root Cargo.lock, exit 0, 11/11 unit checks pass including the new exact metadata rule and unknown-event rejection. No actual model call in this unit run. Follow-up local subprocess suite checks error/termination paths after the parser addition; no repeated real adapter smoke.

AI-P12 final result: root Cargo.lock unit 11/11 and integration 8/8 pass (real adapter test intentionally ignored; no repeated model smoke), integration started 2026-11-17 21:28:22 KST, runtime 4.36 s. Independent storage Critic confirmed this exact metadata rule cannot change text/model/terminal, rejects pre-init/post-terminal/malformed numeric values, and retains fail-closed unknown-subtype handling with sanitized error identifiers. No required residual finding in reviewed compatibility scope. Rust formatting and Python probe syntax checks passed; generated Python cache and synthetic prompt file removed. Source refrozen for root's final real full workflows, whose results are recorded in integration history separately.
