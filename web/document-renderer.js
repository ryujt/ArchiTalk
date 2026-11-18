import { Marked, Renderer } from './vendor/marked.js';
import DOMPurify from './vendor/dompurify.js';
import JobFlowToSVG from './diagrams/jobflow.js';
import LayoutToHTML from './diagrams/layout.js';
import { stateToMermaid } from './diagrams/state.js';
import { navigationToMermaid } from './diagrams/navigation.js';
import { findTextFenceClose } from './diagrams/block-parser.js';
import { fixCjkEmphasis } from './diagrams/cjk-emphasis.js';
import { preprocessSmartMd, applySmartTableSpecs } from './diagrams/smartmd.js';

const generations = new WeakMap();
const documentAnchors = new WeakMap();
const documentLinkHandlers = new WeakMap();
const diagramLanguages = new Set(['jobflow', 'layout', 'state', 'navigation', 'mermaid']);
let mermaidModule;
let mermaidQueue = Promise.resolve();
let sequence = 0;
const escapeHtml = (value) => String(value).replace(/[&<>"']/g, (char) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[char]));
const htmlPolicy = {
  ALLOWED_TAGS: ['a', 'p', 'br', 'hr', 'h1', 'h2', 'h3', 'h4', 'h5', 'h6', 'strong', 'em', 'del', 's', 'blockquote', 'pre', 'code', 'ul', 'ol', 'li', 'table', 'thead', 'tbody', 'tfoot', 'tr', 'th', 'td', 'caption', 'div', 'span', 'details', 'summary', 'kbd', 'sup', 'sub', 'dl', 'dt', 'dd'],
  ALLOWED_ATTR: ['class', 'href', 'title', 'colspan', 'rowspan', 'align', 'start', 'reversed', 'open', 'type'],
  ALLOW_DATA_ATTR: false,
  ALLOW_ARIA_ATTR: false,
  FORBID_ATTR: ['id', 'name', 'style'],
  RETURN_DOM_FRAGMENT: true,
};
const mermaidConfig = {
  startOnLoad: false,
  securityLevel: 'strict',
  suppressErrorRendering: true,
  htmlLabels: false,
  flowchart: { htmlLabels: false, useMaxWidth: true },
  state: { htmlLabels: false },
  maxTextSize: 50000,
  maxEdges: 500,
  theme: 'neutral',
  fontFamily: 'system-ui, -apple-system, sans-serif',
  deterministicIds: true,
  secure: ['securityLevel', 'startOnLoad', 'maxTextSize', 'maxEdges', 'suppressErrorRendering', 'htmlLabels', 'flowchart', 'state', 'themeCSS', 'themeVariables', 'dompurifyConfig', 'fontFamily', 'secure'],
};

export function headingSlug(title) {
  return [...String(title).normalize('NFC').toLowerCase()].map((char) => /[\p{L}\p{N}_-]/u.test(char) ? char : /\s/u.test(char) ? '-' : '').join('');
}

function decodePart(value) {
  try { return decodeURIComponent(value).normalize('NFC'); } catch { return null; }
}

/** Resolve package references without constructing a URL or requesting a file. */
export function resolveDocumentTarget(href, documentLinks = [], currentDocument = 'architecture') {
  const value = String(href || '').trim();
  if (!value || /[\u0000-\u001f\u007f\\]/.test(value) || value.startsWith('//')) return null;
  const hash = value.indexOf('#');
  const file = decodePart(hash < 0 ? value : value.slice(0, hash));
  const anchor = hash < 0 ? null : decodePart(value.slice(hash + 1));
  if (file === null || (hash >= 0 && anchor === null) || /[\u0000-\u001f\u007f\\]/.test(file) || (anchor !== null && /[\u0000-\u001f\u007f]/.test(anchor))) return null;
  if (file === '' && hash >= 0) return { kind: currentDocument, anchor: anchor.toLowerCase() };
  const basename = file.startsWith('./') ? file.slice(2) : file;
  if (!/^[^/\\?#\u0000-\u001f]+\.md$/iu.test(basename)) return null;
  const match = (Array.isArray(documentLinks) ? documentLinks : []).find((entry) => entry && typeof entry.kind === 'string' && /^[a-z][a-z0-9_]*$/.test(entry.kind) && typeof entry.filename === 'string' && entry.filename.normalize('NFC') === basename);
  return match ? { kind: match.kind, anchor: anchor === null ? null : anchor.toLowerCase() } : null;
}

function assignHeadingIds(root, prefix) {
  const anchors = new Map();
  for (const heading of root.querySelectorAll('h1,h2,h3,h4,h5,h6')) {
    const base = headingSlug(heading.textContent);
    let slug = base, duplicate = 0;
    while (anchors.has(slug)) slug = `${base}-${++duplicate}`;
    heading.id = `${prefix}-heading-${slug || 'section'}`;
    anchors.set(slug, heading);
  }
  return anchors;
}

export function scrollToDocumentAnchor(container, anchor = '') {
  if (!container.isConnected) return false;
  const value = String(anchor).replace(/^#/, '').normalize('NFC');
  const target = value === '' ? container : documentAnchors.get(container)?.get(value.toLowerCase());
  if (!target) return false;
  if (target !== container) { target.tabIndex = -1; target.focus?.({ preventScroll: true }); }
  target.scrollIntoView?.({ block: 'start', inline: 'nearest', behavior: 'instant' });
  return true;
}

function safeLinks(root, options, anchors) {
  const localLinks = new WeakMap();
  const kind = options.currentDocument || 'architecture';
  for (const link of root.querySelectorAll('a')) {
    const href = (link.getAttribute('href') || '').trim();
    if (/^(?:https?:\/\/|mailto:)/i.test(href) && !/[\u0000-\u0020\u007f]/.test(href)) {
      link.setAttribute('target', '_blank');
      link.setAttribute('rel', 'noopener noreferrer nofollow');
      continue;
    }
    const target = resolveDocumentTarget(href, options.documentLinks, kind);
    if (!target || (target.kind !== kind && typeof options.onDocumentLink !== 'function')) { link.removeAttribute('href'); continue; }
    const heading = target.kind === kind && target.anchor !== null ? anchors.get(target.anchor) : null;
    link.setAttribute('href', heading ? `#${heading.id}` : '#');
    link.classList.add('document-package-link');
    localLinks.set(link, target);
  }
  return localLinks;
}

function installDocumentLinks(container, localLinks, options, current) {
  const previous = documentLinkHandlers.get(container);
  if (previous) { container.removeEventListener('click', previous); container.removeEventListener('auxclick', previous); }
  const listener = (event) => {
    const link = event.target.closest?.('a');
    const target = link && localLinks.get(link);
    if (!target) return;
    event.preventDefault();
    if (!current() || event.type === 'auxclick') return;
    if (target.kind === (options.currentDocument || 'architecture')) scrollToDocumentAnchor(container, target.anchor || '');
    else options.onDocumentLink?.({ ...target });
  };
  container.addEventListener('click', listener);
  container.addEventListener('auxclick', listener);
  documentLinkHandlers.set(container, listener);
}

function sanitizeMarkdown(html) {
  // The first DOM parse belongs to DOMPurify's isolated string entry point.
  // Even an inert template in the host document can compile inline handlers
  // during parsing and trigger CSP violations. Never parse raw HTML there.
  // Preserve table metadata as unpredictable text tokens through sanitization;
  // only those tokens are restored as inert comments on the clean fragment.
  const tableMarkers = new Map();
  const tableToken = `ARCHITALK-TABLE-${crypto.randomUUID()}`;
  const protectedHtml = html.replace(/<!--\s*(SMART_TABLE_(?:SPEC|ALIGN):[\s\S]*?)-->/g, (_, comment) => {
    const token = `${tableToken}-${tableMarkers.size}`;
    tableMarkers.set(token, comment);
    return `<span class="document-table-spec">${token}</span>`;
  });
  const clean = DOMPurify.sanitize(protectedHtml, htmlPolicy);
  for (const marker of clean.querySelectorAll('span.document-table-spec')) {
    const comment = tableMarkers.get(marker.textContent);
    if (comment !== undefined) marker.replaceWith(document.createComment(comment));
  }
  applySmartTableSpecs(clean);
  const allowedClasses = new Set(['smart-frontmatter', 'smart-callout', 'smart-callout-title', 'smart-caption', 'smart-figure', 'smart-figure-caption', 'success', 'warning', 'danger', 'info', 'document-diagram-slot', 'document-image-placeholder', 'document-task-marker']);
  for (const node of clean.querySelectorAll('[class]')) {
    const classes = [...node.classList].filter((name) => allowedClasses.has(name) || /^language-[a-z0-9_+-]+$/i.test(name));
    if (classes.length) node.setAttribute('class', classes.join(' '));
    else node.removeAttribute('class');
  }
  return clean;
}

function fixOutsideFences(source) {
  const lines = source.split('\n');
  const output = [];
  for (let i = 0; i < lines.length; i++) {
    const opener = lines[i].match(/^ {0,3}(`{3,}|~{3,})(.*)$/);
    if (!opener) { output.push(fixCjkEmphasis(lines[i])); continue; }
    let end;
    if (/^\s*text\s*$/i.test(opener[2]) && /^(`{3,}|~{3,})/.test(lines[i])) end = findTextFenceClose(lines, i);
    else {
      end = -1;
      const close = new RegExp(`^ {0,3}${opener[1][0]}{${opener[1].length},}\\s*$`);
      for (let j = i + 1; j < lines.length; j++) if (close.test(lines[j])) { end = j; break; }
    }
    if (end < 0) end = lines.length - 1;
    output.push(...lines.slice(i, end + 1));
    i = end;
  }
  return output.join('\n');
}

function parserFor(slots, token) {
  const parser = new Marked({ gfm: true, breaks: false });
  const fallback = new Renderer();
  parser.use({
    tokenizer: {
      fences(source) {
        if (!/^(`{3,}|~{3,})[ \t]*text[ \t]*(?:\n|$)/i.test(source)) return false;
        const lines = source.split('\n');
        const close = findTextFenceClose(lines, 0);
        const end = close < 0 ? lines.length : close;
        return { type: 'code', raw: lines.slice(0, end + 1).join('\n'), lang: 'text', text: lines.slice(1, end).join('\n') };
      },
    },
    renderer: {
      code(code, info = '') {
        const language = info.trim().split(/\s+/)[0].toLowerCase();
        if (!diagramLanguages.has(language)) return fallback.code(code, info);
        const key = `${token}-${slots.size}`;
        slots.set(key, { language, source: code });
        return `<pre class="document-diagram-slot"><code>${key}</code></pre>\n`;
      },
      checkbox(checked) { return `<span class="document-task-marker">${checked ? '☑' : '☐'}</span> `; },
      image(href, title, text) {
        return `<span class="document-image-placeholder">[이미지: ${escapeHtml(text || title || href || '')}]</span>`;
      },
    },
  });
  return parser;
}

function assertDiagramSize(source) {
  if (source.length > 50000 || source.split('\n').length > 700) throw new Error('다이어그램이 너무 큽니다. 50,000자·700줄 이하로 나누어 주세요.');
}

function assertSafeMermaid(source) {
  assertDiagramSize(source);
  // Reject configuration and active resource features BEFORE Mermaid creates its
  // temporary measurement DOM. Labels remain ordinary text; no bindFunctions run.
  if (/^\s*---(?:\s|$)/.test(source) || /%%\s*\{/.test(source)) throw new Error('다이어그램 설정 지시문은 지원하지 않습니다.');
  if (/^\s*(?:click|link|links|classDef|linkStyle|style)\b/im.test(source) || /\b(?:href|img|icon)\s*:/i.test(source)) throw new Error('링크·이미지·사용자 스타일 지시문은 지원하지 않습니다.');
  if (/(?:\b(?:https?|ftp|file|data|javascript|vbscript)\s*:|\burl\s*\(|@import|\\)/i.test(source) || /<\s*(?:script|style|img|image|iframe|object|embed|link|video|audio|source)\b/i.test(source)) throw new Error('외부 리소스를 사용하는 다이어그램은 지원하지 않습니다.');
}

function sanitizeSvg(svg, prefix, decodeMermaidLabels = false) {
  const fragment = DOMPurify.sanitize(svg, {
    USE_PROFILES: { svg: true, svgFilters: true },
    ADD_TAGS: ['style'],
    FORBID_TAGS: ['a', 'foreignObject', 'image', 'script', 'animate', 'animateMotion', 'animateTransform', 'set'],
    FORBID_ATTR: ['href', 'xlink:href', 'src', 'srcset', 'name'],
    ALLOW_DATA_ATTR: false,
    RETURN_DOM_FRAGMENT: true,
  });
  const root = fragment.querySelector('svg');
  if (!root) throw new Error('다이어그램 SVG를 만들지 못했습니다.');
  const ids = new Map();
  for (const node of [root, ...root.querySelectorAll('[id]')]) {
    const id = node.getAttribute('id');
    if (id && !ids.has(id)) ids.set(id, `${prefix}-${ids.size}`);
  }
  const localUrl = (value) => value.replace(/url\(\s*["']?#([^\s)'";]+)["']?\s*\)/g, (_, id) => ids.has(id) ? `url(#${ids.get(id)})` : 'none');
  for (const style of root.querySelectorAll('style')) {
    // This stylesheet is generated by the pinned renderer, never taken from raw
    // Markdown. Restrict it to this SVG and permit fragment-only paint servers.
    let css = style.textContent || '';
    if (/@import|@font-face|expression\s*\(|\\|(?:https?|data|file|javascript)\s*:/i.test(css) || /url\((?!\s*["']?#)/i.test(css)) { style.remove(); continue; }
    css = css.replace(/#([A-Za-z_][A-Za-z0-9_-]*)/g, (match, id) => ids.has(id) ? `#${ids.get(id)}` : match);
    style.textContent = css;
  }
  for (const node of [root, ...root.querySelectorAll('*')]) {
    for (const attribute of [...node.attributes]) {
      const name = attribute.name.toLowerCase();
      if (name.startsWith('on') || name.startsWith('data-') || /^(?:href|xlink:href|src|srcset|name)$/.test(name)) { node.removeAttribute(attribute.name); continue; }
      if (name === 'id') { node.setAttribute('id', ids.get(attribute.value)); continue; }
      if (name === 'aria-labelledby' || name === 'aria-describedby') { node.removeAttribute(attribute.name); continue; }
      if (/url\(/i.test(attribute.value)) {
        if (/\\|url\((?!\s*["']?#)/i.test(attribute.value)) node.removeAttribute(attribute.name);
        else node.setAttribute(attribute.name, localUrl(attribute.value));
      } else if (name === 'style' && /(?:@|\\|expression\s*\(|(?:https?|data|file|javascript)\s*:)/i.test(attribute.value)) node.removeAttribute(attribute.name);
    }
  }
  if (decodeMermaidLabels) {
    // Mermaid's text-label mode can retain converter punctuation entities as
    // literal text. Decode this small punctuation set on Text nodes only, after
    // sanitization; assigning nodeValue can never create markup or attributes.
    const punctuation = { 34: '"', 40: '(', 41: ')', 64: '@', 91: '[', 93: ']', 123: '{', 125: '}' };
    const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT);
    let text;
    while ((text = walker.nextNode())) {
      if (!text.parentElement?.closest('text')) continue;
      text.nodeValue = text.nodeValue.replace(/&#(34|40|41|64|91|93|123|125);|&quot;/g, (entity, code) => code ? punctuation[code] : '"');
    }
  }
  // Preserve readable diagram text on narrow screens; the figure itself owns
  // horizontal scrolling, rather than shrinking a whole graph to phone width.
  const viewBox = (root.getAttribute('viewBox') || '').trim().split(/[ ,]+/).map(Number);
  const intrinsicWidth = viewBox.length === 4 && viewBox.every(Number.isFinite) ? viewBox[2] : Number.parseFloat(root.getAttribute('width'));
  if (Number.isFinite(intrinsicWidth) && intrinsicWidth > 0) root.style.minWidth = `${Math.max(280, Math.min(1600, intrinsicWidth))}px`;
  root.removeAttribute('height');
  root.setAttribute('role', 'img');
  return root;
}

async function mermaidSvg(source, prefix, current) {
  const task = async () => {
    if (!current()) return null;
    mermaidModule ||= import('./vendor/mermaid.js');
    const { default: mermaid } = await mermaidModule;
    if (!current()) return null;
    mermaid.initialize(mermaidConfig);
    const host = document.createElement('div');
    host.className = 'document-diagram-measure';
    host.setAttribute('aria-hidden', 'true');
    document.body.appendChild(host);
    try {
      const { svg } = await mermaid.render(prefix, source, host);
      return current() ? sanitizeSvg(svg, prefix, true) : null;
    } finally {
      host.remove();
    }
  };
  const result = mermaidQueue.then(task, task);
  mermaidQueue = result.catch(() => {});
  return result;
}

function diagramError(figure, source, error) {
  const message = document.createElement('p');
  message.className = 'diagram-error document-diagram-error';
  message.textContent = `다이어그램 표시 오류: ${error instanceof Error ? error.message.slice(0, 350) : '문법을 확인해 주세요.'}`;
  const pre = document.createElement('pre');
  const code = document.createElement('code');
  code.textContent = source;
  pre.appendChild(code);
  figure.append(message, pre);
  figure.dataset.diagramStatus = 'error';
}

async function fillDiagram(figure, diagram, prefix, current) {
  try {
    assertDiagramSize(diagram.source);
    let output;
    if (diagram.language === 'jobflow') {
      const parsed = JobFlowToSVG.parseJobFlow(diagram.source);
      if (!parsed.objects.length || parsed.objects.length > 80) throw new Error('jobflow 객체 선언이나 관계를 확인해 주세요. (최대 80개 객체)');
      output = sanitizeSvg(JobFlowToSVG.generateSVG(diagram.source), prefix);
    } else if (diagram.language === 'layout') {
      output = DOMPurify.sanitize(LayoutToHTML.generateHTML(diagram.source), {
        ALLOWED_TAGS: ['div'], ALLOWED_ATTR: ['class', 'style'], ALLOW_DATA_ATTR: false, RETURN_DOM_FRAGMENT: true,
      });
    } else {
      let source = diagram.source;
      if (diagram.language === 'mermaid') assertSafeMermaid(source);
      else {
        if (!source.includes('-->')) throw new Error('전이 화살표(-->)를 확인해 주세요.');
        source = diagram.language === 'state' ? stateToMermaid(source) : navigationToMermaid(source);
      }
      output = await mermaidSvg(source, prefix, current);
    }
    if (!current() || !figure.isConnected || !output) return;
    figure.appendChild(output);
    figure.dataset.diagramStatus = 'ready';
  } catch (error) {
    if (current() && figure.isConnected) diagramError(figure, diagram.source, error);
  }
}

/** Render untrusted Markdown/SmartMD in place. Returns when its local diagrams settle. */
export async function renderDocument(container, markdown, options = {}) {
  const generation = (generations.get(container) || 0) + 1;
  generations.set(container, generation);
  documentAnchors.delete(container);
  const current = () => generations.get(container) === generation && container.isConnected;
  container.classList.add('document-markdown');
  container.dataset.renderState = 'loading';
  container.setAttribute('aria-busy', 'true');
  const source = String(markdown ?? '').replace(/\r\n?/g, '\n');
  const prefix = `architalk-doc-${++sequence}`;
  try {
    if (source.length > 2000000) throw new Error('문서가 너무 큽니다. 2,000,000자 이하로 나누어 주세요.');
    const slots = new Map();
    const token = `ARCHITALK-DIAGRAM-${crypto.randomUUID()}`;
    const parser = parserFor(slots, token);
    const transformed = preprocessSmartMd(source, (inner) => parser.parse(inner));
    const fragment = sanitizeMarkdown(parser.parse(fixOutsideFences(transformed)));
    const anchors = assignHeadingIds(fragment, prefix);
    const localLinks = safeLinks(fragment, options, anchors);
    const pending = [];
    for (const placeholder of fragment.querySelectorAll('pre.document-diagram-slot')) {
      const diagram = slots.get(placeholder.textContent);
      if (!diagram) continue;
      const figure = document.createElement('figure');
      figure.className = 'diagram document-diagram';
      figure.dataset.diagram = diagram.language;
      figure.dataset.diagramKind = diagram.language;
      figure.dataset.diagramStatus = 'pending';
      const caption = document.createElement('figcaption');
      caption.textContent = diagram.language;
      figure.appendChild(caption);
      placeholder.replaceWith(figure);
      pending.push({ figure, diagram, prefix: `${prefix}-diagram-${pending.length}` });
    }
    container.replaceChildren(fragment);
    documentAnchors.set(container, anchors);
    installDocumentLinks(container, localLinks, options, current);
    await Promise.all(pending.map(({ figure, diagram, prefix: id }) => fillDiagram(figure, diagram, id, current)));
    if (current()) {
      container.dataset.renderState = 'ready'; container.setAttribute('aria-busy', 'false');
      if (options.anchor !== undefined && options.anchor !== null) scrollToDocumentAnchor(container, options.anchor);
    }
  } catch (error) {
    if (!current()) return;
    container.replaceChildren();
    diagramError(container, source, error);
    container.dataset.renderState = 'error';
    container.setAttribute('aria-busy', 'false');
  }
}
