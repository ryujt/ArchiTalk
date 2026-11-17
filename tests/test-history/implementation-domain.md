# Interview policy correction verification

Baseline: new implementation on main 1052929 plus parallel implementation files. Previous history: implementation-integration covers initial workflow; no correction/supersession or competing-priority regression existed. Scope owns src/domain.rs and tests/domain_contract.rs only; model owner added defaulted audit/source fields to keep stored snapshots deserializable. AI adapter remains frozen.

## DOM-01-20261117-counterexamples

Entry created before execution. Actual date/time and result follow. Establish pre-fix failures for: 30-day retention corrected to 7-day across requirements/claims/decisions, same-text changed acceptance/evidence, stale-source amendment rejection with atomic no-mutation, blocker fallback defeating model priority=1 scope, known conflict before risk, and reply/question one-question enforcement. Tests use the actual public domain API and synthetic user sources; no model call. Missing defaulted model-constructor fields, if encountered during owner handoff, are compilation blockers rather than behavior evidence.

DOM-01a: initial command exited 101 before executing tests because model-owner audit fields had landed before domain constructor defaults. Added only empty/None constructor values to compile the unchanged old behavior; this is not a behavioral fix. Run DOM-01b preserves the actual counterexample baseline next.

## DOM-01b-20261117-behavior-baseline

Entry created before execution. Run `cargo test --test domain_contract -- --nocapture` after constructor-only compatibility adjustment. Expect failures proving each new acceptance criterion is not yet implemented.

DOM-01b result: exit 101; all seven public-API counterexample tests failed under pre-fix behavior. Failures directly observed old status remaining proposed, same-text updates discarded, stale/unknown source amendments accepted, model scope priority beating risk fallback, known conflict not first, and multi-question reply accepted. No test passed merely from compilation. This is the failing behavior baseline for DOM-02.

## DOM-02-20261117-policy-fix

Entry created before implementation verification. Implement immutable audit-linked amendment versions, exact-source validation (latest user required for amendment), active/history prompt separation, server-derived conflict/risk/core priority with fallbacks considered together, and single-question enforcement. All state changes apply to a clone and commit only after validation succeeds. Run public regression suite and existing workflow/controller tests; actual full-product model path remains root-owned.

DOM-02 initial result: original seven counterexamples pass 7/7. Added explicit collection/8KiB scalar/4-option/256KiB aggregate budgets and sourced-requirement/decision rejection checks. Independent UI Critic then found two required counterexamples: anonymous historical value resurrection and slicing an unknown answer to remove its unknown marker. Both accepted: all three record types reject anonymous resurrection of a superseded value/title, and amendment source checks the entire latest user answer conservatively for unknown/delegation/deferral before matching the quote. Explicitly reverting via the current active stable id plus a new direct user quote remains supported. Readiness additionally requires at least one active requirement and rechecks exact quote/ref/acceptance; superseded records stay history only.

## DOM-03-20261117-final-regressions

Entry created before execution. Run final domain contract suite and existing workflow/controller tests. Added regressions for sliced unknown quotes, historical requirement/claim/decision resurrection with valid explicit revert, bounds, and readiness with no/invalid active requirements. No real model call here; root owns final real product workflow after AI consumed-handle bug fix.

DOM-03 result: exit 0, 13 domain contract cases passed and 3 existing workflow/controller/HTTP integration cases passed (0.78 s). Added active-vs-history export trace separation so the latest 7-day rule is shown only in current proposals, with obsolete 30-day requirement solely in explicit historical audit; a trace assertion verifies this boundary. Final metadata now records prompt_version architalk-v2 and candidate schema_version 2 to distinguish these validation rules from earlier attempts. Final trace regression rerun follows; no other product module changed by this domain task.

DOM-03 trace rerun result: root `cargo test --test domain_contract -- --nocapture` exit 0, 13/13 passed including current-vs-history trace assertion. Earlier intermediate DOM-02 bound/source suite also passed 9 domain + 3 workflow cases; retained separately here rather than replacing the original 7-failure baseline. Model snapshot metadata change to architalk-v2/schema2 changes labeling only. Domain files frozen for final root product validation. All domain runs occurred on 2026-11-17 KST; exact per-run start times were not captured in the first local tool commands, so no fabricated start/end times are supplied. Initial local clock observed 21:12:21 KST; final static verification below records an actual clock for handoff. This timestamp precision gap does not replace root's final timestamped test evidence.

## DOM-04-20261117-handoff-static

Entry created before execution. Final Rust formatting checks only; dynamic outcomes are DOM-03. Root/UI independent final review requested after both required follow-up findings were fixed. Raw user messages and superseded fields remain immutable aside from the explicit status transition; new model interpretations remain proposed. Exact quote/ref checks are structural evidence validation, not a semantic proof that a paraphrase means the same thing. Mixed latest answers containing unknown/deferred/delegated wording conservatively cannot authorize amendments and may require a separate clear correction.

DOM-04 result: 2026-11-17 21:21:16–21:21:16 KST, `rustfmt --edition 2024 --check src/ai.rs src/domain.rs tests/ai_adapter.rs tests/domain_contract.rs` exited 0. No formatter mutation. Source remains frozen; root owns final full build/workflow and history index updates.

Independent UI Critic final closure: read-only review confirmed anonymous historical resurrection is blocked across Claim/Requirement/Decision, latest full user answer prevents sliced-quote unknown bypass, and the 13-case suite includes both counterexamples plus explicit reverting to 30 days through a current stable target. No additional required finding remains within reviewed D1/D2 scope. Critic did not duplicate the already passing dynamic suite.
