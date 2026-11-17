# Storage / project files implementation verification

Baseline: main `1052929`; new `src/storage.rs`, `src/files.rs`, and their tests. Related prior history: `20261117-tobe-validation.md` covers design only; no prior product execution exists. Local real filesystem and SQLite checks use the product public entry points. macOS local disk is the observed environment; Linux behavior requires its own run.

## ST-01 — P0 directory publication

- Started: 2026-11-17 20:56:44 Asia/Seoul; status: failed before execution (unrelated root modules absent).
- Command: `cargo test --test files_spike -- --nocapture`.
- Acceptance: pinned directory open refuses symlink ancestors; existing export content survives; exclusive directory reservation rejects collision; manifest appears after matching files; exact hash replay succeeds.
- Evidence level: local real filesystem, no doubles.

- ST-01 result: 0 tests executed; compilation blocked by concurrently unimplemented ai/app/domain/http/storage modules. No product behavior conclusion. Follow-up ST-02 uses a temporary harness that imports the exact product source without stubs.

## ST-02 — P0 isolated product-source harness

- Started: 2026-11-17 20:59 Asia/Seoul.
- Command: `cargo test --manifest-path "$HARNESS/Cargo.toml" --test files_spike -- --nocapture` (temporary harness imports repository `src/files.rs`, same test file; no copied product implementation).
- Acceptance and environment: ST-01, actual macOS filesystem.
- Status: running.

- ST-02 result: **failed**, 0 passed / 1 failed. The actual macOS kernel rejects `/dev/fd/<directory-fd>/child` with ENOTDIR. This invalidated the original directory-listing approach before consumer integration. Replaced listing with `fdopendir/readdir`; SQLite uses its Unix VFS syscall hooks to route an internal path marker through `openat/fstatat/faccessat/unlinkat` while retaining SQLite journal and locking. Follow-up ST-03.

## ST-03 — corrected directory and SQLite VFS spike

- Started: 2026-11-17 21:06 Asia/Seoul.
- Command: temporary product-source harness `cargo test --offline --test files_spike --test storage_spike -- --nocapture`.
- Acceptance: ST-02 plus actual SQLite commit/reopen, project OS lock, state/db/journal symlink denial, pinned state-directory replacement never follows external symlink.
- Status: running.

- ST-03 result: compilation failed before test execution (`Result<&(), Error>` versus `Result<(), Error>` in one-time VFS registration). Fixed by mapping the borrowed unit result. No behavior conclusion.

## ST-04 — corrected VFS registration

- Started: 2026-11-17T21:00:27+09:00.
- Command and acceptance: ST-03, exact product-source harness.
- Status: running.

- ST-04 result: **passed**, files 1/1 + storage 3/3, ended 2026-11-17T21:03:59+09:00. This is actual macOS kernel/SQLite evidence. Directory-swap test observed SQLite writing only through its original pinned directory, leaving the substituted external symlink target empty.

## ST-05 — storage/files contract and failure recovery

- Started: 2026-11-17T21:03:59+09:00.
- Command: `cargo test --test files_spike --test files_contract --test storage_spike --test storage_contract -- --nocapture`.
- Scope: original P0 cases; partial export and manifest recovery; wrong/missing identity; external edits and immutable natural revisions; Unicode/reserved/traversal/case validation; custom export base; permission failure/retry; scanner selection/size/binary/secrets; concurrent parent symlink swaps; schema/corruption preservation, migration backup, post-open journal symlink and real subprocess lock.
- Status: running against repository crate.

- ST-05 result: files 11/11 passed; storage contract 5/6 passed, 1 failed; Cargo stopped before storage_spike. Post-open journal symlink test correctly observed commit rejection and external marker preservation, but incorrectly expected reads to work while the unsafe journal remained. SQLite also rejects reads in that state. Test now verifies read rejection, removes only its own test symlink, and verifies the last committed snapshot. Added VFS post-open hard-link/regular-file checks before any truncation to preserve the same boundary after open.

## ST-06 — corrected unsafe-journal expectation and VFS hard-link guard

- Started: 2026-11-17T21:04:37+09:00.
- Command and coverage: ST-05.
- Status: running.

- ST-06 result: same files 11/11 pass + storage 5/6 pass; intended test fix did not match rustfmt output, so the unchanged assertion repeated ST-05 failure. Verified source location and replaced exact line before next run. This was test editing failure, not a new product defect. Correction: ST-02/ST-03 manual start-time entries were approximate/prefilled; only tool-generated ISO timestamps from ST-04 onward should be used as precise clock evidence.

## ST-07 — verified assertion correction plus post-open hard-link case

- Started: 2026-11-17T21:04:59+09:00.
- Command: ST-05 with additional real hard-link injection after Store::open.
- Status: running.

- ST-07 result: **passed**, 21/21 (files_contract 10, files_spike 1, storage_contract 7, storage_spike 3), ended 2026-11-17T21:05:27+09:00. No skipped tests on the observed non-root macOS account. Product source and real SQLite/kernel/filesystem/subprocess were used.
- Remaining evidence limits: Linux was not executed; Windows is outside v1. Forced power-loss durability, actual disk-full and network filesystems were not exercised. Permission-denied retry and manually interrupted partial bundle states cover the local failure/recovery mechanisms. A directory created before its identity is persisted is deliberately preserved and rejected rather than auto-adopted. Existing unexpected temporary files cause conservative conflict; they are never deleted as presumed owned files.
- Developer and self-tester: storage implementation subagent, inherited session model (no override). Independent Critic is being assigned by integration owner; self-review is not reported as independent.

## ST-08 — lifetime root identity across commands

- Started: 2026-11-17T21:08:17+09:00.
- Change: integration review identified that path-based compatibility APIs reopen the selected root on each call. Added lifetime-pinned `ProjectFiles`, derived from Store's original root directory handle, and moved the same product operations to handle-based helpers. Existing path APIs wrap this implementation. No file operation through ProjectFiles reopens its root pathname.
- Command: ST-05 suite (plus two root-replacement cases).
- Acceptance: replacing the original root pathname with a regular directory or an external symlink cannot change the source of selected evidence or target of exports/SQLite. New operations continue on the original directory inode, and external replacement directories remain untouched.
- Status: running.

- ST-08 result: **passed**, 23/23 (files_contract 12, files_spike 1, storage_contract 7, storage_spike 3), ended 2026-11-17T21:09:07+09:00. Both actual root-path replacement cases used the Store-derived pinned gateway and preserved the external/replacement directories.

## ST-09 — independent Critic project-lock and FIFO findings

- Started: 2026-11-17T21:09:07+09:00.
- Independent Critic: AI implementation subagent (inherited model; no override). S-C1 accepted: replacing regular .architalk could create a second workspace.lock inode, so the original project directory fd now holds an OS flock before all state setup. S-C2 accepted: SQLite routed opens now use O_NONBLOCK before regular-file validation to prevent a malicious FIFO journal from blocking.
- Command: ST-05 suite with root inode lock replacement and subprocess-bounded FIFO fault injection.
- Status: running.

- ST-09 result: files 13/13 passed, storage contract 9/10 passed; FIFO test reproduced the Critic's actual blocking defect and killed its child at the 3-second bound. Root-inode flock replacement test passed. The attempted O_NONBLOCK source edit had not matched the formatted expression, so the FIFO case still exercised the pre-fix code. Applied the exact expression and verified it in source before rerun. storage_spike was not reached.

## ST-10 — FIFO nonblocking open correction

- Started: 2026-11-17T21:09:23+09:00.
- Command and acceptance: ST-09, with verified O_NONBLOCK addition.
- Status: running.

- ST-10 result: **passed**, 26/26 top-level test cases (files_contract 12, files_spike 1, storage_contract 10, storage_spike 3), ended 2026-11-17T21:09:35+09:00. storage_contract includes two subprocess entrypoint helpers; its parent lock/FIFO cases execute these helpers in real child processes. The FIFO child now exits in 0.04 seconds rather than reaching the 3-second kill bound. Independent Critic S-C1/S-C2 accepted, fixed, and regression-tested.

## Cross-review handoff — AI adapter (static review)

- At 2026-11-17T21:10:08+09:00, storage author acted as independent Critic for `src/ai.rs`; no AI source files were edited. AI-C1 required preserving probe termination uncertainty; AI-C2 required bounding remote-policy reads before allocation and rejecting unsafe files; AI-C3 required Codex turn-start ordering. AI owner accepted all three. Read-only source recheck confirmed the changes; AI owner owns behavioral/real CLI reruns and records them in `implementation-ai.md`. No additional required finding remained in this scope.

## ST-11 — large snapshot cause isolation

- Started: 2026-11-17T21:16:16+09:00.
- Context: integration owner observed a real design run remaining running after its CLI child disappeared. Independent read-only App review found automatic critic preparation can return before any design completion commit; App owner is adding stage-only observability. A separate unresolved hypothesis was custom SQLite VFS behavior when a document grows beyond the page cache.
- Command: `cargo test --test storage_large_payload -- --nocapture`.
- Acceptance: product Store commits a 3MiB JSON document then a 4MiB document with receipt/event consistency, reads and reopens the exact final state, within a 30-second local bound. No App source changes.
- Status: running.

- ST-11 result: **passed**, 1/1 actual SQLite product-path case, 0.24 seconds (ended 2026-11-17T21:17:15+09:00). Ordinary multi-megabyte snapshot size did not reproduce the integration symptom.
- Independent static cause found in AI cleanup: an error result consumed from a stdout/stderr/writer JoinHandle leaves its success-state flag unset, then cleanup polls the already-consumed JoinHandle again. Tokio 1.54.1 task/core.rs explicitly panics on that second output take. App execute can therefore unwind before its completion callback, matching running state with no remaining child. AI/root owners were notified immediately for their source fixes and product-path regressions; this review made no App/AI source changes. A separate App hazard is automatic critic preparation returning before the completed design state is committed.
