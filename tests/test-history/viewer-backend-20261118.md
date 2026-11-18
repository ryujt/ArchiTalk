# Viewer files and transcription backend — 2026-11-18

## VB01 plan, before execution

Isolated temporary projects/configuration only. No user database, global key, microphone, clipboard or real API/model calls. Add HTTP contract tests before routes: bounded pruned text catalog, real filesystem Unicode paths, traversal/symlink/sensitive/binary rejection, reset guard, write-only separate private shared secret, omitted/null key semantics, and missing-key rejection before polling audio. Run `cargo test --test viewer_backend`; the baseline should fail because routes do not exist. Then implement pinned descriptor reads and a constant-endpoint bounded transcription client; cover upstream protocol with an injected local fake server only.

VB01 baseline: `cargo test --test viewer_backend -- --nocapture` failed 0 passed / 3 failed; all expected routes returned 404 (catalog expected 200, key PATCH expected 200, missing-key POST expected 400). No audio body was polled. The deliberately unread stream produced an unreachable-code warning; replace its panic with a polling counter before final clippy.

VB01 first implementation run: 2 passed / 1 failed. Shared-secret PATCH returned 400 because descriptor traversal correctly refused macOS's system `/var` symlink in tempfile paths. Root independently identified the same platform alias. Resolve the trusted config parent/nearest existing ancestor once in `Service::new`, retain absent suffixes, then perform secret and lock operations through NOFOLLOW canonical directory descriptors. Test corrections: use actual `workspace.db`/`-wal`, retain the isolated project TempDir, and observe audio polling with an atomic flag.

## VB02 plan, before execution

Add bounded catalog/depth/large-file and no-follow read regressions, secret symlink/hardlink/mode and invalid model tests, pending multipart reset/shutdown/single-slot tests, and private transport unit tests against a bound loopback fake server. Assert form contains only model/file, redirected requests are not followed, upstream error content never escapes, calls are not retried, invalid audio never reaches server, and oversized/invalid response fails closed. Actual OpenAI requests remain zero.

VB01/VB02 initial GREEN: HTTP6/6 passed (0.45s), local fake transport5/5 passed (0.06s). First focused clippy failed only `needless_question_mark` in viewer UTF-8 conversion; simplify that expression. Add canonical-equivalent Unicode read assertion before hardening read output to return real directory spelling rather than caller aliases.

VB02 additional sensitive-file regression: force the Markdown fixture filename to NFD and add a synthetic `auth.json`. First test edit failed to compile because a heterogeneous byte-array fixture lost its slice coercion (seven E0308 diagnostics); fix the first entry with `as_slice()` without changing assertions. The resulting run failed the expected catalog count (3 vs 2), exposing the known authentication filename. Add auth.json/auth.toml/auth.yaml/auth.yml and passwd/shadow exclusions to the viewer policy only. Account scan budget against actual content bytes after reading to retain the 32 MiB budget even if file size changes.

VB02 hardlink regression: add an isolated external sentinel file hardlinked under a harmless text filename. The same catalog count assertion fails 3 vs 2. Require `nlink == 1` for viewer reads/catalog membership; existing evidence behavior remains unchanged. Next run all viewer HTTP cases plus existing files/reset contracts, the five fake transport unit tests, then focused clippy and owned-file rustfmt check.

## Final focused verification and freeze

- `cargo test --test viewer_backend --test files_contract --test state_reset -- --nocapture`: **27 passed** (viewer6, existing file/export14, existing reset7); no ignored tests. Viewer run0.45s, files0.30s, reset1.08s.
- `cargo test --lib transcription::tests -- --nocapture`: **5 passed**, 18 unrelated lib tests filtered out. All transport requests target bound loopback fake servers; no paid request, real key, microphone or clipboard use.
- `cargo clippy --lib --test viewer_backend -- -D warnings`: **exit0**.
- `rustfmt --edition 2024 --check src/files.rs src/app.rs src/http.rs src/transcription.rs tests/viewer_backend.rs`: **exit0**.

Frozen source: `src/files.rs`, `src/app.rs`, `src/http.rs`, `src/transcription.rs`, `src/lib.rs`, main `Cargo.toml` reqwest/axum multipart dependencies and generated `Cargo.lock`; new `tests/viewer_backend.rs` and this history. Shared Settings/Workspace/receipts schema is unchanged. Key/settings writes use only the separate trusted config sibling; audio/transcript are transient memory. The HTTP handler permits one request, only an `audio` multipart field, cancels pending upload/outbound work on reset/shutdown, and returns only `{text,model}`. New static routes embed the four owner-provided browser modules.

Implementation bounds: 2000 viewer files, depth12, 20,000 visited entries, 32 MiB text scan and 2 MiB per read; existing directory descriptor enumeration also rejects individual directories above its existing10,000-entry ceiling. Such oversized directories fail closed with `file_unavailable`. File catalog access never adds AI evidence. The key's environment fallback is production shared-config mode only; deleting the file key can leave `configured:true,key_source:environment` when the caller has explicitly supplied `OPENAI_API_KEY`.
