# tools.camp diagram and SmartMD adaptations

These seven modules were adapted from the user-specified tools.camp mdview source:

`cli/mdview/internal/mdview/static/js/{smartmd,block-parser,cjk-emphasis,jobflow,layout,state,navigation}.js`

Source checkout commit: `46dffca650760db12d6fd89529228f09e779e331`.
The source history identifies Ryu (류종택) as its author. No repository-wide LICENSE
or module license header was present in that checkout; this notice records
provenance without assigning an upstream license that was not supplied. The user
explicitly requested integrating this existing renderer into ArchiTalk. ArchiTalk's
new adapter and modifications are distributed under the repository's MIT license.

Adaptations:

- Local Marked import and parser callback replace mdview's CDN/global dependency.
- SmartMD prose transforms preserve every ordinary fenced code region, plus
  mdview's nested `text` fence semantics. Fences inside callouts are restored before
  parsing the callout's content.
- Table metadata travels through string sanitization as unpredictable inert
  tokens, then is validated and applied only on the clean fragment; only bounded
  numeric widths, known alignment values, and plain titles survive.
- Layout labels are HTML-escaped, generated IDs contain safe characters, sizes are
  numeric and bounded, and cycles/deep or oversized trees produce a local error.
- ArchiTalk's `document-renderer.js` independently sanitizes all HTML/SVG, removes
  active resources and host-app attributes, and serializes Mermaid operations.
  The mdview application, CDN initialization, copy handlers, zoom handlers, and
  mutable global Markdown renderer are not included.
