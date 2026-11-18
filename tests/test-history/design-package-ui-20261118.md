# Design package UI and local document links — 2026-11-18

## Scope and contract

This change displays the immutable ten-document Method-R design package supplied
by the backend. `Draft.markdown` remains the principal architecture document;
`Draft.documents` supplies companion `{kind, markdown}` records. Only the server's
`workspace.document_types` catalog supplies document kinds, filenames, labels, and
ordering. Model-provided filenames/paths/labels never become UI or link targets.
Legacy drafts without companions continue to expose one principal document.

The review toolbar retains the draft-version selector and adds
`#document-kind-picker` (`aria-label="문서 종류 선택"`). The auxiliary document tab
contains a selectable item for each available kind. The `.markdown` preview and
its collapsed original-source disclosure show the same selected document. Review,
approval, export preview, and save still bind the entire immutable draft/package;
buttons and explanatory text explicitly state the complete document count.

Preview reuse keys include session ID, draft ID, document kind, exact Markdown,
and available catalog links. Switching sessions/new sessions resets to the main
document. A selected kind survives snapshots and generation while it exists in
the selected draft; selecting a legacy/missing-kind draft falls back to main.

Renderer options are additive:

```js
renderDocument(container, markdown, {
  documentLinks: [{kind, filename}], // currently available server catalog entries
  currentDocument: kind,
  onDocumentLink: ({kind, anchor}) => selectDocument(kind, anchor),
  anchor: null, // optional heading slug; empty string means document start
});
```

Sanitized headings receive trusted, scoped IDs based on NFC/lowercase Unicode
letter/number/underscore/hyphen slugs, whitespace-to-hyphen conversion, and numeric
duplicate suffixes. Raw HTML IDs/names/data attributes remain forbidden. Internal
link destinations live in a WeakMap, never in source-controlled app attributes.
Known package basenames (optionally `./`) and Unicode/percent-encoded fragments
switch the local document or scroll its heading. No package link performs a file
GET, API request, or location navigation. Unknown files, traversal, schemes,
queries, malformed encodings, and encoded controls stay inert. HTTP(S)/mailto
links retain the existing explicit external-link policy.

Same-document links scroll/focus a trusted heading. Other-document links select
the kind and move to the anchor after rendering settles. Diagram SVG IDs and
existing sanitation/CSP/stale-result boundaries remain separate and unchanged.

## Focused verification

- `node --check web/app.js` and `node --check web/document-renderer.js`: passed.
- `node --test tests/web.test.mjs`: **21 passed**, existing API/SSE/review/export
  and recovery behavior preserved.
- `node --test tests/design-package-ui.test.mjs`: **6 passed**. Covers catalog
  ownership/order, immutable source preservation, legacy fallback, invalid or
  model-only kinds/paths, Unicode/NFD/encoded local references, forbidden link
  targets, and heading normalization (including supplementary Unicode letters).
- Full application/browser/package verification is owned by the integration and
  browser testing tasks. No browser result is claimed by these Node tests.

No paid model request, dependency installation, source-repository modification,
or commit was performed by this task.

## Adjustable AI request time limit

The settings form now exposes `작업 시간 제한(초)` as the existing
`ai.timeout_seconds` setting. The numeric input is required, uses integer steps,
and has minimum 10 / maximum 600. Help text explains that this applies to each
AI request and that edits take effect on the next task. The field displays the
server's current value; this UI does not replace existing user configuration.

`settingsFromValues` parses the field with `Number` only when it is present.
Older forms that omit it preserve the existing value. Non-integers, empty values,
non-finite/non-numeric values, and values outside 10–600 raise an explicit Korean
`INVALID_SETTINGS` error. Parsing runs inside the existing command error boundary
so an invalid value remains in the settings dialog with a clear error and is not
sent as a PATCH. Cloning preserves the current settings and active-run snapshot.

Verification: syntax check passed; combined
`node --test tests/web.test.mjs tests/design-package-ui.test.mjs` completed
**28 passed / 0 failed**. The added contract checks both inclusive limits, 300,
omitted-field preservation, trusted executable/profile preservation, invalid
values, and source immutability. The browser tester owns two targeted desktop/
mobile settings cases after the final server build; the full browser suite need
not be repeated for this field alone.
