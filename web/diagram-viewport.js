// Trusted presentation controls for already-sanitized SVG and Layout output.
const PADDING = 12;
const STEP = 1.25;
const MAX_ZOOM = 16;
const positive = (value, fallback) => Number.isFinite(value) && value > 0 ? value : fallback;
const clamp = (value, low, high) => Math.max(low, Math.min(high, value));

export function diagramFit(width, height, availableWidth) {
  width = positive(width, 1); height = positive(height, 1);
  const viewportWidth = positive(availableWidth, 1);
  const usableWidth = Math.max(1, viewportWidth - PADDING * 2);
  const naturalHeight = height * Math.min(1, usableWidth / width);
  const viewportHeight = clamp(naturalHeight + PADDING * 2, 180, 480);
  return { width: viewportWidth, height: viewportHeight, scale: Math.min(1, usableWidth / width, (viewportHeight - PADDING * 2) / height) };
}

export function diagramOffset(offset, viewportSize, contentSize) {
  if (contentSize <= viewportSize - PADDING * 2) return (viewportSize - contentSize) / 2;
  return clamp(Number.isFinite(offset) ? offset : 0, viewportSize - PADDING - contentSize, PADDING);
}

export function diagramZoomLimit(fitScale) {
  // Even a very wide graph can reach its native readable text size.
  return Math.max(MAX_ZOOM, 1 / positive(fitScale, 1));
}

function intrinsicSize(visual) {
  if (visual.tagName.toLowerCase() !== 'svg') {
    // Layout represents a designed scene, not the current phone's CSS reflow.
    // A stable canvas makes fit/zoom comparable while preserving every child.
    visual.style.width = '720px'; visual.style.minWidth = '0'; visual.style.boxSizing = 'border-box';
    return { width: 720, height: positive(visual.offsetHeight, 48) };
  }
  const box = (visual.getAttribute('viewBox') || '').trim().split(/[ ,]+/).map(Number);
  const validBox = box.length === 4 && box.every(Number.isFinite) && box[2] > 0 && box[3] > 0;
  const width = validBox ? box[2] : positive(Number.parseFloat(visual.getAttribute('width')), 640);
  const height = validBox ? box[3] : positive(Number.parseFloat(visual.getAttribute('height')), 320);
  if (!validBox) visual.setAttribute('viewBox', `0 0 ${width} ${height}`);
  visual.style.width = '100%'; visual.style.height = '100%';
  visual.style.minWidth = '0'; visual.style.maxWidth = 'none';
  visual.setAttribute('preserveAspectRatio', 'xMidYMid meet');
  return { width, height };
}

/** Mount after sanitization; no model-authored actions, IDs or attributes enter
 * these controls. Returns a disposer for document replacement. */
export function mountDiagramViewport(figure, { current = () => figure.isConnected } = {}) {
  const visual = figure.querySelector('svg, .layout-preview');
  if (!visual) throw new Error('다이어그램 표시 영역을 만들지 못했습니다.');
  const size = intrinsicSize(visual);
  const controls = document.createElement('div');
  controls.className = 'document-diagram-controls';
  controls.setAttribute('role', 'group'); controls.setAttribute('aria-label', '도표 확대 및 이동');
  const button = (text, label) => {
    const node = document.createElement('button'); node.type = 'button'; node.textContent = text;
    node.setAttribute('aria-label', label); node.title = label; return node;
  };
  const minus = button('−', '도표 축소'), plus = button('+', '도표 확대'), fit = button('전체 보기', '전체 보기');
  const percentage = document.createElement('output');
  percentage.className = 'document-diagram-scale'; percentage.setAttribute('aria-live', 'polite');
  percentage.setAttribute('aria-label', '도표 배율');
  controls.append(minus, percentage, plus, fit);
  const viewport = document.createElement('div');
  viewport.className = 'document-diagram-viewport'; viewport.tabIndex = 0;
  viewport.setAttribute('role', 'group');
  viewport.setAttribute('aria-label', '도표. 더하기와 빼기로 확대·축소, 전체 보기로 초기화. 확대 후 드래그나 방향키로 이동');
  const camera = document.createElement('div'); camera.className = 'document-diagram-camera';
  const content = document.createElement('div'); content.className = 'document-diagram-content';
  content.style.width = `${size.width}px`; content.style.height = `${size.height}px`;
  content.appendChild(visual); camera.appendChild(content); viewport.appendChild(camera);
  const hint = document.createElement('p'); hint.className = 'document-diagram-hint';
  hint.textContent = '확대 후 드래그하거나 방향키로 이동할 수 있어요.';
  figure.append(controls, viewport, hint);
  const state = { width: 0, height: 0, fit: 1, zoom: 1, x: 0, y: 0, pointer: null, disposed: false };
  const active = () => !state.disposed && current() && figure.isConnected;
  const listeners = [];
  const listen = (node, type, handler) => {
    node.addEventListener(type, handler); listeners.push(() => node.removeEventListener(type, handler));
  };
  const canPan = () => size.width * state.fit * state.zoom > state.width - PADDING * 2 + 1 || size.height * state.fit * state.zoom > state.height - PADDING * 2 + 1;
  const draw = () => {
    if (!active()) return;
    const scale = state.fit * state.zoom;
    state.x = diagramOffset(state.x, state.width, size.width * scale);
    state.y = diagramOffset(state.y, state.height, size.height * scale);
    content.style.transform = `translate(${state.x}px, ${state.y}px) scale(${scale})`;
    viewport.dataset.zoom = String(state.zoom); viewport.dataset.mode = state.zoom === 1 ? 'fit' : 'zoom';
    viewport.dataset.panEnabled = String(canPan());
    const label = `${Math.max(0.1, Math.round(scale * 1000) / 10)}%`;
    if (percentage.textContent !== label) percentage.textContent = label;
    minus.disabled = state.zoom <= 1; plus.disabled = state.zoom >= diagramZoomLimit(state.fit);
  };
  const refresh = () => {
    if (!active() || viewport.clientWidth <= 0) return;
    if (visual.tagName.toLowerCase() !== 'svg') {
      size.height = positive(visual.offsetHeight, size.height);
      content.style.height = `${size.height}px`;
    }
    const previousScale = state.fit * state.zoom;
    const focusX = (state.width / 2 - state.x) / previousScale;
    const focusY = (state.height / 2 - state.y) / previousScale;
    const next = diagramFit(size.width, size.height, viewport.clientWidth);
    if (state.width === next.width && state.height === next.height && state.fit === next.scale) return;
    state.width = next.width; state.height = next.height; state.fit = next.scale;
    viewport.style.height = `${state.height}px`;
    const scale = state.fit * state.zoom;
    state.x = state.zoom === 1 ? (state.width - size.width * scale) / 2 : state.width / 2 - focusX * scale;
    state.y = state.zoom === 1 ? (state.height - size.height * scale) / 2 : state.height / 2 - focusY * scale;
    draw();
  };
  const zoom = (value) => {
    if (!active()) return;
    const next = clamp(value, 1, diagramZoomLimit(state.fit)), factor = next / state.zoom;
    state.x = state.width / 2 - (state.width / 2 - state.x) * factor;
    state.y = state.height / 2 - (state.height / 2 - state.y) * factor;
    state.zoom = next; draw();
  };
  const reset = () => {
    if (!active()) return;
    state.zoom = 1;
    state.x = (state.width - size.width * state.fit) / 2;
    state.y = (state.height - size.height * state.fit) / 2;
    draw();
  };
  listen(minus, 'click', () => zoom(state.zoom / STEP));
  listen(plus, 'click', () => zoom(state.zoom * STEP));
  listen(fit, 'click', reset);
  listen(viewport, 'keydown', (event) => {
    if (!active()) return;
    if (event.key === '+' || event.key === '=') zoom(state.zoom * STEP);
    else if (event.key === '-') zoom(state.zoom / STEP);
    else if (event.key === '0' || event.key === 'Home') reset();
    else if (canPan() && ['ArrowLeft', 'ArrowRight', 'ArrowUp', 'ArrowDown'].includes(event.key)) {
      const delta = event.shiftKey ? 80 : 32;
      state.x += event.key === 'ArrowLeft' ? -delta : event.key === 'ArrowRight' ? delta : 0;
      state.y += event.key === 'ArrowUp' ? -delta : event.key === 'ArrowDown' ? delta : 0;
      draw();
    } else return;
    event.preventDefault();
  });
  listen(viewport, 'pointerdown', (event) => {
    if (!active() || !canPan() || state.pointer || (event.pointerType === 'mouse' && event.button !== 0)) return;
    viewport.focus({ preventScroll: true });
    state.pointer = { id: event.pointerId, x: event.clientX, y: event.clientY, left: state.x, top: state.y };
    viewport.dataset.dragging = 'true'; viewport.setPointerCapture?.(event.pointerId); event.preventDefault();
  });
  listen(viewport, 'pointermove', (event) => {
    if (!active() || state.pointer?.id !== event.pointerId) return;
    state.x = state.pointer.left + event.clientX - state.pointer.x;
    state.y = state.pointer.top + event.clientY - state.pointer.y;
    draw(); event.preventDefault();
  });
  const stopDrag = (event) => {
    if (state.pointer?.id !== event.pointerId) return;
    state.pointer = null; viewport.dataset.dragging = 'false';
    if (viewport.hasPointerCapture?.(event.pointerId)) viewport.releasePointerCapture(event.pointerId);
  };
  listen(viewport, 'pointerup', stopDrag); listen(viewport, 'pointercancel', stopDrag); listen(viewport, 'lostpointercapture', stopDrag);
  const observer = new ResizeObserver(refresh); observer.observe(viewport);
  if (visual.tagName.toLowerCase() !== 'svg') observer.observe(visual);
  refresh();
  return () => {
    state.disposed = true; observer.disconnect();
    if (state.pointer && viewport.hasPointerCapture?.(state.pointer.id)) viewport.releasePointerCapture(state.pointer.id);
    state.pointer = null; listeners.forEach(remove => remove());
  };
}
