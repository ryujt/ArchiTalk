# Sequential document generation domain contract — 2026-11-18

## Plan before implementation

Generate the same ten-document architecture package in ten ordered, independent
model requests: drivers, quality_scenarios, alternatives, decisions, delivery,
jobflow, state, screen_layout, navigation, architecture. Each validated document
is immutable input to later requests. Keep the original nine-kind companion
enum, complete package validator, Method-R/SmartMD/dedicated DSL rules, byte caps,
and full-package critic contract. No analysis call, automatic repair, model or
effort change, or timeout reduction is added. The default 1800-second and maximum
3600-second per-request settings remain unchanged.

Owned source: design_package/domain, a new design_generation module and lib
declaration, common package rules plus single-document task rules, the new
design_generation_contract tests, and this record. App/checkpoint/model/storage/UI
ownership is separate. Tests precede source changes: the new APIs are absent, so
their initial compile failure is the expected first red signal. Then reuse the
original document analysis, add prefix constraints and strict single-response
validation, and retain original whole-package validation before accepting the
last architecture document.

Counterexamples cover skipped/duplicate/wrong kinds, unknown fields or paths,
missing profile/sections/trace ownership/required edges, forward trace obligations
not defined on the owner's turn, backward undefined traces, forbidden future
anchors and invalid current/completed anchors, total package size, and locally
valid architecture that violates final module collaboration. Prompt tests cover
complete source/prior-document data, actual ID/anchor/pending manifests, source
hashes excluding transport/draft/checkpoint state, contract version/order/rules
hashing, and the existing combined 900000-byte context bound without truncation.

All data is synthetic. No real model, user's DB, live server, shared settings,
installation, or commit is touched.

## Red baseline

- `cargo test --test design_generation_contract -- --nocapture`: exit **101**
  before production implementation. The concurrently added model checkpoint field
  referenced the not-yet-existing `design_generation` module, and new_session
  lacked its new `design_generations` initializer. This is an API-absence compile
  red signal, not a claimed behavioral regression result. The new nine test cases
  were already on disk and the package/domain implementation was unchanged.

The first implementation passed nine new contracts and all 37 existing package
and domain contracts. Before freezing, an additional restoration counterexample
found a real prefix-validation hole: when both Drivers and Quality checkpoints
were present, Drivers could retroactively acquire a forward anchor into Quality.
`cargo test --test design_generation_contract restored_prefix_cannot --
--nocapture` exited **101**, **0 passed / 1 failed**. The fix compares the target's
generation ordinal to the referring document's own ordinal, preserving the
original stage boundary even when revalidating a longer restored prefix.

## Implementation and focused green result

`GenerationKind` is a separate, strict snake_case ten-target enum; the old
`DocumentKind::ALL` remains the original nine companion kinds. Each response is
strict `{kind,markdown}` for the next target, normalizes CRLF as before, and must
pass the original per-document analysis before acceptance. That analysis was
extracted from the complete validator and reused without weakening frontmatter,
SmartMD, section, Method-R, dedicated DSL, trace ownership/definition/edge or byte
rules. The complete validator still enforces the original package relationships.

Prefix validation checks fixed order, total bytes, obligations for every already
generated trace owner, actual current/completed anchors and server-owned names.
Future trace references remain explicit pending obligations until the owner's
turn. Future file links use bare server filenames only; their anchor cannot be
retroactively legitimized by a later restored document. Navigation must have its
real Layout anchors before it becomes a checkpoint. The final architecture
response runs the original whole-package validator before returning a document;
`assemble` also runs it before producing the ordinary DesignPackage. Thus a failed
whole-package check cannot leave a newly accepted architecture checkpoint.

The single prompt contains the unchanged prior document text and an extracted,
deterministically ordered manifest of actual trace definitions, heading anchors
and pending ID owners/referencing files. Source context plus this generation data
is one JSON object under the original 900000-byte bound. Nothing is summarized,
truncated or silently dropped. TARGET_JSON is the sole trusted target marker.
The common rules now defer only their response envelope to the task, keeping
Method-R, evidence/unknown/proposal status, SmartMD and dedicated DSL requirements.

`design_context_hash` covers the actual prompt-source context, normalized response
language and design_revision. It excludes session transport revision/status,
active run, drafts and checkpoint/provenance state; changing stored checkpoint
model/effort/timeout metadata does not change it. `contract_hash` covers the
generation version, fixed order, complete/single prompt rules and document/package
byte caps. Settings are deliberately outside both generation identity inputs so
an explicit resume can use newly selected settings without rewriting prior parts.

- Final focused command: `cargo test --test design_generation_contract --test
  design_package --test domain_contract -- --nocapture`: **47 passed / 0 failed**
  (10 staged contracts, 20 original package contracts, 17 domain contracts).
- `rustfmt --edition 2024 --check src/design_package.rs src/design_generation.rs
  src/domain.rs src/lib.rs tests/design_generation_contract.rs`: exit **0**.
- `cargo clippy --lib --test design_generation_contract --test design_package
  --test domain_contract -- -D warnings`: exit **0**.

The stage fixture helper changes only two future-anchor links from the original
synthetic package to bare server filenames (Jobflow→principal and Layout→future
Navigation). All document bodies, analysis and DSL quality remain, and the
assembled package passes the original validator. The original fixture file and
legacy whole-package tests remain unchanged.

Independent read-only review found no concrete blocker and confirmed pending
owner resolution, ordinal anchor rules, strict response kind/schema, final full
validation and the 900000-byte no-truncation boundary. Its suggested clarification
that future trace references are temporary obligations, not undefined references
in an accepted final package, was incorporated in the common prompt. App/model/
checkpoint/storage/UI/process orchestration tests belong to the other agents and
are not claimed here. Real model calls, user DB/server access, shared-setting
changes, installations and commits: **zero**.
