// Rebuild the checked-in browser assets from already unpacked, pinned npm packages.
// No npm install, remote import, source repository path, or Node runtime is used by ArchiTalk.
import { createRequire } from 'node:module';
import { readFile, writeFile, mkdir, readdir } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { createHash } from 'node:crypto';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const modules = path.resolve(process.argv[2] || 'node_modules');
const dompurify = path.resolve(process.argv[3] || path.join(modules, 'dompurify'));
const mermaid = path.resolve(process.argv[4] || path.join(modules, 'mermaid'));
const require = createRequire(path.join(modules, '_architalk_vendor.cjs'));
const esbuild = require('esbuild');
if (esbuild.version !== '0.25.12') throw new Error('Expected esbuild 0.25.12');
const out = path.join(root, 'web/vendor');
await mkdir(out, { recursive: true });
const specs = [
  { name: 'marked', version: '12.0.2', dir: path.join(modules, 'marked'), entry: 'lib/marked.esm.js', file: 'marked.js', source: 'https://registry.npmjs.org/marked/-/marked-12.0.2.tgz' },
  { name: 'dompurify', version: '3.4.18', dir: dompurify, entry: 'dist/purify.es.mjs', file: 'dompurify.js', source: 'https://registry.npmjs.org/dompurify/-/dompurify-3.4.18.tgz' },
  { name: 'mermaid', version: '11.16.1', dir: mermaid, entry: 'dist/mermaid.core.mjs', file: 'mermaid.js', source: 'https://registry.npmjs.org/mermaid/-/mermaid-11.16.1.tgz' },
];
const allPackages = new Map();
const manifest = { buildTool: 'esbuild@0.25.12', format: 'browser ESM, bundled without split chunks', packages: [] };
for (const spec of specs) {
  const pkg = JSON.parse(await readFile(path.join(spec.dir, 'package.json'), 'utf8'));
  if (pkg.version !== spec.version) throw new Error(`Expected ${spec.name}@${spec.version}; found ${pkg.version}`);
  const outfile = path.join(out, spec.file);
  const result = await esbuild.build({ nodePaths: [modules], alias: { dompurify: path.join(dompurify, 'dist/purify.es.mjs') }, entryPoints: [path.join(spec.dir, spec.entry)], outfile, bundle: true, format: 'esm', platform: 'browser', target: 'es2022', minify: true, legalComments: 'linked', metafile: true, logLevel: 'warning' });
  for (const output of Object.values(result.metafile.outputs)) {
    if (output.imports.some((entry) => entry.external)) throw new Error('Unexpected external browser import');
  }
  for (const input of Object.keys(result.metafile.inputs)) {
    let directory = path.dirname(path.resolve(input));
    while (directory !== path.dirname(directory)) {
      try {
        const packageData = JSON.parse(await readFile(path.join(directory, 'package.json'), 'utf8'));
        if (packageData.name && packageData.version) allPackages.set(`${packageData.name}@${packageData.version}`, { directory, packageData });
        break;
      } catch { directory = path.dirname(directory); }
    }
  }
  const data = await readFile(outfile);
  manifest.packages.push({ name: spec.name, version: spec.version, license: pkg.license, source: spec.source, file: spec.file, sha256: createHash('sha256').update(data).digest('hex') });
  const licenseName = spec.name === 'marked' ? 'LICENSE.md' : 'LICENSE';
  await writeFile(path.join(out, `${spec.name}.LICENSE.txt`), await readFile(path.join(spec.dir, licenseName)));
}
const notices = ['ArchiTalk document renderer: bundled third-party dependency notices.\nThe following packages contribute code to the browser assets.\n'];
manifest.bundledDependencies = [];
for (const [key, { directory, packageData }] of [...allPackages].sort(([a], [b]) => a.localeCompare(b))) {
  const noticeFiles = (await readdir(directory)).filter((file) => /^(?:licen[cs]e|copying|notice)(?:[.-]|$)/i.test(file));
  notices.push(`\n===== ${key} (${packageData.license || 'see notices'}) =====\n`);
  for (const file of noticeFiles) {
    try { notices.push(`\n${file}\n${await readFile(path.join(directory, file), 'utf8')}`); } catch { /* directories are not notices */ }
  }
  manifest.bundledDependencies.push({ name: packageData.name, version: packageData.version, license: packageData.license || null });
}
await writeFile(path.join(out, 'THIRD-PARTY-NOTICES.txt'), notices.join('\n'));
await writeFile(path.join(out, 'manifest.json'), `${JSON.stringify(manifest, null, 2)}\n`);
