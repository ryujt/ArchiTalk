# Conversation field limits and question failure — 2026-11-18

## Plan before implementation

Baseline: clean `7273e60`. The reported first conversation reached a successful
Claude CLI terminal result and then failed domain validation with `question
exceeds field limit`. Read-only project metadata reported one session, one user
message, no stored question and a failed run. The raw response was intentionally
not retained, so the exact offending field is unknown. Five options is a possible
counterexample, not an established description of the user's actual response.

The current validator uses UTF-8 byte lengths, rejects NUL, and permits question
fields up to target_ref 200 bytes / text 2000 / reason 8192 / four options / each
option 1000. The same generic error covers every one of those failures. The
conversation prompt supplies none of these numerical limits. A Korean syllable
normally occupies three UTF-8 bytes, making a character-count assumption unsafe.

Authorized scope: `src/domain.rs`, `tests/domain_contract.rs`, this record. First
add executable counterexamples for missing prompt limits and ambiguous errors,
run them against the unchanged validator, then implement shared constants,
model-facing limit metadata and precise value-free field/index/observed/limit
errors. Keep every existing cap, atomic application, source evidence policy and
server-selected single displayed question. Recommend one candidate while keeping
the existing hard maximum of twenty candidate questions for priority selection.
Do not truncate input, relax limits, add a model attempt, mutate the user's DB,
stop the user's server, or invoke a real model.

Static orchestration review: `src/app.rs` already includes the validation error in
its existing one repair prompt for conversation/critic. Design has no automatic
repair. This task changes no retry orchestration or quota policy.

## Red baseline

The focused command and its results will be recorded before source changes.

- The first preparation compile caught two nonliteral assertion-message arguments
  in the new test code. They were corrected; no product source had changed.
- Actual red baseline: `cargo test --test domain_contract
  conversation_limit_contract -- --nocapture`, exit **101**, **1 passed / 2
  failed**. The existing 1000-byte four-option boundary was accepted and preserved.
  The prompt test failed because no numeric-contract marker existed. All four
  distinct question counterexamples (five options, 1002-byte option, 2001-byte
  question, NUL reason) returned the same generic error, failing the precise-field
  assertion. These failures precede the implementation below.

## Implementation and focused green result

Every existing bound now has one shared constant used by validation and a
`CONVERSATION_LIMITS_JSON` block in the conversation prompt. The block names all
known string fields, list caps, optional/nonempty amendment IDs, reference-string
bounds, unsigned priority representation, and the exact set of text fields counted
in the unchanged 262144-byte aggregate budget. It explicitly says UTF-8 bytes,
not characters, and forbids NUL. The normal recommendation is one candidate, with
the old twenty-candidate hard cap and server-prioritized single display retained.

Validation errors expose only a structural field/index, observed count and limit:
for example `questions[0].options: observed 5 items; limit 4 items`. They do not
print user/model text or evidence values. Optional IDs and reference entries also
identify an empty or excessive field. Aggregate rejection identifies its budget.
All validation still completes on the cloned session before anything commits.

The redundant post-validation `take(4)` was removed: validated options are moved
unchanged, preserving all four options and their exact text/order. Redundant,
looser size checks after the strict validator were removed without changing the
accepted set. Source-quote matching, amendment rules, facts/assumptions status,
priority selection, and retry behavior were not altered.

- Focused green command: `cargo test --test domain_contract
  conversation_limit_contract -- --nocapture`: **3 passed / 0 failed**.
- Broader owned-contract command: `cargo test --test domain_contract --test
  design_package -- --nocapture`: **37 passed / 0 failed** (17 domain contracts,
  20 package contracts). Additional cases cover unchanged collection caps, IDs,
  reference lists/strings, kind/acceptance/alternative/quote fields and aggregate
  text budget, always checking rejection leaves the entire session unchanged.
- The separate workflow tester owns real subprocess/App validation-to-repair,
  UTF-8 response correction and the two-invalid-responses stop condition. Those
  results are not claimed by the pure domain tests above.

Independent read-only review found no blocker and confirmed that accepted caps,
UTF-8/NUL rules, candidate/display policy, direct option preservation, and
value-free diagnostics remain consistent. The reviewer reported a passing
`clippy --lib` plus its retry/workflow/reset/shared target checks. This task did
not duplicate that lint run or claim a separate all-target lint result; root owns
the final integrated all-target check and any explicit real-model validation.
