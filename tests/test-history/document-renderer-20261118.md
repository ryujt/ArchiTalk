# Document renderer implementation checks — 2026-11-18

Scope: offline Markdown/SmartMD and diagram renderer; actual application/browser
regression coverage is recorded by the separate browser testing task.

## Passed

- `node --check web/document-renderer.js` and all seven `web/diagrams/*.js` modules.
- Marked 12.0.2 imports `Marked`/`Renderer` as browser ESM and emits actual code HTML.
- SmartMD keeps ordinary JavaScript fences containing `{{name}}`, `:::warning`,
  and `{table ...}` byte-for-byte unchanged before Markdown parsing.
- SmartMD keeps nested `text` fences containing a Mermaid example unchanged.
- SmartMD restores a fenced JavaScript block inside an info callout before parsing;
  the callout contains an actual `pre > code.language-js`, without an internal mask.
- Cyclic layout (`A V B`, `B V A`) throws a local, explanatory cycle error.
- A layout label containing `<img src="/api">` becomes escaped text, never an image.
- Jobflow generates an actual SVG for `Object: User, API` and a method relation.
- `packaging/vendor-document.mjs` rebuilt the local browser bundles with pinned
  Marked 12.0.2, DOMPurify 3.4.18, Mermaid 11.16.1, and esbuild 0.25.12. It asserts
  that no browser output imports an external module. The resulting Mermaid bundle
  includes DOMPurify 3.4.18 (no older embedded sanitizer).

An independent read-only Critic review confirmed the renderer boundaries and found
no remaining critical/major blocker. All three asset SHA-256 hashes matched the
manifest after the final rebuild.

No model call, npm install, external-source modification, or commit was performed.
The DOMPurify/Mermaid updates came from downloaded official npm tarballs unpacked
under `/tmp`; the Rust build/runtime uses only the checked-in assets.

## Security behavior implemented for browser validation

Raw document HTML passes through a restricted DOMPurify allowlist. Form controls,
SVG, style elements/attributes, images, resource URLs, IDs/names, arbitrary classes,
and all `data-*` attributes are removed; Markdown images are inert text. Explicit
HTTP(S)/mailto links open with `noopener noreferrer nofollow`; other link protocols
and relative URLs are inert. GFM task checkboxes are inert Unicode markers.

Generated SVG is sanitized separately, with isolated IDs/local references and no
links, foreignObject, image, scripting, animation, external URL, or binding calls.
Raw Mermaid configuration, click/link, custom style, image/icon, CSS URL/import,
backslash escapes, and external-resource URI syntax is rejected before rendering.
Mermaid uses strict security and text labels; operations run in a serial queue.

Document/diagram errors retain escaped original source locally. Generation and
attachment checks prevent stale asynchronous render results from mutating a new
or detached preview. Resource limits and layout cycle/depth limits bound rendering.

## Browser-driven corrections

The browser tester found a visible label defect and CSP diagnostics that required
further investigation after initial integration:

- Navigation punctuation entities appeared literally in SVG text under Mermaid's
  `htmlLabels:false` mode. The adapter now decodes a small, explicit punctuation
  entity whitelist on SVG `Text.nodeValue` only, after SVG sanitization. It does
  not reparse text as HTML or change the generated graph's quoted-label grammar.
- Hostile images/SVG left no unsafe final DOM and the execution probe remained
  false, but produced three CSP `script-src-attr` diagnostics per viewport.
  Fragment import/clone was the initial hypothesis. The attempted pre-strip on
  an inert template did not remove these diagnostics; browser D03 located their
  source at the initial `template.innerHTML` line itself. The pre-strip attempt
  was superseded by the canonical string-entry correction below.

The browser task separately records the failing runs and subsequent verification;
these issues are not counted as passed by the earlier focused Node checks.


### D03-driven correction

The first raw HTML parse now uses `DOMPurify.sanitize(htmlString, htmlPolicy)`;
there is no host-document raw template/innerHTML parse. SmartMD table comments
are captured into an independent Map and replaced with unpredictable text tokens
before sanitization. Only matching tokens on the clean fragment are restored as
inert comments, then the validated table metadata function runs there. Unsafe
HTML, resources, handlers, IDs/names/data attributes and styles reach no host
DOM parse. Final browser CSP interpretation and independent checks are recorded
below and in the browser task.

Generated diagrams now retain their natural readable width (minimum 280px,
maximum readability floor 1600px) inside the figure's own horizontal scroll
area, so narrow viewports do not compress typical labels into 5–7px text. This
does not increase the document/viewport width.


The unchanged three diagnostics later moved to the pinned DOMPurify's native
`DOMParser.parseFromString` line. The browser task found an official Chromium
regression involving CSP reports from document-without-Window inert parsers; it
independently checked that explanation against native DOMParser behavior,
actual execution probes, final DOM, and resource requests (B06 below). A CSP console report
alone is not evidence that a handler executed. The canonical string sanitizer
and the production CSP remained unchanged throughout those checks. See the separate
[actual browser record](document-rendering-browser-20261118.md) for the complete
failing runs, primary-source reference, diagnostics, and final browser result.

### B06 browser result

The separate browser tester completed all four desktop/mobile target cases:
**4 passed, exit 0, 10.8 seconds**. In each viewport, a native DOMParser document
with `defaultView === null`, never attached to the page, reproduced one CSP
warning while its execution probe remained false. This independently confirms
the known Chromium inert-parser reporting regression; the three exact diagnostics
from the pinned DOMPurify parse line are classified on that evidence.

The same hostile product rendering was also reloaded on an isolated test page
without that page's CSP header: final DOM remained clean, execution probe false,
authentication leak false, resource attempts zero, and CSP messages zero. The
production CSP was not changed. The browser test continues to fail on any other
CSP diagnostic, active DOM, execution, or unexpected request.

The tester opened the mobile PNG and verified readable natural-size SVG with
horizontal movement confined to each figure, with no page overflow. Correct
Navigation punctuation, SmartMD table specifications, nested code/diagram
callouts, local errors, snapshot preservation, and reload rendering also passed.
The broader browser regression suite is recorded by the browser task separately.
