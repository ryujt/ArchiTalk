const API_ROOT = '/api/v1/';
const TOKEN_KEY = 'architalk.authorization';
const LEGACY_SESSION_KEY = 'architalk.session';
const ACTIVE_STATUSES = new Set(['queued', 'running', 'cancelling', 'starting']);
const esc = value => String(value ?? '').replace(/[&<>"']/g, c => ({'&':'&amp;', '<':'&lt;', '>':'&gt;', '"':'&quot;', "'":'&#39;'}[c]));
const idPath = value => encodeURIComponent(String(value));
const list = value => Array.isArray(value) ? value : [];

export function takeToken(location, history, storage) {
  const params = new URLSearchParams(location.hash.replace(/^#/, ''));
  const supplied = params.get('token');
  // Remove the capability before making any requests or updating the page.
  if (supplied !== null) history.replaceState(null, '', `${location.pathname}${location.search}`);
  let token = supplied ?? '';
  try {
    if (supplied !== null) {
      if (token) storage.setItem(TOKEN_KEY, token);
      else storage.removeItem?.(TOKEN_KEY);
    } else token = storage.getItem(TOKEN_KEY) || '';
  } catch { /* Private-mode storage may be unavailable; the in-memory token works. */ }
  return token;
}

export class ApiError extends Error {
  constructor(status, body) {
    super(body?.error?.message || `요청을 확인하지 못했어요. (HTTP ${status})`);
    this.name = 'ApiError';
    this.status = status;
    this.code = body?.error?.code || 'REQUEST_FAILED';
    this.snapshot = body?.snapshot;
  }
}

export function createApi(token, fetcher = globalThis.fetch) {
  return async function request(path, options = {}) {
    if (!path.startsWith(API_ROOT) || path.startsWith('//') || /[\\\r\n]/.test(path)) throw new Error('허용하지 않는 API 경로예요.');
    const headers = new Headers(options.headers || {});
    headers.set('Authorization', `Bearer ${token}`);
    headers.set('Accept', options.stream ? 'text/event-stream' : 'application/json');
    const multipart = typeof FormData !== 'undefined' && options.body instanceof FormData;
    if (multipart) headers.delete('Content-Type');
    if (options.body !== undefined && !multipart) headers.set('Content-Type', 'application/json');
    const response = await fetcher(path, {
      method: options.method || 'GET', headers, signal: options.signal,
      body: options.body === undefined ? undefined : multipart ? options.body : JSON.stringify(options.body),
      credentials: 'same-origin', mode: 'same-origin', redirect: 'error', cache: 'no-store'
    });
    if (options.stream && response.ok) {
      if (!response.headers.get('content-type')?.includes('text/event-stream')) throw new Error('진행 연결의 응답 형식을 확인하지 못했어요.');
      return response;
    }
    let body;
    try { body = await response.json(); } catch { body = null; }
    if (!response.ok) throw new ApiError(response.status, body);
    if (body === null) throw new Error('서버 응답을 읽지 못했어요. 상태를 다시 확인해 주세요.');
    return body;
  };
}

// This parser is shared by the live fetch stream and protocol tests. It keeps
// decoder state across byte boundaries and dispatches only complete SSE frames.
export function createSSEParser(onEvent, {maxBuffer = 1024 * 1024} = {}) {
  const decoder = new TextDecoder('utf-8');
  let buffer = '', data = [], event = '', eventId, first = true, frameLength = 0;
  function line(value) {
    if (first) { value = value.replace(/^\uFEFF/, ''); first = false; }
    if (value === '') {
      if (data.length) onEvent({data: data.join('\n'), event: event || 'message', ...(eventId !== undefined ? {id: eventId} : {})});
      data = []; event = ''; eventId = undefined; frameLength = 0;
      return;
    }
    frameLength += value.length;
    if (frameLength > maxBuffer) throw new Error('진행 이벤트가 너무 커서 상태를 다시 확인해야 해요.');
    if (value.startsWith(':')) return;
    const colon = value.indexOf(':');
    const field = colon < 0 ? value : value.slice(0, colon);
    let content = colon < 0 ? '' : value.slice(colon + 1);
    if (content.startsWith(' ')) content = content.slice(1);
    if (field === 'data') data.push(content);
    else if (field === 'event') event = content;
    else if (field === 'id' && !content.includes('\0')) eventId = content;
  }
  function drain(final = false) {
    let start = 0;
    for (let i = 0; i < buffer.length; i++) {
      if (buffer[i] !== '\n' && buffer[i] !== '\r') continue;
      if (buffer[i] === '\r' && i === buffer.length - 1 && !final) break;
      line(buffer.slice(start, i));
      if (buffer[i] === '\r' && buffer[i + 1] === '\n') i++;
      start = i + 1;
    }
    buffer = buffer.slice(start);
    if (buffer.length + frameLength > maxBuffer) throw new Error('진행 연결의 크기 제한을 넘었어요.');
  }
  return {
    push(bytes) { buffer += decoder.decode(bytes, {stream: true}); drain(); },
    end() { buffer += decoder.decode(); drain(true); /* Incomplete frame is intentionally discarded. */ }
  };
}

export async function readSSE(response, onEvent, signal) {
  if (!response.body) throw new Error('진행 스트림을 사용할 수 없어요.');
  const reader = response.body.getReader();
  const parser = createSSEParser(onEvent);
  const abort = () => { reader.cancel().catch(() => {}); };
  signal?.addEventListener('abort', abort, {once: true});
  try {
    while (!signal?.aborted) {
      const {done, value} = await reader.read();
      if (done) { parser.end(); return; }
      parser.push(value);
    }
  } finally {
    signal?.removeEventListener('abort', abort);
    reader.releaseLock();
  }
}

export function draftIsCurrent(session, draft) {
  return Boolean(draft && Number(draft.input_revision) === Number(session?.design_revision));
}

export function approvalBlockers(session, draft) {
  if (!draft) return ['아직 완성된 초안이 없어요.'];
  const blockers = [];
  if (!draftIsCurrent(session, draft)) blockers.push('새 답변을 반영해 다시 정리해 주세요.');
  if (!draft.reviewed) blockers.push('독립 비판 검토가 아직 완료되지 않았어요.');
  for (const coverage of list(session?.coverage)) {
    if (coverage.required && (coverage.status === 'unknown' ||
        (coverage.status === 'deferred' && (coverage.blocker || !coverage.owner || !coverage.follow_up)) ||
        (coverage.status === 'not_applicable' && coverage.assessment_status !== 'user_confirmed'))) {
      blockers.push(`${coverage.title}: ${coverage.status === 'unknown'?'확인 필요':coverage.status === 'deferred'?'보류 조건 확인 필요':'해당 없음 근거 확인 필요'}`);
    }
  }
  for (const finding of list(session?.findings)) {
    if (finding.draft_id === draft.id && ['critical','major'].includes(finding.severity) && !['rejected','resolved'].includes(finding.disposition)) blockers.push(`검토 지적: ${finding.text}`);
  }
  if (list(session?.requirements).some(requirement=>!list(requirement.evidence_refs).length || !requirement.acceptance)) blockers.push('요구사항의 근거와 수락 조건을 확인해 주세요.');
  return blockers;
}

export function safeRelativeDirectory(path) {
  return Boolean(path && !/^[\\/~]|^[A-Za-z]:|[\0\r\n]/.test(path) && !path.split(/[\\/]/).some(part => part === '..' || part === '.' || !part) && !path.includes('\\'));
}

export function settingsFromValues(current, values) {
  const settings = structuredClone(current);
  const value = name => String(values[name] || '').trim() || null;
  if (Object.hasOwn(values, 'timeout-seconds')) {
    const timeout = Number(values['timeout-seconds']);
    if (!Number.isInteger(timeout) || timeout < 10 || timeout > 3600) throw new ApiError(400, {error:{code:'INVALID_SETTINGS',message:'작업 시간 제한은 10~3600초 사이의 정수로 입력해 주세요.'}});
    settings.ai.timeout_seconds = timeout;
  }
  settings.ai.default_backend = value('default-backend');
  for (const backend of ['claude','codex']) {
    settings.ai.backends[backend].model = value(backend+'-model');
    settings.ai.backends[backend].effort = value(backend+'-effort');
  }
  for (const role of ['conversation','design','critic']) settings.ai.roles[role] = {
    ...settings.ai.roles[role], backend:value(role+'-backend'), model:value(role+'-model'), effort:value(role+'-effort')
  };
  return settings;
}


function runIsActive(run) { return Boolean(run && ACTIVE_STATUSES.has(run.status)); }
function unknownTermination(run) { return Boolean(run && (run.process_termination === 'unknown' || run.termination === 'unknown')); }
function runTimedOut(run) { return run?.error?.code === 'timeout'; }
function runTimeoutLabel(run) {
  const seconds = run?.effective_settings?.timeout_seconds;
  if (!Number.isSafeInteger(seconds) || seconds <= 0) return '';
  return `${seconds}초${seconds >= 60 && seconds % 60 === 0 ? ` (${seconds / 60}분)` : ''}`;
}
function describeStatus(run) {
  return ({queued:'작업 대기 중', starting:'로컬 도구 시작 중', running:'답변을 받고 있어요', cancelling:'도구의 종료를 확인하고 있어요', completed:'작업 완료', succeeded:'작업 완료', cancelled:'중지 확인됨', interrupted:'중단된 작업', failed:'작업 실패'})[run?.status] || run?.status || '작업 상태 확인';
}
function formatTime(value) {
  if (!value) return '';
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? '' : date.toLocaleString('ko-KR', {month:'short', day:'numeric', hour:'2-digit', minute:'2-digit'});
}
function button(action, label, cls = 'secondary', disabled = false, extra = '') {
  return `<button type="button" class="${cls}" data-action="${action}"${disabled ? ' disabled' : ''}${extra}>${label}</button>`;
}

const LEGACY_DOCUMENT_TYPE = Object.freeze({ kind: 'architecture', filename: '아키텍처-설계.md', title: '아키텍처 설계' });

/** Server-owned kinds and filenames are the only package navigation targets. */
export function documentsForDraft(workspace, currentDraft) {
  if (!currentDraft) return [];
  const seenKinds = new Set(), seenFiles = new Set();
  const catalog = list(workspace?.document_types).filter((entry) => {
    if (!entry || typeof entry.kind !== 'string' || !/^[a-z][a-z0-9_]*$/.test(entry.kind) || typeof entry.filename !== 'string' || !/^[^/\\?#\u0000-\u001f]+\.md$/iu.test(entry.filename) || typeof entry.title !== 'string') return false;
    const filename = entry.filename.normalize('NFC');
    if (seenKinds.has(entry.kind) || seenFiles.has(filename)) return false;
    seenKinds.add(entry.kind); seenFiles.add(filename); return true;
  });
  const primary = catalog.find((entry) => entry.kind === 'architecture') || LEGACY_DOCUMENT_TYPE;
  const documents = [{ ...primary, markdown: typeof currentDraft.markdown === 'string' ? currentDraft.markdown : '' }];
  for (const type of catalog) {
    if (type.kind === 'architecture') continue;
    const document = list(currentDraft.documents).find((entry) => entry?.kind === type.kind && typeof entry.markdown === 'string');
    if (document) documents.push({ ...type, markdown: document.markdown });
  }
  return documents;
}

export function startWorkspace(doc = document, win = window) {
  const $ = id => doc.getElementById(id);
  let storage;
  try { storage = win.sessionStorage; } catch {}
  const token = takeToken(win.location, win.history, storage);
  const transportApi = createApi(token, win.fetch.bind(win));
  // Selection is project state; keep it in this page only. Retire the old key.
  try { storage?.removeItem(LEGACY_SESSION_KEY); } catch {}
  const state = {
    workspace: null, session: null, run: null, scene: 'loading', panel: 'notes',
    workspaceEpoch: 0, connectionBlocked: false, connectionError: null,
    selectedDraft: null, selectedDocument: 'architecture', documentAnchor: null, busy: false, error: null, notice: '', live: '', progress: '',
    connection: 'connecting', modal: null, modalError: '', settings: null,
    diagnostics: null, choices: {}, settingsRevision: null, plan: null, saved: null,
    answer: '', intro: '', cursor: '', feed: null, refreshTimer: null, diagnosticTimer: null,
    leftCollapsed: Boolean(win.matchMedia?.('(max-width:1030px)').matches),
    rightCollapsed: Boolean(win.matchMedia?.('(max-width:760px)').matches),
    fileTabActive: false,
    contextPaths: new Set(),
    refreshPromise: null, requestEpoch: 0, dialogFocus: null, dialogFocusAction: null,
    restoreFocus: false, dialogScroll: null, runScene: 'interview', disposed: false
  };
  let renderedScene = null, renderedConversation = null, followConversation = false;
  let documentPreview = null, documentRenderer = null;
  let fileWorkspace=null, documentRecorder=null, viewerMenu=null, viewerPromise=null;
  let transcriptionDialog=null, transcriptionEpoch=0, transcriptionRequest=null;
  let lastReading='';
  const dialog = $('scene-dialog');
  const draft = () => list(state.session?.drafts).find(d => d.id === state.selectedDraft) || list(state.session?.drafts).at(-1);
  const availableDocuments = () => documentsForDraft(state.workspace, draft());
  const selectedDocument = () => {
    const documents = availableDocuments();
    const selected = documents.find((item) => item.kind === state.selectedDocument) || documents[0];
    if (selected) state.selectedDocument = selected.kind;
    return selected;
  };
  const approvalIssues = () => draft()?.id === list(state.session?.drafts).at(-1)?.id && Array.isArray(state.session?.readiness_issues) ? state.session.readiness_issues : approvalBlockers(state.session,draft());
  const canApprove = () => !approvalIssues().length && !runIsActive(state.run) && !unknownTermination(state.run);
  const globalRun = () => state.workspace?.active_run;
  const disabled = () => state.connectionBlocked || state.busy || runIsActive(state.run) || unknownTermination(state.run) || runIsActive(globalRun()) || unknownTermination(globalRun());
  const canRetryConversation = () => Boolean(state.session && state.run?.kind === 'conversation'
    && ['failed','cancelled','interrupted'].includes(state.run.status)
    && ['confirmed','manually_confirmed'].includes(state.run.process_termination));
  const canRetryDesign = () => Boolean(state.session && state.run?.kind === 'design'
    && ['failed','cancelled','interrupted'].includes(state.run.status)
    && ['confirmed','manually_confirmed'].includes(state.run.process_termination));
  const designGeneration = (includeCompleted = false) => {
    if (state.run?.kind === 'critic') {
      if (!includeCompleted) return null;
      const latestDraft = list(state.session?.drafts).at(-1);
      return latestDraft ? list(state.session?.design_generations).findLast(generation => generation.draft_id === latestDraft.id) || null : null;
    }
    const generation = list(state.session?.design_generations).at(-1);
    if (!generation || generation.input_revision !== state.session?.design_revision || (!includeCompleted && generation.draft_id)) return null;
    const language = state.workspace?.settings?.documents?.language || state.settings?.documents?.language || 'ko';
    if (!runIsActive(state.run) && generation.language !== language) return null;
    return generation;
  };
  const designRetryLabel = () => list(designGeneration()?.completed).length ? '남은 문서 이어 만들기' : '초안 다시 생성';
  const envelope = (payload, revision = state.session?.revision) => ({command_id: win.crypto.randomUUID(), expected_revision: revision, payload});
  const sessionPath = suffix => `${API_ROOT}sessions/${idPath(state.session.id)}${suffix}`;

  async function api(path, options = {}) {
    if (state.connectionBlocked) throw state.connectionError;
    const epoch = state.workspaceEpoch;
    try {
      const result = await transportApi(path, options);
      if (state.connectionBlocked) throw state.connectionError;
      if (epoch !== state.workspaceEpoch) throw new ApiError(409, {error:{code:'workspace_view_changed',message:'프로젝트 기록이 바뀌었어요. 새 기록에서 다시 시작해 주세요.'}});
      return result;
    } catch (error) {
      if (error instanceof ApiError && (error.code === 'workspace_reset_required' || error.status === 401)) invalidateConnection(error);
      throw error;
    }
  }

  function clearProjectView(workspace = null) {
    state.requestEpoch++; state.workspaceEpoch++;
    documentRecorder?.clear();viewerMenu?.clear();clearTranscriptionDialog();fileWorkspace?.clear();state.fileTabActive=false;
    state.feed?.abort(); state.feed = null;
    win.clearTimeout(state.refreshTimer); state.refreshTimer = null;
    win.clearTimeout(state.diagnosticTimer); state.diagnosticTimer = null;
    state.workspace = workspace; state.session = null; state.run = null;
    state.selectedDraft = null; state.selectedDocument = 'architecture'; state.documentAnchor = null;
    state.plan = null; state.saved = null; state.lastExport = null;
    state.live = ''; state.progress = ''; state.answer = ''; state.intro = ''; state.cursor = '';
    state.contextPaths.clear(); state.notice = ''; state.error = null; state.busy = false;
    state.modal = null; state.modalError = ''; state.dialogFocus = null; state.dialogFocusAction = null;
    state.restoreFocus = false; state.dialogScroll = null;
    if (dialog.open) dialog.close();
    dialog.innerHTML = ''; delete dialog.dataset.modal;
    documentPreview = null; renderedScene = null; renderedConversation = null; followConversation = false;
    state.scene = 'welcome';
  }

  function invalidateConnection(error) {
    if (state.connectionBlocked) return;
    clearProjectView();
    state.connectionBlocked = true; state.connectionError = error; state.scene = 'disconnected';
    try { storage?.removeItem(TOKEN_KEY); storage?.removeItem(LEGACY_SESSION_KEY); } catch {}
    connection('offline'); render();
    announce(error.code === 'connection_replaced' ? '새 서버에 연결하고 있어요.' : '이전 프로젝트 기록을 비웠어요. 서버를 다시 실행하고 새 접속 주소를 열어 주세요.');
    $('main')?.focus?.({preventScroll:true});
    win.scrollTo?.({left:0, top:0, behavior:'instant'});
  }

  function announce(text) { $('announcer').textContent = text; }
  function connection(status) {
    if (state.connectionBlocked) status = 'disconnected';
    state.connection = status;
    const el = $('connection');
    el.dataset.state = status === 'connected' ? 'online' : 'offline';
    el.lastElementChild.textContent = ({disconnected:'새 접속 필요', connecting:'연결 확인 중', connected:'로컬에 연결됨', polling:'재연결 · 상태 확인 중', offline:'연결 확인 필요'})[status] || status;
    if (state.workspace || state.scene!=='loading') renderNotice();
  }

  function updateScene({recover = false, keep = false} = {}) {
    if (!state.session) { state.scene = 'welcome'; return; }
    if (state.run?.kind==='diagnostic' && (runIsActive(state.run)||unknownTermination(state.run))) { state.scene='diagnostic'; return; }
    if (runIsActive(state.run)) {
      if (state.scene !== 'generating') state.runScene = state.scene === 'review' && draft() ? 'review' : 'interview';
      state.scene = 'generating'; return;
    }
    if (unknownTermination(state.run) || recover) { state.scene = 'recovery'; return; }
    if (keep && ['interview','review','saved'].includes(state.scene)) return;
    if (state.run && ['failed','interrupted'].includes(state.run.status) && state.run.kind!=='diagnostic') { state.scene = 'error'; return; }
    if (state.run?.status === 'cancelled') { state.scene = 'paused'; return; }
    state.scene = state.run?.kind === 'conversation' && ['completed','succeeded'].includes(state.run.status) ? 'interview' : draft() ? 'review' : 'interview';
  }

  async function refresh({initial = false, keep = true} = {}) {
    if (state.connectionBlocked) return;
    if (state.refreshPromise) {
      await state.refreshPromise;
      return refresh({initial,keep});
    }
    const epoch = state.requestEpoch;
    state.refreshPromise = (async () => {
      const workspace = await api(`${API_ROOT}workspace`);
      if (epoch !== state.requestEpoch) return;
      state.workspace = workspace;
      const sessions = list(workspace.sessions);
      const wanted = state.session?.id;
      const selected = sessions.find(s => s.id === wanted) || (initial ? sessions.at(-1) : null);
      if (selected) {
        const session = await api(`${API_ROOT}sessions/${idPath(selected.id)}`);
        if (epoch !== state.requestEpoch) return;
        state.session = session.session || session;
        const runId = state.session.active_run_id || (workspace.active_run?.session_id === state.session.id ? workspace.active_run.id : null) || list(workspace.runs).filter(run=>run.session_id===state.session.id).at(-1)?.id;
        if (runId) {
          const run = await api(`${API_ROOT}runs/${idPath(runId)}`);
          if (epoch !== state.requestEpoch) return;
          const resolved=run.run || run;
          if (state.session.active_run_id===resolved.id && !runIsActive(resolved) && !unknownTermination(resolved)) {
            // Completion committed between the session and run reads. Preserve
            // the current stage and obtain a coherent newer session snapshot.
            scheduleRefresh();
            return;
          }
          state.run = resolved;
        } else state.run = null;
        const lastExport=list(workspace.exports).filter(entry=>entry.session_id===state.session.id).at(-1);
        state.lastExport=lastExport || null;
        if (!state.feed && state.session.cursor !== undefined) state.cursor=String(state.session.cursor);
        updateScene({recover: initial && !runIsActive(state.run), keep});
      } else {
        if (state.session) clearProjectView(workspace);
        state.session=null;
        state.run=workspace.active_run?.kind==='diagnostic'?workspace.active_run:null;
        state.scene=state.run?'diagnostic':'welcome';
      }
      state.error = null;
      render();
      ensureFeed();
      void ensureFileWorkspace();
    })().finally(() => { state.refreshPromise = null; });
    return state.refreshPromise;
  }

  function scheduleRefresh() {
    if (state.connectionBlocked) return;
    if (state.refreshTimer) return;
    state.refreshTimer = win.setTimeout(() => {
      state.refreshTimer = null;
      refresh({keep: true}).catch(() => { connection('polling'); });
    }, 80);
  }

  function ensureFeed() {
    if (state.connectionBlocked) return;
    if (globalRun()?.kind==='diagnostic' && runIsActive(globalRun())) {
      if (!state.diagnosticTimer && !state.disposed) state.diagnosticTimer=win.setTimeout(()=> {
        state.diagnosticTimer=null;
        refresh({keep:true}).catch(()=>{connection('offline');ensureFeed();});
      },1500);
    } else if (state.diagnosticTimer) { win.clearTimeout(state.diagnosticTimer); state.diagnosticTimer=null; }
    const sessionId = state.session?.id;
    if (!sessionId) { state.feed?.abort(); state.feed = null; return; }
    if (state.feed?.sessionId === sessionId) return;
    state.feed?.abort();
    const controller = new AbortController();
    controller.sessionId = sessionId;
    state.feed = controller;
    void watchSession(sessionId, controller);
  }

  async function watchSession(sessionId, controller) {
    let failures = 0;
    while (!controller.signal.aborted && !state.disposed && !state.connectionBlocked) {
      try {
        const path = `${API_ROOT}sessions/${idPath(sessionId)}/events${state.cursor ? `?after=${encodeURIComponent(state.cursor)}` : ''}`;
        const response = await api(path, {stream: true, signal: controller.signal});
        connection('connected'); failures = 0;
        await readSSE(response, frame => {
          if (controller.signal.aborted || state.session?.id !== sessionId) return;
          let event;
          try { event = JSON.parse(frame.data); } catch { scheduleRefresh(); return; }
          if (event.type === 'workspace_reset_required' || frame.event === 'workspace_reset_required') {
            invalidateConnection(new ApiError(410, {error:{code:'workspace_reset_required',message:event.error?.message || '프로젝트 기록을 더 사용할 수 없어요.'}}));
            return;
          }
          if (event.session_id && event.session_id !== sessionId) return;
          if (event.event_id !== undefined) state.cursor = String(event.event_id);
          else if (frame.id !== undefined) state.cursor = frame.id;
          if (event.type === 'delta' || event.type === 'token_delta') {
            state.live += String(event.payload?.text ?? event.payload?.delta ?? '');
            if (state.live.length > 32000) state.live = state.live.slice(-32000);
            const output = $('live-output'); if (output) output.textContent = state.live;
          } else {
            if (event.payload?.stage || event.payload?.message) state.progress = String(event.payload.stage || event.payload.message);
            scheduleRefresh();
          }
        }, controller.signal);
        if (!controller.signal.aborted) throw new Error('진행 연결이 끝났어요.');
      } catch (error) {
        if (controller.signal.aborted || state.connectionBlocked) return;
        connection('polling'); failures++;
        // A lost feed never creates another run. Reconcile durable snapshots and
        // poll while reconnecting; this also handles expired replay cursors.
        try { await refresh({keep: true}); } catch { connection('offline'); }
        if (error instanceof ApiError && error.status === 401) { showError(error); return; }
        if (failures > 1) state.cursor = '';
        await new Promise(resolve => {
          const timeout = win.setTimeout(resolve, Math.min(1000 * failures, 5000));
          controller.signal.addEventListener('abort', () => { win.clearTimeout(timeout); resolve(); }, {once:true});
        });
      }
    }
  }

  function showError(error, modal = false) {
    if (state.connectionBlocked) return;
    const message = error instanceof ApiError ? error.message : '로컬 서버와 상태를 확인하지 못했어요. 연결을 확인한 뒤 다시 확인해 주세요.';
    if (modal) state.modalError = message;
    else state.error = {message, code: error.code || 'CONNECTION_FAILED'};
    announce(message); render();
  }

  async function command(task, {modal = false} = {}) {
    if (state.busy || state.connectionBlocked) return;
    state.busy = true; state.error = null; state.modalError = ''; render();
    try { await task(); }
    catch (error) {
      if (error instanceof ApiError && (error.status === 409 || error.snapshot)) {
        try { await refresh({keep:true}); } catch {}
      }
      showError(error, modal);
    } finally { state.busy = false; render(); }
  }

  function adoptResult(result, {reset = true} = {}) {
    // A command receipt supersedes every snapshot that started before it.
    // Session identity alone cannot detect an older same-session GET response.
    if (result.session || result.run) state.requestEpoch++;
    if (result.session) state.session = result.session;
    if (result.run) { state.run = result.run; state.live = ''; state.progress = ''; }
    if (reset) updateScene();
    ensureFeed();
  }

  async function selectSession(id) {
    if (state.busy || state.connectionBlocked) return;
    fileWorkspace?.showChat();
    if(win.matchMedia?.('(max-width:1030px)').matches){state.leftCollapsed=true;renderPanels();}
    state.requestEpoch++;
    state.feed?.abort(); state.feed = null; state.cursor = ''; state.run = null;
    state.selectedDraft = null; state.selectedDocument = 'architecture'; state.documentAnchor = null; state.answer = ''; state.error = null; state.saved=null; state.lastExport=null;
    await command(async () => {
      const session = await api(`${API_ROOT}sessions/${idPath(id)}`);
      state.session = session.session || session;
      state.scene = 'recovery';
      await refresh({keep: true});
      if (!runIsActive(state.run)) state.scene = 'recovery';
    });
  }

  function render() {
    renderPanels();
    let scroll = {left: win.scrollX, top: win.scrollY, behavior: 'instant'};
    const scene = state.scene === 'generating' ? state.runScene : state.scene;
    const messages = list(state.session?.messages);
    const conversation = JSON.stringify([state.session?.id, messages.length, messages.at(-1)?.id, runIsActive(state.run) ? state.run.id : null]);
    if (scene === 'interview' && !state.fileTabActive) {
      if (scene !== renderedScene || conversation !== renderedConversation) followConversation = true;
    } else followConversation = false;
    renderedScene = scene; renderedConversation = conversation;
    const workspace = state.workspace;
    $('project-name').textContent = workspace?.project_name || '프로젝트';
    $('sidebar-project').textContent = workspace?.project_name || '프로젝트';
    $('root-display').textContent = workspace?.root_display || '';
    $('workspace-title').textContent = state.session?.title || '프로젝트를 소개해 주세요';
    const phase = ({disconnected:'새 접속 필요',loading:'연결 확인', welcome:'시작하기', recovery:'이어가기', diagnostic:'도구 진단 확인', interview:'이야기 나누기', generating:describeStatus(state.run), review:state.session?.content_status === 'approved' ? '검토본' : '초안 검토', paused:'중지 확인', error:'작업 확인', saved:'저장 완료'})[state.scene] || '설계';
    $('phase').textContent = phase;
    for (const name of ['interview','review','save']) $('step-'+name).classList.toggle('active', name === (['review','saved'].includes(state.scene) ? state.scene === 'saved' ? 'save' : 'review' : 'interview'));
    renderSessions(); renderNotice();
    const content = $('workspace-content');
    // Templates and source text are escaped. The document renderer separately
    // sanitizes the rich preview before it enters the application DOM.
    const focused = doc.activeElement;
    const editing = focused?.id === 'answer-input' || focused?.id === 'project-intro';
    const selection = editing ? [focused.selectionStart, focused.selectionEnd] : null;
    if(!state.fileTabActive){content.innerHTML = renderStage();renderDocumentPreview(content);}
    if (editing && !state.fileTabActive) {
      const replacement = $(focused.id);
      if (replacement) { replacement.focus({preventScroll: true}); replacement.setSelectionRange(...selection); }
    }
    renderAux();
    renderRecorderControls();
    if (state.connectionBlocked) for (const control of doc.querySelectorAll?.('button,select,input,textarea') || []) control.disabled = true;
    if (state.modal) renderModal();
    if (state.restoreFocus && !state.modal) {
      state.restoreFocus=false;
      const returnTarget=state.dialogFocus?.isConnected?state.dialogFocus:
        state.dialogFocusAction?doc.querySelector?.(`[data-action="${state.dialogFocusAction}"]`):null;
      (returnTarget || $('main')).focus({preventScroll: true});
      if (state.dialogScroll) scroll = state.dialogScroll;
    }
    if (followConversation && !state.modal) {
      // The auxiliary panel can be taller than the conversation. Follow the
      // composer rather than the bottom of the whole document.
      $('composer-end')?.scrollIntoView?.({block: 'end', inline: 'nearest', behavior: 'instant'});
      followConversation = false;
    } else if (win.scrollX !== scroll.left || win.scrollY !== scroll.top) {
      // Replacing the stage or restoring focus must not move someone who is
      // reading older messages during a snapshot or modal update.
      win.scrollTo?.(scroll);
    }
  }

  function renderPanels() {
    doc.body.classList.toggle('left-panel-collapsed',state.leftCollapsed);
    doc.body.classList.toggle('right-panel-collapsed',state.rightCollapsed);
    for (const [id, collapsed, label] of [
      ['sidebar-toggle',state.leftCollapsed,'프로젝트 패널'],
      ['info-toggle',state.rightCollapsed,'설계 메모']
    ]) {
      const control=$(id), action=`${label} ${collapsed?'펼치기':'접기'}`;
      control?.setAttribute('aria-expanded',String(!collapsed));
      control?.setAttribute('title',action);
      control?.setAttribute('aria-label',action);
    }
  }

  function renderDocumentPreview(content) {
    const target = content.querySelector?.('[data-document-preview]');
    if (!target) { documentPreview = null; return; }
    const current = draft(), document = selectedDocument();
    if (!current || !document) return;
    const documents = availableDocuments();
    const documentLinks = documents.map(({kind, filename}) => ({kind, filename}));
    const catalogKey = JSON.stringify(documentLinks);
    const anchor = state.documentAnchor;
    state.documentAnchor = null;
    const sessionId = state.session.id, draftId = current.id;
    if (documentPreview?.sessionId === sessionId && documentPreview.draftId === draftId && documentPreview.kind === document.kind && documentPreview.markdown === document.markdown && documentPreview.catalogKey === catalogKey) {
      const preview = documentPreview;
      target.replaceWith(preview.node);
      if (anchor !== null) void preview.ready?.then(async () => {
        const { scrollToDocumentAnchor } = await documentRenderer;
        if (!state.disposed && preview.node.isConnected && state.session?.id === sessionId && draft()?.id === draftId && state.selectedDocument === preview.kind) scrollToDocumentAnchor(preview.node, anchor);
      });
      return;
    }
    const preview = {sessionId, draftId, kind: document.kind, markdown: document.markdown, catalogKey, node: target};
    documentPreview = preview;
    documentRenderer ||= import('./document-renderer.js');
    preview.ready = documentRenderer.then(({renderDocument}) => {
      if (!state.disposed && target.isConnected) return renderDocument(target, document.markdown, {
        documentLinks, currentDocument: document.kind, anchor,
        onDocumentLink: ({kind, anchor: destination}) => {
          if (state.session?.id === sessionId && draft()?.id === draftId && target.isConnected) selectDocument(kind, destination);
        },
      });
    }).catch(() => {
      if (!state.disposed && target.isConnected) {
        target.dataset.renderState = 'error'; target.setAttribute('aria-busy', 'false');
        target.textContent = '문서를 표시하지 못했어요. 아래의 Markdown 원문을 확인하거나 화면을 새로고침해 주세요.';
      }
    });
  }

  function selectDocument(kind, anchor = null) {
    if (state.connectionBlocked) return;
    if (!availableDocuments().some((document) => document.kind === kind)) return;
    state.selectedDocument = kind;
    state.documentAnchor = anchor ?? '';
    if (runIsActive(state.run)) { state.runScene = 'review'; state.scene = 'generating'; }
    else state.scene = 'review';
    state.notice = ''; render();
  }

  function inDocumentPreview(target) {
    const preview=target.closest?.('[data-document-preview],[data-file-preview]');
    return preview?.dataset?.documentPreview!==undefined || preview?.dataset?.filePreview!==undefined;
  }

  function renderSessions() {
    const sessions = list(state.workspace?.sessions);
    $('session-list').innerHTML = sessions.map(s => `<button class="session-item${s.id === state.session?.id ? ' selected' : ''}" type="button" data-session="${esc(s.id)}"${state.busy?' disabled':''}><span class="session-icon" aria-hidden="true">◌</span><span>${esc(s.title || '대화')}<span class="session-sub">${list(s.messages).length ? `${list(s.messages).length}개의 대화 기록` : `기록 ${esc(s.revision)}`}</span></span></button>`).join('');
    $('session-switcher').innerHTML = `<option value="">새 대화 시작</option>${sessions.map(s => `<option value="${esc(s.id)}"${s.id === state.session?.id?' selected':''}>${esc(s.title || '대화')}</option>`).join('')}`;
  }

  function renderNotice() {
    if (state.connectionBlocked) { $('notice').innerHTML = ''; return; }
    let html = '';
    if (!token) html += `<div class="notice error"><b>접속 권한을 확인해 주세요.</b><p>ArchiTalk를 실행한 터미널의 전체 접속 주소로 다시 열어 주세요.</p></div>`;
    if (state.error) html += `<div class="notice error" role="alert"><b>${esc(state.error.message)}</b><p class="small">${esc(state.error.code)}</p><div class="button-row">${button('refresh','상태 다시 확인')}${button('settings','도구 설정 확인','quiet-button')}</div></div>`;
    if (state.notice) html += `<div class="notice" role="status">${esc(state.notice)}</div>`;
    if (globalRun()?.kind==='diagnostic' && (runIsActive(globalRun())||unknownTermination(globalRun())) && state.scene!=='diagnostic') html += `<div class="notice">${unknownTermination(globalRun())?'도구 진단 프로세스의 종료를 확인해야 해요.':'로컬 도구의 설치·help 진단을 실행하고 있어요.'} 새 AI 작업은 종료 확인 후 시작할 수 있어요.<div class="button-row">${button('diagnostic-status','진단 작업 확인')}</div></div>`;
    else if (globalRun() && globalRun().kind!=='diagnostic' && globalRun().session_id !== state.session?.id && (runIsActive(globalRun()) || unknownTermination(globalRun()))) html += `<div class="notice">다른 대화의 로컬 작업을 확인하고 있어요. 종료가 확인된 뒤 새 작업을 시작할 수 있어요.<div class="button-row"><button type="button" class="secondary" data-session="${esc(globalRun().session_id)}">진행 중인 대화 보기</button></div></div>`;
    if (state.connection === 'polling' || state.connection === 'offline') html += `<div class="notice">진행 연결을 다시 확인하고 있어요. 서버 기록을 조회하며 기존 작업에 다시 연결해요.</div>`;
    $('notice').innerHTML = html;
  }

  function renderStage() {
    if (state.connectionError?.code === 'connection_replaced') return `<p class="loading" role="status">새 서버에 연결하고 있어요…</p>`;
    if (state.connectionBlocked) return `<section class="status-card"><div class="status-icon warn" aria-hidden="true">↶</div><h1>${state.connectionError?.code === 'workspace_reset_required' ? '프로젝트 기록 연결이 초기화됐어요' : '이전 서버 연결을 사용할 수 없어요'}</h1><p>${esc(state.connectionError?.message || '서버 연결을 다시 확인해 주세요.')}</p><p>ArchiTalk 서버를 종료한 뒤 해당 프로젝트에서 다시 실행하고, 터미널에 표시된 새 접속 주소를 열어 주세요.</p></section>`;
    if (state.scene === 'loading') return `<p class="loading" role="status">로컬 워크스페이스를 확인하고 있어요.</p>`;
    if (state.scene === 'welcome') return welcome();
    if (state.scene === 'recovery') return recovery();
    if (state.scene === 'diagnostic') return diagnosticStage();
    if (state.scene === 'generating') return generating();
    if (state.scene === 'error' || state.scene === 'paused') return errorStage();
    if (state.scene === 'saved') return saved();
    if (state.scene === 'review' && draft()) return review();
    return interview();
  }

  function contextChoices() {
    const files = list(state.workspace?.context_files).filter(file=>file.selectable!==false);
    if (!files.length) return `<p class="context-summary">자동으로 선택된 참고 파일이 없어요. 프로젝트 소개로 시작할 수 있어요.</p>`;
    return `<details class="context-picker"><summary>대화에 전달할 참고 파일 선택 · ${files.length}개</summary><p class="context-summary">선택한 파일만 참고해요. 전달할 범위와 파일을 확인한 뒤 포함해 주세요.</p>${files.map((file,i) => `<label class="context-choice"><input type="checkbox" name="context-path" value="${esc(file.path)}" id="context-${i}"${state.contextPaths.has(file.path)?' checked':''}><span><b>${esc(file.path)}</b>${file.size!==undefined?` <small>${esc(file.size)} bytes</small>`:''}${file.summary?`<span class="context-summary">${esc(file.summary)}</span>`:''}${file.excerpt?`<span class="context-summary"><pre>${esc(file.excerpt)}</pre></span>`:''}</span></label>`).join('')}</details>`;
  }

  function welcome() {
    return `<section class="welcome"><div class="welcome-emblem" aria-hidden="true">✳</div><h1>어떤 서비스를<br>만들고 싶으세요?</h1><p class="intro">목적과 중요한 조건을 하나씩 이야기해 주세요.<br>지금 규모에 맞는 설계를 함께 정리해요.</p><form id="start-form" class="starter-card"><label for="project-intro">프로젝트를 한두 문장으로 소개해 주세요</label><textarea class="starter-input" id="project-intro" name="intro" required maxlength="20000" placeholder="누구를 위한 서비스인지, 어떤 문제를 풀고 싶은지 들려주세요.">${esc(state.intro)}</textarea>${contextChoices()}<div class="starter-footer"><span>아직 정하지 않은 내용은 함께 좁혀 가요.</span><button type="submit" class="primary"${disabled()||!token?' disabled':''}>${state.busy?'준비 중':'대화 시작'} <span aria-hidden="true">→</span></button></div></form><div class="guidance-row"><div class="guidance-card"><span aria-hidden="true">◌</span><h3>한 번에 한 가지씩</h3><p>작게 시작하고, 중요한 선택을 먼저 정리해요.</p></div><div class="guidance-card"><span aria-hidden="true">▤</span><h3>검토한 뒤 문서로</h3><p>초안과 미정을 살펴본 다음 파일에 반영해요.</p></div></div><p class="privacy-note">선택한 로컬 CLI의 기존 로그인을 사용해요. 전체 소스나 인증 정보를 자동으로 전달하지 않아요.</p></section>`;
  }

  function pendingMessage() {
    const label = state.run?.status === 'cancelling' ? '도구의 종료를 확인하고 있어요' : ({conversation: '답변 작성 중…', design: '설계 초안 작성 중…', critic: '설계 초안 검토 중…'})[state.run?.kind] || '작업 진행 중…';
    const limit = runTimeoutLabel(state.run);
    return `<article class="chat-turn pending-turn" aria-busy="true"><div class="avatar" aria-hidden="true">a</div><div class="chat-message"><div class="sender">ArchiTalk</div><p role="status">${esc(label)}</p><p class="small muted">완료되면 이 자리에 결과를 보여드려요.</p>${generationProgress(true)}${limit?`<p class="small muted">요청당 시간 제한: ${esc(limit)}<br>설정 변경은 다음 작업부터 적용돼요.</p>`:''}<div class="button-row">${button('cancel',state.run?.status==='cancelling'?'종료 확인 중':'작업 중지','quiet-button',state.busy||state.run?.status==='cancelling')}</div></div></article>`;
  }

  function generationProgress(includeCompleted = false) {
    if (!['design','critic'].includes(state.run?.kind)) return '';
    const generation = designGeneration(includeCompleted);
    if (!generation) return '';
    const catalog = list(state.workspace?.document_types);
    const total = catalog.length || 10;
    const completed = list(generation.completed);
    const count = Math.min(completed.length, total);
    const labels = new Map(catalog.map(document => [document.kind, document.title]));
    const target = !generation.draft_id && list(state.workspace?.document_generation_order)[completed.length];
    const current = target ? `현재 문서: ${target.title || target.filename || target.kind}` : generation.draft_id ? '전체 문서 비판 검토' : state.run?.progress;
    return `<section class="generation-progress" aria-label="문서 생성 진행"><p><strong>문서 ${count}/${total} 완료</strong></p><progress value="${count}" max="${total}" aria-label="완료한 설계 문서"></progress>${current?`<p class="small muted" data-current-document>${esc(current)}</p>`:''}${completed.length?`<details><summary>작성한 문서 ${count}개</summary><ul>${completed.map(part=>`<li>${esc(labels.get(part.document?.kind) || part.document?.kind || '문서')}</li>`).join('')}</ul></details>`:''}<p class="small muted">${generation.draft_id?'문서를 종합한 뒤 전체 비판 검토를 진행해요.':'중간 문서는 프로젝트에 보존돼요. 전체 검증이 끝나야 초안으로 저장할 수 있어요.'}</p></section>`;
  }

  function restartDesignButton() {
    return canRetryDesign() && designGeneration() ? `${button('restart-design','처음부터 새로 생성','quiet-button',disabled())}<p class="small muted">처음부터 생성하면 중간 문서를 재사용하지 않아요. 대화와 완성된 초안은 유지돼요.</p>` : '';
  }

  function messages(pending = false) {
    return `<div class="chat-thread">${list(state.session?.messages).map(message => `<article class="chat-turn ${message.role === 'user'?'user':''}"><div class="avatar ${message.role === 'user'?'user':''}" aria-hidden="true">${message.role === 'user'?'나':'a'}</div><div class="chat-message"><div class="sender">${message.role === 'user'?'나':message.role === 'assistant'?'ArchiTalk':'안내'}<span class="time">${esc(formatTime(message.created_at))}</span></div><pre>${esc(message.content)}</pre></div></article>`).join('')}${pending?pendingMessage():''}</div>`;
  }

  function composer(nextStage = false) {
    const label = nextStage ? draft() ? '다음 단계로 · 초안 다시 정리' : '다음 단계로 · 초안 만들기' : draft() ? '현재 답변으로 다시 정리' : '현재 답변으로 초안 만들기';
    return `<form id="answer-form" class="composer"><label for="answer-input" class="sr-only">${draft()?'설계 수정 요청 또는 답변':'답변'}</label><textarea id="answer-input" name="content" required maxlength="20000" placeholder="생각을 들려주세요. 아직 정하지 않았다는 답변도 괜찮아요."${disabled()?' disabled':''}>${esc(state.answer)}</textarea><div class="composer-bottom"><span>Enter로 보내기 · Shift+Enter 줄바꿈</span><button type="submit" class="send-button" aria-label="답변 보내기"${disabled()?' disabled':''}>↑</button></div></form><div class="composer-help" id="composer-end"><span>${nextStage?'아직 정하지 않은 조건은 미정으로 남겨요.':'확인된 답변은 대화에 기록돼요.'}</span>${button('generate',label,nextStage?'primary':'quiet-button',disabled())}</div>`;
  }

  function interview(pending = false) {
    const question=list(state.session?.questions).find(q=>q.id===state.session?.last_question_id && q.status==='pending');
    return `${messages(pending)}${question?`<div class="question-why">이 질문을 드리는 이유: ${esc(question.reason)}</div>${list(question.options).length?`<div class="quick-answers">${question.options.map(option=>button('quick-answer',esc(option),'quick-answer',disabled(),` data-answer="${esc(option)}"`)).join('')}</div>`:''}`:''}<p class="checkpoint">✓ 서버에 기록된 대화 · 설계 기록 ${esc(state.session?.design_revision)}</p>${draft()?`<div class="inline-notice">${draftIsCurrent(state.session,draft())?'현재 대화 기준 초안이 있어요.':'이전 대화 기준 초안이 있어요. 새 답변은 다시 정리할 때 반영돼요.'} ${button('review','초안 살펴보기','quiet-button')}</div>`:''}${composer(true)}`;
  }

  function generating() {
    return state.runScene === 'review' && draft() ? review(true) : interview(true);
  }

  function review(pending = false) {
    const currentDraft = draft(), current = draftIsCurrent(state.session,currentDraft);
    const approved = currentDraft.content_status === 'approved';
    const issues = approvalIssues(), documents = availableDocuments(), document = selectedDocument();
    const count = documents.length, packaged = count > 1;
    const approveLabel = approved ? packaged ? `전체 ${count}개 검토본 확정됨` : '검토본 확정됨' : packaged ? `전체 ${count}개 문서를 검토본으로 확정` : '이 버전을 검토본으로 확정';
    return `<p class="intro">${packaged ? `같은 초안의 ${count}개 설계 문서를 살펴보고, 바꾸고 싶은 내용은 대화로 들려주세요.` : '완성된 초안을 살펴보고, 바꾸고 싶은 내용은 대화로 들려주세요.'}</p><article class="document-preview"><div class="document-toolbar"><label class="document-kind-control" for="document-kind-picker"><span>문서 종류 · ${count}개</span><select id="document-kind-picker" class="document-kind-picker" aria-label="문서 종류 선택">${documents.map((item) => `<option value="${esc(item.kind)}"${item.kind === document.kind ? ' selected' : ''}>${esc(item.title)}</option>`).join('')}</select></label><label class="document-version-control" for="draft-picker"><span>초안 버전</span><select class="draft-picker" id="draft-picker" aria-label="초안 버전 선택">${list(state.session.drafts).map((item,i)=>`<option value="${esc(item.id)}"${item.id===currentDraft.id?' selected':''}>초안 ${i+1} · 설계 기록 ${esc(item.input_revision)}</option>`).join('')}</select></label><span class="document-version-status">${approved?'검토본':current?'현재 대화 기준 초안':'이전 대화 기준 초안'}</span></div><div class="document-paper"><p class="doc-label">${esc(document.filename)} · 설계 기록 ${esc(currentDraft.input_revision)} 기준${current?' · 현재 대화와 일치':' · 새 답변 반영 필요'}</p><div class="markdown" data-document-preview data-document-kind="${esc(document.kind)}" data-render-state="loading" aria-busy="true"><p role="status">문서를 표시하고 있어요…</p></div><details class="document-source"><summary>현재 문서의 Markdown 원문 보기</summary><pre>${esc(document.markdown)}</pre></details></div></article>${findings()}${issues.length?`<details class="warning-box"><summary>검토본 확정 전에 확인할 내용 · ${issues.length}개</summary><ul>${issues.map(issue=>`<li>${esc(issue)}</li>`).join('')}</ul></details>`:''}<div class="review-actions"><p class="review-note">${packaged ? `문서 선택은 보기에만 적용돼요. 이 초안의 ${count}개 문서를 함께 확정·저장해요.` : '이 초안의 아키텍처 설계 문서 1개를 확정·저장해요.'}<br>${approved?'현재 설계 기록의 내용을 검토본으로 확정했어요.':current?'내용 확정과 파일 저장은 별개예요. 초안으로 저장하면 가정과 미정을 그대로 남겨요.':'이전 초안을 그대로 저장할 수 있어요. 현재 버전 확정은 다시 정리한 뒤 가능해요.'}</p><div class="button-row">${button('approve',approveLabel,'secondary',state.busy||approved||!canApprove())}${button('save-preview',packaged ? `전체 ${count}개 문서 반영 ↗` : '문서에 반영 ↗','primary',state.busy||runIsActive(state.run))}</div></div>${pending?pendingMessage():''}${composer()}${button('interview','대화 기록 살펴보기','quiet-button')}`;
  }

  function findings() {
    const values = list(state.session?.findings).filter(f=>f.draft_id===draft()?.id);
    if (!values.length) return `<div class="inline-notice">${draft()?.reviewed?'완료된 비판 검토에서 지적이 없어요.':'독립 비판 검토가 아직 완료되지 않았어요.'} 초안의 가정과 미정도 함께 확인해 주세요.${!draft()?.reviewed?`<div class="button-row">${button('critic','이 초안의 비판 검토 실행','secondary',disabled()||!draftIsCurrent(state.session,draft()))}</div>`:''}</div>`;
    return `<section aria-label="검토 메모"><h3>검토 메모</h3>${values.map(f => {
      const resolved = Boolean(f.disposition);
      return `<div class="finding ${resolved?'resolved':''}"><b>${esc(({critical:'중대',major:'주요',minor:'참고'})[f.severity] || f.severity)}</b><p>${esc(f.text)}</p><details><summary>근거와 영향</summary><p>${esc(f.evidence)}</p><p>${esc(f.impact)}</p><p>${esc(f.proposed_resolution)}</p></details><p class="small">${resolved?`처리: ${esc(({accepted:'수정 필요 수용',rejected:'근거를 들어 기각',deferred:'추후 검토',resolved:'해결됨'})[f.disposition]||f.disposition)} · ${esc(f.disposition_reason)}`:'검토 결정을 기록해 주세요.'}</p><div class="button-row">${button('finding-decision',resolved?'검토 결정 수정':'검토 결정 기록','secondary',disabled(),` data-finding="${esc(f.id)}"`)}</div></div>`;
    }).join('')}</section>`;
  }

  function recovery() {
    const run = state.run, last = list(state.session?.messages).at(-1), lastExport = state.lastExport;
    const retry = canRetryConversation(), retryDesign = canRetryDesign();
    return `<section class="status-card"><div class="status-icon ${unknownTermination(run)?'warn':''}" aria-hidden="true">↶</div><h1>남겨 둔 설계에서 이어가세요</h1><p>서버에 확인된 대화와 초안을 불러왔어요. 중단된 작업을 자동으로 다시 요청하지 않아요.</p>${generationProgress()}${runTimedOut(run)?`<div class="warning-box"><p>${esc(timeoutExplanation())}</p>${button('timeout-settings','시간 제한 조정','secondary',state.busy)}</div>`:''}<div class="recovery-details"><div><span>대화 주제</span><strong>${esc(state.session?.title)}</strong></div><div><span>마지막 대화</span><strong>${esc(formatTime(last?.created_at)||'기록 없음')}</strong></div><div><span>문서 상태</span><strong>${draft()?`초안 ${list(state.session.drafts).length}개 · ${draftIsCurrent(state.session,draft())?'현재':'이전'} 대화 기준`:'아직 생성하지 않음'}</strong></div>${run?`<div><span>최근 작업</span><strong>${esc(describeStatus(run))}</strong></div>`:''}${lastExport?`<div><span>최근 문서 반영</span><strong>${lastExport.status==='completed'?'저장 완료':lastExport.status==='preview'?'미리보기만 확인':lastExport.status==='conflict'?'충돌 · 새 사본 확인 필요':'저장 결과 확인 필요'}</strong></div>`:''}</div>${lastExport?.result?.paths?.length?`<div class="saved-path">${lastExport.result.paths.map(path=>`<strong>${esc(path)}</strong>`).join('')}</div>`:''}${unknownTermination(run)?`<div class="warning-box">이전 로컬 도구의 종료가 확인되지 않았어요. 실행한 터미널에서 해당 작업을 정리한 뒤 종료를 확인해 주세요. 확인 전에는 새 AI 작업을 시작할 수 없어요.</div><label class="confirm-row"><input type="checkbox" id="termination-confirm">이전 작업의 로컬 도구를 직접 종료했어요.</label><div class="button-row">${button('confirm-termination','수동 종료 확인','secondary',state.busy)}</div>`:''}<div class="button-row">${retry?button('retry-conversation','답변 다시 받기','primary',disabled()):''}${retryDesign?button('generate',designRetryLabel(),'primary',disabled()):''}${button('interview','이어서 대화하기',retry||retryDesign?'secondary':'primary',state.busy)}${draft()?button('review','초안 먼저 살펴보기','secondary',state.busy):''}${button('new-session','새 대화 시작','quiet-button',state.busy)}</div>${restartDesignButton()}</section>`;
  }

  function diagnosticStage() {
    const run=globalRun()?.kind==='diagnostic'?globalRun():state.run;
    const unknown=unknownTermination(run);
    return `<section class="status-card"><div class="status-icon ${unknown?'warn':''}" aria-hidden="true">${unknown?'!':'◌'}</div><h1>${unknown?'도구의 종료를 확인해 주세요':'설치·help 진단을 확인하고 있어요'}</h1><p>${unknown?'이전 도구 진단 프로세스의 종료가 확인되지 않았어요. 실행한 터미널에서 해당 도구를 직접 정리한 뒤 확인해 주세요.':'설치와 지원 옵션을 확인하는 중이에요. 이 작업은 모델을 호출하지 않아요. 진단이 끝나면 도구 설정에서 결과를 확인할 수 있어요.'}</p>${unknown?`<label class="confirm-row"><input type="checkbox" id="diagnostic-termination-confirm">진단에서 시작한 로컬 도구를 직접 종료했어요.</label><div class="button-row">${button('confirm-diagnostic-termination','진단 프로세스 수동 종료 확인','primary',state.busy)}</div>`:`<div class="button-row">${button('refresh','진단 상태 다시 확인','secondary',state.busy)}</div>`}<div class="button-row">${button('settings','도구 설정 보기','quiet-button',state.busy)}${state.session?button('interview','대화 기록 보기','quiet-button',state.busy):''}</div><p class="progress-foot">종료가 확인될 때까지 새 AI 작업이나 진단을 시작하지 않아요.</p></section>`;
  }

  function errorStage() {
    const cancelled = state.scene === 'paused';
    const retry = canRetryConversation(), retryDesign = canRetryDesign(), timeout = runTimedOut(state.run);
    const reviewFirst = state.run?.kind === 'critic' && draft();
    const detail = state.run?.error ? esc(typeof state.run.error === 'string' ? state.run.error : state.run.error.message || state.run.error.code) : '';
    const explanation = timeout ? timeoutExplanation() : cancelled ? '서버가 로컬 도구의 종료를 확인했어요.' : '마지막으로 확인된 대화와 완성된 초안에서 이어갈 수 있어요.';
    return `<section class="status-card"><div class="status-icon ${cancelled?'warn':'error'}" aria-hidden="true">${cancelled?'Ⅱ':'!'}</div><h1>${timeout?'작업 시간이 초과됐어요':cancelled?'작업 중지를 확인했어요':'작업을 완료하지 못했어요'}</h1><p>${esc(explanation)}</p>${generationProgress()}${detail?timeout?`<details class="error-detail"><summary>세부 오류</summary><p>${detail}</p></details>`:`<div class="error-detail">${detail}</div>`:''}<div class="button-row">${timeout?button('timeout-settings','시간 제한 조정',retryDesign?'primary':'secondary',state.busy):''}${retry?button('retry-conversation','답변 다시 받기','primary',disabled()):''}${retryDesign?button('generate',designRetryLabel(),timeout?'secondary':'primary',disabled()):''}${button('interview','대화로 돌아가기',retry||retryDesign||reviewFirst?'secondary':'primary')}${draft()?button('review','완성된 초안 보기',reviewFirst?'primary':'secondary'):''}${state.run?.kind!=='design'?button('generate','현재 답변으로 다시 정리','secondary',disabled()):''}${button('settings','도구 설정 확인','quiet-button')}</div>${restartDesignButton()}<p class="progress-foot">${timeout?'시간 제한 변경은 다음 작업부터 적용돼요. 작업은 자동으로 재실행하지 않아요.':retry?'답변 다시 받기는 마지막 사용자 입력으로 새 AI 작업을 시작해요. 대화에 같은 입력을 추가하지 않아요.':'다시 생성하기는 새 AI 작업을 시작해요. 이전 응답을 자동으로 재실행하지 않아요.'}</p></section>`;
  }

  function timeoutExplanation() {
    const operation = ({conversation:'답변 작성',design:'설계 초안 작성',critic:'설계 초안 검토'})[state.run?.kind] || 'AI 작업';
    const limit = runTimeoutLabel(state.run);
    return `${operation} 요청에 적용된 ${limit ? `${limit}의 시간 제한에 도달했어요.` : '시간 제한에 도달했어요.'} 대화와 완성된 초안은 그대로 보존되어 있어요. 시간 제한을 조정한 뒤 다시 요청할 수 있어요.`;
  }

  function saved() {
    const result = state.saved || {}, files = list(result.paths);
    return `<section class="status-card"><div class="status-icon" aria-hidden="true">✓</div><h1>설계 문서를 저장했어요</h1><p>서버가 ${files.length > 1 ? `전체 ${files.length}개 문서의 ` : ''}파일 저장을 확인했어요. 다음에 반영할 때도 새 개정 폴더를 사용해요.</p><div class="saved-path"><span>이번에 저장한 문서</span>${files.length?files.map(file=>`<strong>${esc(typeof file==='string'?file:file.path || file.relative_path)}</strong>`).join(''):`<strong>${esc(result.path || result.directory || result.bundle_path || '')}</strong>`}</div><div class="button-row">${button('review','문서 살펴보기','primary')}${button('interview','대화 이어가기')}</div></section>`;
  }

  function renderAux() {
    for (const panel of ['notes','documents']) {
      const tab = $(panel+'-tab'); tab.classList.toggle('active',state.panel===panel);
      tab.setAttribute('aria-selected',String(state.panel===panel)); tab.tabIndex=state.panel===panel?0:-1;
    }
    $('aux-content').setAttribute('aria-labelledby',state.panel+'-tab');
    if (state.panel === 'documents') {
      const documents = availableDocuments(), selected = selectedDocument();
      $('aux-content').innerHTML = `<div class="panel-heading"><h3>설계 문서</h3><span class="mini-label">${draft()?`${documents.length}개 문서`:'미생성'}</span></div>${draft()?documents.map((document) => `<button type="button" class="document-item${document.kind === selected?.kind ? ' selected' : ''}" data-action="inspect-document" data-document-kind="${esc(document.kind)}"${document.kind === selected?.kind ? ' aria-current="page"' : ''}><span class="document-icon" aria-hidden="true">▤</span><span><b>${esc(document.title)}</b><small>${esc(document.filename)}<br>설계 기록 ${esc(draft().input_revision)} · ${draftIsCurrent(state.session,draft())?'현재 대화 기준':'이전 대화 기준'}</small></span></button>`).join(''):`<p class="context-intro">대화를 시작하고 ‘현재 답변으로 초안 만들기’를 선택하면 여기에 완성된 초안이 표시돼요.</p>`}<p class="context-intro">문서 선택은 보기에만 적용돼요. 반영할 때는 같은 초안의 문서를 모두 저장해요.</p>`;
      return;
    }
    const session=state.session;
    const requirements=list(session?.requirements).map(value=>`<div class="fact-card"><div class="fact-title">요구 · ${esc(value.status)}</div><p>${esc(value.text)}</p><p class="source">수락 조건: ${esc(value.acceptance)}<br>근거: ${esc(list(value.evidence_refs).join(', '))}</p></div>`).join('');
    const claims=list(session?.claims).map(value=>`<div class="fact-card"><div class="fact-title">${esc(({assumption:'가정',proposal:'제안',interpretation:'해석'})[value.kind]||value.kind)} · ${esc(value.status)}</div><p>${esc(value.text)}</p><p class="source">근거 진술: ${esc(value.source_quote)}</p>${value.status==='proposed'?button('confirm-claim','내 요구로 확인','quiet-button',disabled(),` data-claim="${esc(value.id)}"`):''}</div>`).join('');
    const decisions=list(session?.decisions).map(value=>`<div class="fact-card"><div class="fact-title">결정 · ${esc(value.status)}</div><p>${esc(value.title)}</p><details class="source"><summary>대안과 선택 이유</summary><p>${esc(list(value.alternatives).join(' / '))}</p><p>${esc(value.rationale)}</p><p>${esc(value.consequences)}</p></details></div>`).join('');
    const coverage=list(session?.coverage).filter(value=>value.status!=='covered').map(value=>`<div class="fact-card unresolved"><div class="fact-title">${value.blocker?'확정 전에 확인 필요':'미정'} · ${esc(value.status)}</div><p>${esc(value.title)}</p>${value.reason?`<p class="source">${esc(value.reason)}</p>`:''}${value.status==='deferred'?`<p class="source">담당: ${esc(value.owner)} · 다음 확인: ${esc(value.follow_up)}</p>`:''}${!value.blocker?button('defer-coverage','담당과 다음 확인을 정해 보류','quiet-button',disabled(),` data-coverage="${esc(value.id)}"`):'<p class="source">중요한 실패·안전 조건은 대화로 확인해 주세요.</p>'}</div>`).join('');
    $('aux-content').innerHTML = `<div class="panel-heading"><h3>이야기에서 모은 내용</h3><span class="mini-label">${session?`설계 기록 ${esc(session.design_revision)}`:'대화 전'}</span></div>${requirements?`<h3 class="small">근거가 연결된 요구 · 확인 전 제안 포함</h3>${requirements}`:''}${claims?`<h3 class="small">해석과 가정</h3>${claims}`:''}${decisions?`<h3 class="small">설계 결정</h3>${decisions}`:''}${coverage?`<h3 class="small">아직 확인할 내용</h3>${coverage}`:''}${!requirements&&!claims&&!coverage&&!decisions?`<p class="context-intro">아직 정리된 메모가 없어요. 확인된 요구와 가정은 대화가 진행되면 여기에 구분해서 표시돼요.</p>`:''}<details class="scope-details"><summary>프로젝트에서 확인한 참고 파일</summary>${list(state.workspace?.context_files).map(file=>`<div class="file-row"><span class="file-symbol" aria-hidden="true">▤</span><span><b>${esc(file.path)}</b><small>${esc(file.summary || '대화 시작 전 전달 범위 선택')}</small></span></div>`).join('')||'<p>참고 파일 없음</p>'}</details>`;
  }

  function header(title) { return `<div class="dialog-header"><div><p class="eyebrow">ARCHITALK</p><h2 id="dialog-title">${title}</h2></div>${button('close-modal','×','dialog-close',state.busy,' aria-label="닫고 이전 화면으로 돌아가기"')}</div>`; }
  function modalError() { return state.modalError?`<div class="notice error dialog-error" role="alert">${esc(state.modalError)}</div>`:''; }
  function findingModal() {
    const finding=list(state.session?.findings).find(item=>item.id===state.findingId);
    return `${header('검토 지적에 대한 결정')}<div class="dialog-body"><p>${esc(finding?.text)}</p><div id="modal-error">${modalError()}</div><form id="finding-form"><div class="form-field"><label for="finding-disposition">처리 방법</label><select id="finding-disposition" name="disposition"><option value="accepted">수정할 지적으로 수용 · 정리 후 다시 검토</option><option value="rejected">근거를 들어 기각</option><option value="deferred">추후 검토로 보류</option><option value="resolved">이미 해결됨을 근거와 함께 확인</option></select></div><div class="form-field"><label for="finding-reason">판단 근거와 후속 처리</label><input id="finding-reason" name="reason" required maxlength="2000" value="${esc(finding?.disposition_reason || '')}"></div><p class="capability-note">주요 지적을 수용하거나 보류하면 해결 확인 전까지 검토본 확정이 제한돼요.</p><div class="dialog-footer">${button('close-modal','취소','quiet-button')}<button type="submit" class="primary">검토 결정 기록</button></div></form></div>`;
  }
  function coverageModal() {
    const coverage=list(state.session?.coverage).find(item=>item.id===state.coverageId);
    return `${header('미정인 조건 보류하기')}<div class="dialog-body"><p>${esc(coverage?.title)} · 결정하지 않은 내용은 미정으로 남겨요. 담당과 다음 확인을 함께 기록해 주세요.</p><div id="modal-error">${modalError()}</div><form id="coverage-form"><div class="form-field"><label for="coverage-reason">지금 보류하는 이유</label><input id="coverage-reason" name="reason" required maxlength="2000" value="${esc(coverage?.reason || '')}"></div><div class="form-field"><label for="coverage-owner">확인할 담당</label><input id="coverage-owner" name="owner" required maxlength="200" value="${esc(coverage?.owner || '')}"></div><div class="form-field"><label for="coverage-follow-up">언제 무엇을 확인할지</label><input id="coverage-follow-up" name="follow_up" required maxlength="2000" value="${esc(coverage?.follow_up || '')}"></div><div class="dialog-footer">${button('close-modal','취소','quiet-button')}<button type="submit" class="primary">보류 조건 기록</button></div></form></div>`;
  }
  function renderModal() {
    dialog.classList.toggle('save-dialog',state.modal==='save');
    // Preserve typed settings and path edits during background snapshots.
    if (dialog.open && dialog.dataset.modal === state.modal) {
      const error = $('modal-error'); if (error) error.innerHTML = modalError();
      for (const el of dialog.querySelectorAll('button[type="submit"], [data-action="diagnose"], [data-action="export-apply"]')) el.disabled = state.busy;
      return;
    }
    dialog.innerHTML = state.modal === 'settings' ? settingsModal() : state.modal === 'finding' ? findingModal() : state.modal === 'coverage' ? coverageModal() : saveModal();
    dialog.dataset.modal = state.modal;
    if (!dialog.open) dialog.showModal();
  }

  function knownChoices(backend, field) {
    return [...new Set(list(state.choices?.[backend]?.[field==='model'?'models':'efforts']).filter(value=>typeof value==='string' && value))];
  }

  function choiceOptions(backend, field, value) {
    const choices=knownChoices(backend,field);
    return `<option value=""${!choices.includes(value)?' selected':''}>직접 입력</option>${choices.map(choice=>`<option value="${esc(choice)}"${choice===value?' selected':''}>${esc(choice)}</option>`).join('')}`;
  }

  function settingField(name, field, value, backend, inherit = false) {
    const id=`${name}-${field}`, label=field==='model'?'모델':'생각 수준';
    const owner=({claude:'Claude Code',codex:'Codex',conversation:'대화',design:'설계 작성',critic:'비판 검토'})[name];
    const placeholder=inherit?'선택한 도구의 기본 설정':field==='model'?'CLI 기본 모델':'CLI 기본값';
    return `<div class="form-field"><label for="${id}">${label}${inherit?' 재정의':''}</label><select class="setting-choice" id="${id}-choice" data-choice-input="${id}" aria-label="${owner} ${label} 목록" aria-describedby="${id}-help">${choiceOptions(backend,field,value || '')}</select><input id="${id}" name="${id}" value="${esc(value || '')}" placeholder="${placeholder}" maxlength="${field==='model'?200:32}" autocomplete="off" spellcheck="false" aria-describedby="${id}-help"><small id="${id}-help">목록에서 고르거나 직접 입력해요. 빈칸은 ${inherit?'선택한 도구의 기본 설정':'CLI 기본값'}을 따라요.</small></div>`;
  }

  function refreshSettingChoices() {
    for (const name of ['claude','codex','conversation','design','critic']) {
      const backend=['claude','codex'].includes(name)?name:($(`${name}-backend`).value || $('default-backend').value);
      for (const field of ['model','effort']) {
        const input=$(`${name}-${field}`), choice=$(`${name}-${field}-choice`);
        choice.innerHTML=choiceOptions(backend,field,input.value.trim());
      }
    }
  }

  function settingsModal() {
    const settings = state.settings || {}, ai = settings.ai || {}, backends = ai.backends || {};
    return `${header('답변에 사용할 도구')}<div class="dialog-body"><p>로컬 Claude Code 또는 Codex의 기존 로그인을 사용해요. 설치·help 진단은 로그인이나 모델 응답을 시험하지 않아요. 에이전트·모델·생각 수준·시간 제한은 모든 프로젝트에 공통 적용되며, 문서 저장 폴더는 현재 프로젝트에만 적용돼요.</p><div id="modal-error">${modalError()}</div><form id="settings-form"><div class="form-field"><label for="default-backend">기본 도구</label><select id="default-backend" name="default-backend">${['claude','codex'].map(kind=>`<option value="${kind}"${(ai.default_backend||'claude')===kind?' selected':''}>${kind==='claude'?'Claude Code':'Codex'}</option>`).join('')}</select></div><div class="form-field"><label for="timeout-seconds">작업 시간 제한(초)</label><input id="timeout-seconds" name="timeout-seconds" type="number" min="10" max="3600" step="1" required value="${esc(ai.timeout_seconds ?? 1800)}" aria-describedby="timeout-seconds-help"><small id="timeout-seconds-help">각 AI 요청을 기다리는 시간을 10~3600초 사이의 정수로 설정해요. 형식 보정이나 비판 검토가 이어지면 요청마다 제한을 따로 적용해요. 변경은 다음 작업부터 적용돼요.</small></div><div class="diagnostics" id="cli-diagnostics">${diagnosticsHTML()}</div>${button('diagnose','설치·버전 진단','secondary',state.busy)}${['claude','codex'].map(kind=>{
      const base=backends[kind] || {};
      return `<section class="settings-section"><h3>${kind==='claude'?'Claude Code':'Codex'} 기본 설정</h3><div class="form-field"><label for="${kind}-executable">신뢰된 실행 파일</label><input id="${kind}-executable" readonly value="${esc(base.executable || kind)}"><small>실행 파일 변경은 터미널의 사용자 전역 설정에서 해 주세요.</small></div>${kind==='codex'?`<div class="form-field"><label for="codex-profile">Codex profile</label><input id="codex-profile" readonly value="${esc(base.profile || 'CLI 기본 profile')}"><small>신뢰된 사용자 전역 설정의 profile을 사용해요.</small></div>`:''}<div class="field-grid">${settingField(kind,'model',base.model,kind)}${settingField(kind,'effort',base.effort,kind)}</div><p class="capability-note">목록은 알려진 값의 예시예요. 계정·CLI·모델별 사용 가능 여부는 실제 요청에서 확인돼요. 목록에 없는 값도 직접 입력할 수 있어요.</p></section>`;
    }).join('')}<details class="settings-section" open><summary>역할마다 다르게 설정하기 · 선택</summary>${['conversation','design','critic'].map((role,i)=>{
      const value=ai.roles?.[role] || {};
      const backend=value.backend || ai.default_backend || 'claude';
      return `<fieldset class="role-setting"><legend>${['대화','설계 작성','비판 검토'][i]}</legend><div class="field-grid"><div class="form-field"><label for="${role}-backend">사용할 도구</label><select id="${role}-backend" name="${role}-backend"><option value="">기본 도구 따름</option>${['claude','codex'].map(kind=>`<option value="${kind}"${value.backend===kind?' selected':''}>${kind==='claude'?'Claude Code':'Codex'}</option>`).join('')}</select></div>${settingField(role,'model',value.model,backend,true)}${settingField(role,'effort',value.effort,backend,true)}</div></fieldset>`;
    }).join('')}</details><div class="connection-notice">설정은 다음 생성부터 적용돼요. 진행 중인 작업은 시작할 때의 설정을 유지해요. 적용 후 작업을 자동으로 재실행하지 않아요.</div><div class="dialog-footer">${button('close-modal','취소','quiet-button',state.busy)}<button type="submit" class="primary"${state.busy?' disabled':''}>다음 작업부터 적용</button></div></form></div>`;
  }

  function diagnosticsHTML() {
    const diagnostics = list(state.diagnostics);
    if (!diagnostics.length) return `<div class="diagnostic">설치·버전<b>진단 전</b></div><div class="diagnostic">모델·생각 수준<b>지원 미확인</b></div><div class="diagnostic">로그인<b>요청 전 미확인</b></div>`;
    return diagnostics.map(item=>`<div class="diagnostic">${esc(item.backend==='claude'?'Claude Code':item.backend==='codex'?'Codex':item.backend)}<b>${item.available?item.compatible?'설치·옵션 확인':'호환성 확인 필요':'설치 확인 필요'}</b>${item.version?`<span>${esc(item.version)}</span>`:''}<p>${esc(item.message)}</p></div>`).join('') + `<div class="diagnostic">로그인·모델<b>실제 요청 전 미확인</b></div>`;
  }

  function saveModal() {
    const plan = state.plan, files = list(plan?.files), count = plan ? files.length : availableDocuments().length, packaged = count > 1;
    const applyLabel=packaged ? `전체 ${count}개를 ${draft()?.content_status==='approved'?'검토본':'초안'}으로 저장` : draft()?.content_status==='approved'?'검토본으로 저장':'초안으로 저장';
    const entries=files.map(file=>`<div class="plan-file diff-line"><b>＋ ${esc(typeof file==='string'?file:`${plan.relative_directory}/${file.relative_path}`)}</b>${file.content || file.preview?`<details class="plan-preview"><summary>원문 미리보기</summary><pre>${esc(file.content || file.preview)}</pre></details>`:''}</div>`).join('');
    const preview=plan?`<div class="diff"><div class="diff-header"><span>전체 ${files.length}개 문서 · 설계 기록 ${esc(plan.document_revision ?? draft()?.input_revision)}</span><span>${draftIsCurrent(state.session,draft())?'현재':'이전'} 대화 기준</span></div>${entries||`<div class="diff-line">${esc(plan.path || plan.bundle_path || plan.directory)}</div>`}</div><div class="inline-notice">저장 대상: 이 초안의 전체 ${files.length}개 문서 · 내용 상태: ${draft()?.content_status==='approved'?'검토본':'초안'}<br>파일 저장과 내용 확정은 별개예요.</div>`:'';
    const actions=plan?`${button('export-apply',applyLabel,'primary',state.busy)}${canApprove()&&draft()?.content_status!=='approved'?button('approve-export',packaged ? `전체 ${count}개를 확정하고 저장` : '검토본으로 확정하고 저장','secondary',state.busy):''}`:`<button type="submit" form="save-form" class="primary"${state.busy?' disabled':''}>새 미리보기 확인</button>`;
    return `${header(packaged ? '전체 문서에 반영하기' : '문서에 반영하기')}<div class="dialog-body save-dialog-body"><p>${packaged ? `지금 보고 있는 문서와 함께 이 초안의 전체 ${count}개 문서를 저장해요. ` : ''}저장할 폴더와 파일 목록을 확인해 주세요. 원문은 각 파일의 미리보기를 펼쳐 볼 수 있어요.</p><div id="modal-error">${modalError()}</div><form id="save-form"><div class="form-field"><label for="output-directory">저장 폴더 · 프로젝트 기준 상대 경로</label><input id="output-directory" name="directory" value="${esc(plan?.relative_directory || '')}" placeholder="비우면 프로젝트 이름으로 제안해요"><small>기존 문서는 보존하고 새 개정 폴더에 저장해요. 폴더를 바꾸면 다시 미리보기를 확인해 주세요.</small></div><button type="submit" class="secondary"${state.busy?' disabled':''}>저장 내용 미리보기</button></form>${preview}</div><div class="dialog-footer save-dialog-footer">${button('close-modal','검토로 돌아가기','quiet-button',state.busy)}<div class="button-row">${actions}</div></div>`;
  }

  function closeModal() {
    if (state.busy) return;
    state.modal = null; state.modalError = ''; delete dialog.dataset.modal;
    dialog.close(); state.restoreFocus=true; render();
  }

  function captureDialogFocus(target) {
    state.dialogFocus=target || doc.activeElement;
    state.dialogFocusAction=state.dialogFocus?.dataset?.action || null;
    state.dialogScroll={left: win.scrollX, top: win.scrollY, behavior: 'instant'};
  }

  function rememberSettings(result, fallback) {
    state.settings=result.settings || fallback;
    state.settingsRevision=result.revision;
    if (state.workspace && Number.isSafeInteger(result.revision) && result.revision >= (state.workspace.settings_revision || 0)) {
      state.workspace={...state.workspace,settings:state.settings,settings_revision:result.revision};
    }
  }

  async function openSettings(target, {focusTimeout = false} = {}) {
    captureDialogFocus(target);
    await command(async()=> {
      const result = await api(`${API_ROOT}settings`);
      rememberSettings(result);
      state.diagnostics=result.diagnostics;
      state.choices=result.choices || {};
      state.modal='settings';
    });
    if (focusTimeout && state.modal === 'settings' && dialog.open && !state.connectionBlocked) $('timeout-seconds')?.focus();
  }

  async function approve() {
    const result = await api(sessionPath('/review'), {method:'POST',body:envelope({action:'approve',draft_id:draft().id})});
    adoptResult(result);
    state.notice='이 버전의 내용을 검토본으로 확정했어요. 파일 저장은 별도예요.';
  }

  async function previewExport(directory) {
    const value = directory.trim();
    if (value && !safeRelativeDirectory(value)) throw new ApiError(400,{error:{code:'INVALID_PATH',message:'프로젝트 안의 상대 폴더를 적어 주세요. 절대 경로나 상위 폴더는 사용할 수 없어요.'}});
    const result = await api(sessionPath('/exports'),{method:'POST',body:envelope({mode:'preview',draft_id:draft().id,...(value?{directory:value}:{})})});
    if (result.session) adoptResult(result,{reset:false});
    state.plan=result.plan || result;
    delete dialog.dataset.modal;
  }

  async function applyExport(withApproval) {
    if (!state.plan?.id && !state.plan?.plan_id) throw new Error('저장 미리보기가 필요해요.');
    if (withApproval) {
      const directory = state.plan.relative_directory || '';
      await approve();
      // Approval changes the session revision, so obtain a new bound plan.
      await previewExport(directory);
    }
    const planId=state.plan.id || state.plan.plan_id;
    let result;
    try {
      result = await api(sessionPath('/exports'),{method:'POST',body:envelope({mode:'apply',plan_id:planId})});
    } catch (error) {
      // An interrupted response may have committed. Inspect that exact export
      // before allowing another explicit save; do not infer success from a POST.
      let entry;
      try { entry=await api(sessionPath(`/exports/${idPath(planId)}`)); } catch {}
      if (entry?.status==='completed') result={export:entry};
      else { if (entry) state.lastExport=entry; throw error; }
    }
    if (result.session) adoptResult(result,{reset:false});
    if (!result.export) throw new Error('저장 완료 기록을 확인하지 못했어요.');
    if (result.export.status !== 'completed' || result.export.result?.status !== 'completed') {
      state.lastExport=result.export; state.plan=null; delete dialog.dataset.modal;
      throw new ApiError(409,{error:{code:result.export.status==='conflict'?'file_conflict':'export_failed',message:result.export.result?.error || '저장을 완료하지 못했어요. 새 미리보기를 확인해 주세요.'}});
    }
    state.saved=result.export.result; state.lastExport=result.export; state.scene='saved'; state.modal=null; state.plan=null; state.restoreFocus=true;
    delete dialog.dataset.modal; dialog.close(); announce('설계 문서를 저장했어요.');
    await refresh({keep:true});
    if(!state.connectionBlocked)void fileWorkspace?.load();
  }

  function newSession() {
    if (state.busy || state.connectionBlocked) return;
    fileWorkspace?.showChat();
    if(win.matchMedia?.('(max-width:1030px)').matches){state.leftCollapsed=true;renderPanels();}
    state.requestEpoch++; state.feed?.abort(); state.feed=null;
    state.session=null; state.run=null; state.scene='welcome'; state.selectedDraft=null; state.selectedDocument='architecture'; state.documentAnchor=null;
    state.error=null; state.notice=''; state.cursor=''; state.answer=''; state.saved=null; state.lastExport=null;
    state.intro=''; state.contextPaths.clear();
    render(); $('project-intro')?.focus();
  }

  function currentReadingContext() {
    if(fileWorkspace?.active!=='chat'&&fileWorkspace)return fileWorkspace.context;
    const scene=state.scene==='generating'?state.runScene:state.scene;
    if(scene!=='review'||!draft())return null;
    const document=selectedDocument(),container=doc.querySelector?.('[data-document-preview]');
    return document&&container?{file:document.filename,name:document.title,source:'draft',container}:null;
  }

  function renderRecorderControls() {
    if(!documentRecorder||state.disposed)return;
    const recording=documentRecorder.state,context=currentReadingContext();
    const reading=context?`${context.source}:${context.file}`:'';
    if(reading!==lastReading){lastReading=reading;if(context)documentRecorder.note('open',context);}
    const active=['starting','recording','stopping','uploading'].includes(recording.phase);
    $('document-recorder').hidden=!context&&!active&&!recording.result&&!recording.error;
    $('record-toggle').textContent=recording.phase==='recording'?'녹음 중지':recording.phase==='starting'?'마이크 준비 중…':recording.phase==='uploading'?'음성 전사 중…':'녹음 시작';
    $('record-toggle').disabled=state.connectionBlocked||(!context&&recording.phase!=='recording')||['starting','stopping','uploading'].includes(recording.phase);
    const speechCount=list(recording.result?.timeline).filter(item=>item.kind==='speech').length;
    const completedLabel=speechCount?(recording.result?.untimedSpeech?'발화와 조작을 시간순으로 정리하고 전체 전사 원문도 포함했어요.':'발화와 문서 조작을 시간순으로 정리했어요.'):'전사문은 시간 정보 없이, 문서 조작은 시간순으로 정리했어요.';
    $('record-status').textContent=({idle:'문서를 보며 음성으로 지시해 주세요.',starting:'전사 설정과 마이크를 확인하고 있어요.',recording:'녹음 중',stopping:'녹음을 마무리하고 있어요.',uploading:'음성을 전사하고 있어요.',complete:completedLabel,error:'녹음을 확인해 주세요.'})[recording.phase]||'';
    const seconds=Math.floor(recording.elapsed/1000);$('record-time').textContent=`${String(Math.floor(seconds/60)).padStart(2,'0')}:${String(seconds%60).padStart(2,'0')}`;
    $('record-events').textContent=[speechCount?`발화 ${speechCount}개`:'',recording.events.length?`문서 참조 ${recording.events.length}개`:''].filter(Boolean).join(' · ');
    $('record-error').textContent=recording.error;$('record-error').hidden=!recording.error;
    $('record-copy').hidden=!recording.result;
  }

  async function ensureFileWorkspace() {
    if(!doc.querySelector?.('#file-tree')||!token||state.connectionBlocked||state.disposed||viewerPromise)return;
    const epoch=state.workspaceEpoch;
    viewerPromise=Promise.all([import('./file-viewer.js'),import('./document-recorder.js'),import('./viewer-menu.js')]).then(([files,recording,menus])=>{
      if(state.disposed||state.connectionBlocked||epoch!==state.workspaceEpoch)return;
      viewerMenu=menus.createViewerMenu({doc,win,announce});
      documentRecorder=recording.createDocumentRecorder({api,getContext:currentReadingContext,mediaDevices:win.navigator.mediaDevices,MediaRecorder:win.MediaRecorder,now:()=>win.performance.now(),onChange:renderRecorderControls,onResult:result=>{if(!state.connectionBlocked&&!state.disposed)void viewerMenu.copy(result.instruction,'음성 지시문');}});
      fileWorkspace=files.createFileViewer({doc,win,api,menu:viewerMenu,announce,onActive:(path,{changed}={})=>{
        if(state.disposed)return;
        state.fileTabActive=path!=='chat';
        if(changed&&path==='chat')render();
        if(changed&&path!=='chat'&&win.matchMedia?.('(max-width:1030px)').matches){state.leftCollapsed=true;renderPanels();}
        renderRecorderControls();
      }});
      $('record-toggle').addEventListener('click',()=>{if(state.connectionBlocked)return;if(documentRecorder.state.phase==='recording')documentRecorder.stop();else void documentRecorder.start();});
      $('record-copy').addEventListener('click',()=>{if(documentRecorder.state.result)void viewerMenu.copy(documentRecorder.state.result.instruction,'음성 지시문');});
      $('recording-settings').addEventListener('click',()=>void openTranscriptionSettings());
      const contextAt=target=>{
        const context=currentReadingContext();if(!context?.container||!context.container.contains(target))return null;
        const element=target.nodeType===1?target:target.parentElement;let heading=element?.closest?.('h1,h2,h3,h4,h5,h6');
        if(!heading)for(const candidate of context.container.querySelectorAll('h1,h2,h3,h4,h5,h6')){if(candidate.compareDocumentPosition(element)&4)heading=candidate;}
        return {...context,heading:heading?.textContent?.trim()||'',anchor:heading?.dataset.documentAnchor||''};
      };
      const rangeContext=(context,range)=>{
        const pre=context?.kind==='text'?context.container.querySelector('pre'):null;
        if(!pre||!range||!pre.contains(range.startContainer)||!pre.contains(range.endContainer))return context;
        const prefix=doc.createRange();prefix.selectNodeContents(pre);prefix.setEnd(range.startContainer,range.startOffset);const start=prefix.toString().length;prefix.setEnd(range.endContainer,range.endOffset);
        return {...context,...recording.lineRangeFromOffsets(pre.textContent,start,prefix.toString().length)};
      };
      const selection=type=>{if(documentRecorder.state.phase!=='recording'||doc.activeElement?.closest?.('dialog'))return;const selected=win.getSelection();if(!selected||selected.isCollapsed)return;const context=contextAt(selected.anchorNode);if(context&&context.container.contains(selected.focusNode))documentRecorder.note(type,rangeContext(context,selected.getRangeAt(0)),selected.toString());};
      doc.addEventListener('click',event=>{if(documentRecorder.state.phase!=='recording'||!win.getSelection()?.isCollapsed)return;const context=contextAt(event.target);if(context)documentRecorder.note('focus',rangeContext(context,doc.caretRangeFromPoint?.(event.clientX,event.clientY)),event.target.closest?.('p,li,td,pre,h1,h2,h3,h4,h5,h6')?.textContent?.slice(0,300)||'');});
      doc.addEventListener('mouseup',event=>{if(currentReadingContext()?.container?.contains(event.target))selection('selection');});doc.addEventListener('keyup',event=>{if(event.key==='Shift'||event.shiftKey)selection('selection');});doc.addEventListener('copy',()=>selection('copy'));
      renderRecorderControls();void fileWorkspace.load();
    }).catch(()=>{if(!state.connectionBlocked&&!state.disposed)$('file-tree').textContent='파일 보기를 준비하지 못했어요. 화면을 다시 열어 주세요.';}).finally(()=>{if(!fileWorkspace)viewerPromise=null;});
    return viewerPromise;
  }

  function clearTranscriptionDialog() {
    transcriptionEpoch++;transcriptionRequest?.abort();
    if(!transcriptionDialog)return;
    for(const input of transcriptionDialog.querySelectorAll('input')){input.value='';input.checked=false;}
    transcriptionDialog.querySelector('[data-transcription-message]').textContent='';
    if(transcriptionDialog.open)transcriptionDialog.close();
  }

  async function openTranscriptionSettings() {
    if(state.connectionBlocked||!documentRecorder)return;
    if(!transcriptionDialog){
      transcriptionDialog=doc.createElement('dialog');transcriptionDialog.id='transcription-dialog';transcriptionDialog.setAttribute('aria-labelledby','transcription-title');
      transcriptionDialog.innerHTML='<div class="dialog-header"><div><p class="eyebrow">음성 지시</p><h2 id="transcription-title">음성 전사 설정</h2></div><button type="button" class="dialog-close" data-transcription-close aria-label="음성 설정 닫기">×</button></div><div class="dialog-body"><p>OpenAI API 키는 서버의 별도 공용 보관함에 저장돼요. 이 화면과 프로젝트 기록에 키를 남기지 않아요. 녹음 전 키 설정 여부를 먼저 확인해요.</p><p data-transcription-message role="status"></p><form id="transcription-form"><fieldset><div class="form-field"><label for="transcription-key">OpenAI API 키</label><input id="transcription-key" name="api_key" type="password" autocomplete="off" spellcheck="false" placeholder="새 키를 입력하거나 비워 두어 기존 키 유지"><small>저장된 키는 다시 표시하지 않아요.</small></div><label class="confirm-row"><input id="transcription-clear-key" type="checkbox">서버에 저장한 키 삭제</label><div class="form-field"><label for="transcription-model">음성 전사 모델</label><input id="transcription-model" name="model" list="transcription-model-options" required maxlength="120" autocomplete="off"><datalist id="transcription-model-options"><option value="whisper-1">Whisper 1 · 발화 시간 지원</option><option value="gpt-transcribe">GPT Transcribe · 전체 전사문</option></datalist><small>발화 시간 동기화에는 whisper-1을 선택하세요. 시간 정보를 제공하지 않는 모델의 전사문은 조작 시각과 따로 표시해요. 저장한 모델 선택은 유지돼요.</small></div><div class="dialog-footer"><button type="button" class="quiet-button" data-transcription-close>취소</button><button type="submit" class="primary">전사 설정 저장</button></div></fieldset></form></div>';
      doc.body.append(transcriptionDialog);
      transcriptionDialog.addEventListener('click',event=>{if(event.target.closest('[data-transcription-close]'))clearTranscriptionDialog();});
      transcriptionDialog.addEventListener('cancel',event=>{event.preventDefault();clearTranscriptionDialog();});
      transcriptionDialog.querySelector('form').addEventListener('submit',event=>{
        event.preventDefault();const form=event.target;if(!form.reportValidity()||form.querySelector('fieldset').disabled)return;
        const epoch=transcriptionEpoch,input=$('transcription-key'),payload={model:$('transcription-model').value.trim()};
        if($('transcription-clear-key').checked)payload.api_key=null;else if(input.value.trim())payload.api_key=input.value.trim();
        input.value='';form.querySelector('fieldset').disabled=true;transcriptionRequest=new AbortController();
        void api('/api/v1/transcription/key',{method:'PATCH',body:payload,signal:transcriptionRequest.signal}).then(()=>{if(epoch===transcriptionEpoch)clearTranscriptionDialog();}).catch(error=>{if(epoch===transcriptionEpoch)transcriptionDialog.querySelector('[data-transcription-message]').textContent=error.message;}).finally(()=>{payload.api_key=null;if(epoch===transcriptionEpoch)form.querySelector('fieldset').disabled=false;});
      });
    }
    clearTranscriptionDialog();const epoch=transcriptionEpoch;
    const form=transcriptionDialog.querySelector('form'),message=transcriptionDialog.querySelector('[data-transcription-message]');form.querySelector('fieldset').disabled=true;message.textContent='전사 설정을 확인하고 있어요…';transcriptionDialog.showModal();transcriptionRequest=new AbortController();
    try{const result=await api('/api/v1/transcription/settings',{signal:transcriptionRequest.signal});if(epoch!==transcriptionEpoch||!transcriptionDialog.open)return;$('transcription-model').value=result.model||'';message.textContent=(result.configured?(result.key_source==='environment'?'환경 변수의 키를 사용하고 있어요.':'서버에 저장된 키를 사용하고 있어요.'):'아직 OpenAI API 키가 없어요.')+' '+(result.timing_supported?'저장된 모델은 발화 시간 동기화를 지원해요.':'저장된 모델은 시간 정보 없는 전체 전사문으로 처리해요.');form.querySelector('fieldset').disabled=false;$('transcription-key').focus();}
    catch(error){if(epoch===transcriptionEpoch)message.textContent=error.message;}
  }

  doc.addEventListener('click', event => {
    if (state.connectionBlocked) return;
    if (inDocumentPreview(event.target)) return;
    const target = event.target.closest('button'); if (!target) return;
    if (target.dataset.session) { void selectSession(target.dataset.session); return; }
    if (target.dataset.panel) { state.panel=target.dataset.panel; renderAux(); return; }
    const action=target.dataset.action;
    if (action==='inspect-document') { selectDocument(target.dataset.documentKind); return; }
    if (action==='settings') { void openSettings(target); return; }
    if (action==='timeout-settings') { void openSettings(target,{focusTimeout:true}); return; }
    if (action==='new-session') { newSession(); return; }
    if (action==='close-modal') { closeModal(); return; }
    if (action==='toggle-sidebar') {
      state.leftCollapsed=!state.leftCollapsed;
      if(!state.leftCollapsed&&win.matchMedia?.('(max-width:1030px)').matches)state.rightCollapsed=true;
      renderPanels();return;
    }
    if (action==='toggle-info') {
      state.rightCollapsed=!state.rightCollapsed;
      if(!state.rightCollapsed&&win.matchMedia?.('(max-width:1030px)').matches)state.leftCollapsed=true;
      renderPanels();return;
    }
    if (action==='interview' || action==='review' || action==='inspect-draft') {
      const scene=action==='interview'?'interview':'review';
      if (runIsActive(state.run)) { state.runScene=scene; state.scene='generating'; }
      else state.scene=scene;
      state.notice=''; render(); return;
    }
    if (action==='quick-answer') {
      if (disabled()) return;
      state.answer=target.dataset.answer; render(); $('answer-form').requestSubmit(); return;
    }
    if (action==='diagnostic-status') { state.scene='diagnostic'; render(); return; }
    if (action==='refresh') { void command(()=>refresh({initial:!state.workspace,keep:false})); return; }
    if (action==='retry-conversation') {
      if (!canRetryConversation() || disabled()) return;
      void command(async()=> {
        adoptResult(await api(sessionPath('/conversation/retry'),{method:'POST',body:envelope({})}));
        announce('같은 사용자 입력으로 답변을 다시 받고 있어요.');
      });
      return;
    }
    if (action==='generate' || action==='restart-design') { if (disabled() || (action==='restart-design' && (!canRetryDesign() || !designGeneration()))) return; void command(async()=> {
      const result=await api(sessionPath('/draft'),{method:'POST',body:envelope(action==='restart-design'?{restart:true}:{})});
      if (draft() && ['error','paused','recovery'].includes(state.scene)) state.scene='review';
      adoptResult(result); announce('설계 정리를 시작했어요.');
    }); return; }
    if (action==='critic') { void command(async()=> {
      adoptResult(await api(sessionPath('/critic'),{method:'POST',body:envelope({})}));
    }); return; }
    if (action==='cancel') { void command(async()=> {
      const result=await api(`${API_ROOT}runs/${idPath(state.run.id)}/cancel`,{method:'POST',body:envelope({})});
      adoptResult(result);
      await refresh({keep:false});
    }); return; }
    if (action==='approve') { void command(approve); return; }
    if (action==='finding-decision' || action==='defer-coverage') {
      captureDialogFocus(target); state.modal=action==='finding-decision'?'finding':'coverage';
      state.findingId=target.dataset.finding; state.coverageId=target.dataset.coverage;
      state.modalError=''; render(); return;
    }
    if (action==='confirm-claim') { void command(async()=> {
      adoptResult(await api(sessionPath('/review'),{method:'POST',body:envelope({action:'confirm_claim',claim_id:target.dataset.claim})}));
      await refresh({keep:true});
    }); return; }
    if (action==='save-preview') { captureDialogFocus(target); state.modal='save'; state.plan=null; state.modalError=''; render(); void command(()=>previewExport(''),{modal:true}); return; }
    if (action==='export-apply' || action==='approve-export') { void command(()=>applyExport(action==='approve-export'),{modal:true}); return; }
    if (action==='diagnose') { void command(async()=> {
      let responseError;
      try {
        const result=await api(`${API_ROOT}settings/diagnostics`,{method:'POST',body:envelope({},state.settingsRevision)});
        state.diagnostics=result.diagnostics || [];
        if (result.error) responseError=new ApiError(400,result);
        else if (state.diagnostics.some(item=>item.termination_unknown)) responseError=new ApiError(400,{error:{code:'process_termination_unknown',message:'도구 진단의 종료를 확인하지 못했어요. 이 창을 닫고 수동 종료 확인을 진행해 주세요.'}});
        $('cli-diagnostics').innerHTML=diagnosticsHTML();
      } catch (error) { responseError=error; }
      // Diagnostics can create a durable process gate even before any session
      // exists. Reconcile that global run after every outcome, including errors.
      try { await refresh({keep:true}); } catch (error) { responseError ||= error; }
      if (responseError) throw responseError;
    },{modal:true}); return; }
    if (action==='confirm-diagnostic-termination') {
      if (!$('diagnostic-termination-confirm')?.checked) { state.error={message:'터미널에서 진단의 로컬 도구를 종료한 뒤 확인란을 선택해 주세요.',code:'CONFIRMATION_REQUIRED'}; render(); return; }
      const run=globalRun()?.kind==='diagnostic'?globalRun():state.run;
      void command(async()=> {
        let revision=state.workspace.workspace_revision;
        if (run.session_id) {
          const session=await api(`${API_ROOT}sessions/${idPath(run.session_id)}`);
          revision=(session.session || session).revision;
        }
        const result=await api(`${API_ROOT}runs/${idPath(run.id)}/recover`,{method:'POST',body:envelope({confirmed:true},revision)});
        if (result.session?.id===state.session?.id) state.session=result.session;
        state.notice='진단 프로세스의 수동 종료를 기록했어요. 새 작업을 시작할 수 있어요.';
        await refresh({keep:false});
      });
      return;
    }
    if (action==='confirm-termination') {
      if (!$('termination-confirm')?.checked) { state.error={message:'터미널에서 이전 로컬 도구를 종료한 뒤 확인란을 선택해 주세요.',code:'CONFIRMATION_REQUIRED'}; render(); return; }
      void command(async()=> {
        adoptResult(await api(`${API_ROOT}runs/${idPath(state.run.id)}/recover`,{method:'POST',body:envelope({confirmed:true})}));
        await refresh({keep:false});
      });
    }
  });

  doc.addEventListener('submit', event => {
    if (state.connectionBlocked) { event.preventDefault(); return; }
    if (inDocumentPreview(event.target)) return;
    if (!['start-form','answer-form','settings-form','save-form','finding-form','coverage-form'].includes(event.target.id)) return;
    event.preventDefault(); const form=event.target;
    if (!form.reportValidity()) return;
    if (form.id==='start-form') {
      const intro=$('project-intro').value.trim();
      if (!intro) { $('project-intro').setCustomValidity('만들고 싶은 서비스를 한두 문장으로 들려주세요.'); $('project-intro').reportValidity(); return; }
      const paths=Array.from(form.querySelectorAll('[name="context-path"]:checked'),el=>el.value);
      void command(async()=> {
        const result=await api(`${API_ROOT}sessions`,{method:'POST',body:envelope({title:state.workspace.project_name,intro,context_paths:paths},state.workspace.workspace_revision)});
        state.cursor=''; adoptResult(result); state.intro=''; await refresh({keep:false});
      });
    } else if (form.id==='answer-form') {
      const content=$('answer-input').value.trim();
      if (!content) { $('answer-input').setCustomValidity('답변이나 수정하고 싶은 내용을 적어 주세요.'); $('answer-input').reportValidity(); return; }
      void command(async()=> {
        const result=await api(sessionPath('/messages'),{method:'POST',body:envelope({content})});
        state.answer=''; adoptResult(result); await refresh({keep:false});
      });
    } else if (form.id==='save-form') {
      void command(()=>previewExport($('output-directory').value),{modal:true});
    } else if (form.id==='finding-form' || form.id==='coverage-form') {
      const values=Object.fromEntries(new FormData(form));
      const payload=form.id==='finding-form'?
        {action:'resolve_finding',finding_id:state.findingId,disposition:values.disposition,reason:values.reason.trim()}:
        {action:'defer_coverage',coverage_id:state.coverageId,reason:values.reason.trim(),owner:values.owner.trim(),follow_up:values.follow_up.trim()};
      void command(async()=> {
        adoptResult(await api(sessionPath('/review'),{method:'POST',body:envelope(payload)}),{reset:false});
        state.modal=null; delete dialog.dataset.modal; dialog.close(); state.restoreFocus=true;
        await refresh({keep:true});
      },{modal:true});
    } else {
      const values = Object.fromEntries(new FormData(form));
      void command(async()=> {
        const settings = settingsFromValues(state.settings, values);
        const result=await api(`${API_ROOT}settings`,{method:'PATCH',body:envelope({settings},state.settingsRevision)});
        rememberSettings(result, settings);
        state.modal=null; delete dialog.dataset.modal; dialog.close(); state.notice='도구 설정을 저장했어요. 다음 생성부터 적용돼요.';
        state.restoreFocus=true;
      },{modal:true});
    }
  });

  doc.addEventListener('input',event=> {
    if (inDocumentPreview(event.target)) return;
    if (state.modal==='settings' && /^(claude|codex|conversation|design|critic)-(model|effort)$/.test(event.target.id)) {
      const choice=$(`${event.target.id}-choice`), value=event.target.value.trim();
      choice.value=Array.from(choice.options).some(option=>option.value===value)?value:'';
    }
    if (event.target.id==='answer-input') { state.answer=event.target.value; event.target.setCustomValidity(''); }
    if (event.target.id==='project-intro') { state.intro=event.target.value; event.target.setCustomValidity(''); }
    if (event.target.id==='output-directory' && state.plan) {
      state.plan=null;
      for (const button of dialog.querySelectorAll('[data-action="export-apply"],[data-action="approve-export"]')) button.disabled=true;
    }
  });
  doc.addEventListener('change',event=> {
    if (inDocumentPreview(event.target)) return;
    if (state.modal==='settings') {
      if (event.target.dataset.choiceInput && event.target.value) $(event.target.dataset.choiceInput).value=event.target.value;
      if (/^(default|conversation|design|critic)-backend$/.test(event.target.id)) refreshSettingChoices();
    }
    if (event.target.name==='context-path') event.target.checked?state.contextPaths.add(event.target.value):state.contextPaths.delete(event.target.value);
    if (event.target.id==='session-switcher') event.target.value?void selectSession(event.target.value):newSession();
    if (event.target.id==='draft-picker') { state.selectedDraft=event.target.value; state.documentAnchor=''; selectedDocument(); render(); $('draft-picker')?.focus({preventScroll:true}); }
    if (event.target.id==='document-kind-picker') { selectDocument(event.target.value); $('document-kind-picker')?.focus({preventScroll:true}); }

  });
  doc.addEventListener('keydown',event=> {
    if (inDocumentPreview(event.target)) return;
    if (event.target.id==='answer-input' && event.key==='Enter' && !event.shiftKey && !event.isComposing) { event.preventDefault(); event.target.form.requestSubmit(); }
    if (event.target.matches('.aux-tab') && ['ArrowLeft','ArrowRight','Home','End'].includes(event.key)) {
      event.preventDefault(); state.panel=event.key==='Home'?'notes':event.key==='End'?'documents':state.panel==='notes'?'documents':'notes'; renderAux(); $(state.panel+'-tab').focus();
    }
  });
  dialog.addEventListener('cancel',event=> { event.preventDefault(); closeModal(); });
  win.addEventListener('hashchange',()=> {
    if (!new URLSearchParams(win.location.hash.replace(/^#/, '')).has('token')) return;
    // A fixed-port restart may change only the fragment, which does not create
    // a new page. Rebootstrap without letting the old connection keep its view.
    invalidateConnection(new ApiError(401,{error:{code:'connection_replaced',message:'새 서버에 연결하고 있어요.'}}));
    state.disposed = true;
    win.location.reload?.();
  });
  win.addEventListener('online',()=> { void refresh({keep:false}).catch(()=>connection('offline')); });
  win.addEventListener('pagehide',()=> { state.disposed=true;documentRecorder?.dispose();clearTranscriptionDialog();fileWorkspace?.dispose();viewerMenu?.dispose();state.feed?.abort();win.clearTimeout(state.diagnosticTimer);win.clearTimeout(state.refreshTimer); });

  if (!token) { state.scene='welcome'; render(); connection('offline'); }
  else void refresh({initial:true,keep:false}).then(()=>{if(!state.session && !state.connectionBlocked)connection('connected');}).catch(error=> { if (state.connectionBlocked) return; state.scene='welcome'; connection('offline'); showError(error); });
  return {state,refresh,api};
}

if (typeof document !== 'undefined' && typeof window !== 'undefined') startWorkspace();
