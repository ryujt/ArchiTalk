# Offline document renderer dependencies

These browser ESM files are checked into the repository and embedded in the Rust
binary. ArchiTalk does not load a CDN, npm, Node.js, or another source checkout at
runtime. Mermaid is loaded from the same origin only when a diagram needs it.
Each JavaScript file is a single bundle, with no split chunks or external imports.

| Package | Pinned version | License | Official source |
| --- | --- | --- | --- |
| Marked | 12.0.2 | MIT | https://github.com/markedjs/marked |
| DOMPurify | 3.4.18 | Apache-2.0 option | https://github.com/cure53/DOMPurify |
| Mermaid | 11.16.1 | MIT | https://github.com/mermaid-js/mermaid |

`manifest.json` records package tarball URLs, output SHA-256 hashes, and the exact
63 packages that contribute code to the bundles. `THIRD-PARTY-NOTICES.txt` contains
their distributed license/notice texts; adjacent `.LICENSE.txt` and `.LEGAL.txt`
files retain the top-level licenses and source legal comments. The Mermaid core
bundle resolves DOMPurify to the same 3.4.18 package as the Markdown sanitizer.

To regenerate from already prepared npm package directories (this is a maintainer
operation, never part of a Rust build or launch):

```sh
node packaging/vendor-document.mjs /path/to/node_modules /path/to/dompurify-3.4.18 /path/to/mermaid-11.16.1
```

The node_modules directory supplies Marked 12.0.2, esbuild 0.25.12, and Mermaid's
transitive packages. The optional package directory arguments permit using
unpacked official tarballs without installing packages or changing a lockfile.
The script verifies the four top-level versions and refuses external browser
imports. Review manifest dependency versions and notices whenever regenerating.
