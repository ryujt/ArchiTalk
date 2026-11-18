# Project-local AI runtime and CLI profile investigation — 2026-11-18

Scope: `src/ai.rs`, `tests/ai_adapter.rs`, and this record. Shared config/main,
filesystem cleanup helpers, workspace reset guards, HTTP, and browser behavior are
owned by the other tasks. No authentication/configuration/history files in the
user's vendor homes are deleted, copied, or printed by this task. No model call
or dependency installation is authorized here; the root task owns the one final
synthetic installed-Claude smoke call.

## Plan and diagnostic evidence

1. Inspect installed CLI paths, `--version`, and `--help` without invoking a model.
2. Keep explicit compatibility profiles and all required restriction checks;
   report observed version and resolved executable when rejecting a version.
3. Create pinned, private per-request runtime directories only beneath an existing
   project `.architalk/runtime/`. Missing/replaced state must fail closed rather
   than resurrect a reset workspace. Generic no-project diagnostic compatibility
   may use a temporary directory containing no project data.
4. Apply the runtime-only OS write boundary to project version/help preparation
   as well as generation. Keep existing shared authentication read-only. Redirect
   temp/cache/SQLite/log destinations locally; never loosen shared-home writes if
   a CLI requires an additional startup path.
5. Verify with quota-free protocol/process/filesystem fixtures, then hand off the
   final installed-CLI smoke to root. Preserve failures and unverified limits.

Installed CLI evidence: Claude resolves through `/Users/ryu/.local/bin/claude` to
its native package binary and reports **2.1.327 (Claude Code)**. Codex resolves
through `/Users/ryu/.local/bin/codex` and reports **codex-cli 0.169.0**. The previous
adapter accepted only the complete Claude 2.1.326 output string, so the reported
patch update was rejected before a model call. The current Claude 2.1.327 help
contains all 14 required option names and the `dontAsk` permission mode.

The adapter now explicitly permits Claude 2.1.326 and 2.1.327, retains Codex
0.169.0, and still rejects unknown versions or missing required flags. An unknown
version message includes the actual observed version, resolved executable path,
and supported list. It does not infer support for arbitrary future patches.

Privacy references checked against primary sources:

- [Claude settings](https://code.claude.com/docs/en/settings): config-directory
  redirection covers settings/history/plugins; moving it is not proof of shared
  OAuth/keychain authentication continuity.
- [Claude environment variables](https://code.claude.com/docs/ko/env-vars):
  supported temporary-directory and nonessential-traffic controls.
- [Codex 0.169.0 configuration source](https://github.com/openai/codex/blob/rust-v0.169.0/codex-rs/config/src/config_toml.rs)
  and [types](https://github.com/openai/codex/blob/rust-v0.169.0/codex-rs/config/src/types.rs):
  exact `history.persistence`, `sqlite_home`, and `log_dir` keys. Local `exec
  --help` confirms ephemeral sessions and that ignoring user config retains the
  normal authentication home.

## Executed checks and failures

- `cargo test --test ai_adapter patch -- --nocapture`: **3 passed**, no failures.
  Quota-free fixtures cover both supported Claude patches, unknown patch refusal
  with observed/path evidence, and restriction-flag enforcement for 2.1.327.
- After the first runtime adapter draft, `cargo check --tests` passed; it exposed
  a temporary unused shared-settings helper warning in another task's WIP source.
- The first full adapter rerun stopped at compilation: `src/http.rs` returned
  `Json(Result<Value, AppError>)` after the settings API changed. No adapter test
  executed in that run. The HTTP owner subsequently corrected the mismatch.
- A targeted quota-free `zero_exit_without_terminal_success_is_rejected` run
  passed with the new project-runtime sandbox before the remaining fixture
  observation files were moved into runtime/stdout.

Further adapter/runtime test results and the final root smoke are recorded below
when they actually execute. Do not interpret version/help acceptance as proof of
model login, selected-model support, or the final native CLI runtime behavior.

## Final quota-free implementation results

- Final `cargo test --test ai_adapter -- --nocapture`: **13 passed / 0 failed /
  1 explicitly ignored real-model test**, 4.5 seconds (repeated after literal-only
  lint cleanup with the same result).
- `cargo test --lib ai::tests -- --nocapture`: **12 passed / 0 failed**.
- `cargo clippy --all-targets -- -D warnings`: **passed**. Two preliminary lint
  runs rejected a redundant error closure and four no-longer-parameterized test
  format strings after fixture records moved into stdout/runtime. Those lint
  errors were corrected; no policy exception or assertion reduction was used.
- `git diff --check -- src/ai.rs tests/ai_adapter.rs`: passed.

The new tests exercise both supported Claude versions, unknown-version/path
reporting, required-option refusal, project version/help write restrictions,
missing state without recreation, state/runtime symlink refusal, a writable
private runtime with project-body/external-history writes denied, exact stdin
prompt handling, local temporary/cache paths and safe-mode/traffic controls,
normal cleanup, validated Codex TOML runtime paths/history disabling, cancellation,
timeouts, dropped futures, and terminal/parser/process faults.

`OwnedRunDirectory` pins root/state/runtime/run directories with NOFOLLOW handles,
rechecks ancestor identities, uses the pinned run descriptor for child cwd, and
cleans only the matching directory tree through the dirfd helper. It opens an
existing `.architalk` and never recreates one removed by a workspace reset. The
same OS profile limits project version/help preparation and generation to writes
inside that private run (plus `/dev/null`). Existing HOME/vendor authentication is
read-only; no credential copies or shared-history write exceptions are added.

The root task separately reported **ISO-REAL-01**, one synthetic installed-Claude
2.1.327 request, passed in **2.09 seconds** with runtime-only writes and cleanup;
the CLI reported model `claude-opus-5-5`. This task did not issue that request.
No repeated architecture/package generation was used for this smoke. Root owns
any additional explicit tiny Codex smoke and its final evidence; this task's
source and tests are frozen pending integration findings.

## Codex ISO-REAL-02 failure investigation and correction

The root task's one installed-Codex synthetic request (ISO-REAL-02) failed after
7.12 seconds with the old generic `cli_failed` message. Claude ISO-REAL-01 stayed
passed. This task issued no additional model request. The following controls used
an OS profile denying all IP networking; only local Unix IPC was allowed after an
initial stricter control, and no real project prompt was supplied.

- Empty stdin with the original shared Codex home: exit 1 after approximately six
  seconds, zero stdout events. The fixed categories showed a nonfatal PATH-alias
  cache permission warning followed by the expected empty-input rejection.
- A constant synthetic stdin with the same shared home: exit 1 after 5.58 seconds,
  zero stdout events, with a further context-free permission failure. Local Unix
  IPC allowance did not change the result. Shared file-write permission remained
  denied throughout; the PATH-alias warning alone was not called the root cause.
- A metadata-only macOS log query returned zero matching file-write denials and no
  access error. No raw OS logs were emitted. An earlier Python source download
  hit a local certificate-store error; verified HTTPS curl was used instead.
- The initial empty private-home control reached a deliberate network-blocked
  wait and hit the 20-second test timeout. Process-group shutdown was confirmed,
  but the first harness discarded partial diagnostics on timeout. That harness
  was corrected to stop the child and retain only bounded classified output;
  the initial failed control remains recorded here rather than counted passed.

Independent primary-source review identified the required bootstrap write:
[Codex 0.169.0 installation identity](https://github.com/openai/codex/blob/rust-v0.169.0/codex-rs/runtime-agent-instance/src/installation_id.rs#L16-L47)
opens `CODEX_HOME/installation_id` for read/write/create even when an identity
already exists. The in-process app-server startup propagates that error.
`sqlite_home`, `log_dir`, and history controls do not move this file. Thus a
read-only shared Codex home cannot satisfy this startup contract.

The correction sets the child process's `CODEX_HOME` to its private run home.
Installation identity, bootstrap locks, cache, and temporary state are local.
Only native `auth.json` is referenced through a read-only symlink to the shared
source. The source is checked with NOFOLLOW for a regular file owned by the
current user, private permissions, and bounded size; credential contents are not
read or copied by ArchiTalk. An explicit file-auth storage override is necessary:
[the pinned auth implementation](https://github.com/openai/codex/blob/rust-v0.169.0/codex-rs/login/src/auth/storage.rs)
reads file authentication normally, while keyring keys depend on the home path.
Keyring-only credentials therefore cannot silently migrate to this private home.
Shared authentication refresh writes remain OS-denied; the user can refresh the
normal CLI login explicitly in a terminal. Cleanup unlinks the private pointer
without following or deleting its shared target.

Final no-network positive control, using the actual installed Codex and production
private-home/auth-pointer setup: **passed**, `thread.started` and `turn.started`
were observed, with only network-blocked retry diagnostics afterward. The harness
stopped the process group at 20 seconds, retained no raw output, and cleaned the
runtime. This distinguishes the former pre-thread permission failure from a
working bootstrap without sending a model request.

Failure reporting now preserves bounded stderr when stdout fails first, rather
than discarding the stderr join result. It emits only a fixed reason/component/
severity vocabulary (at most eight distinct categories), terminal event kinds,
and exit codes. It never emits raw stderr, filesystem paths, prompts, or tokens.
The retained stderr limit remains 64 KiB. Fixtures verify secret/path omission and
the stdout-failure/stderr-drain ordering.

Final checks after this correction:

- Adapter tests: **14 passed / 0 failed / 1 real-model test ignored**.
- AI unit tests: **14 passed / 0 failed / 3 explicit installed-CLI offline probes
  ignored by ordinary runs**. The private-home positive control was separately
  executed as described above.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `git diff --check -- src/ai.rs tests/ai_adapter.rs`: passed.

Source and tests are frozen. The root task alone owns any one explicitly approved
post-correction tiny Codex model request and its separate result; this record does
not claim that paid recheck has already passed.
