//! Single-writer application boundary. No DB transaction is held across AI work.
use crate::{
    ai,
    config::{self, Settings},
    design_package::{DesignDocument, DocumentKind, MAIN_FILENAME, package_hash},
    domain, files,
    model::*,
    storage::{EventInput, EventRecord, ReceiptInput, Store},
};
use anyhow::Context;
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};
use tokio::sync::{broadcast, mpsc};
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone)]
pub struct AppError {
    pub code: String,
    pub message: String,
    pub snapshot: Option<Value>,
}
impl AppError {
    pub fn new(code: &str, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            snapshot: None,
        }
    }
}
impl From<anyhow::Error> for AppError {
    fn from(e: anyhow::Error) -> Self {
        if e.is::<crate::storage::WorkspaceResetRequired>() {
            return Self::new(crate::storage::RESET_CODE, crate::storage::RESET_MESSAGE);
        }
        tracing::error!(error=%e,"application operation failed");
        Self::new("operation_failed", e.to_string())
    }
}
type Result<T> = std::result::Result<T, AppError>;
struct Inner {
    state: Workspace,
    store: Store,
    cancellations: HashMap<String, CancellationToken>,
    shared_ai_hash: Option<String>,
}
#[derive(Debug, Clone)]
pub struct LiveEvent {
    pub session_id: String,
    pub run_id: Option<String>,
    pub kind: String,
    pub payload: Value,
}
pub struct App {
    pub root: PathBuf,
    workers: AtomicUsize,
    files: files::ProjectFiles,
    inner: Mutex<Inner>,
    pub events: broadcast::Sender<LiveEvent>,
    pub shutdown: CancellationToken,
    diagnostic_gate: tokio::sync::Mutex<()>,
    trusted: Settings,
    shared_config: Option<PathBuf>,
}
struct Launch {
    run_id: String,
    session: Session,
    spec: ai::RunSpec,
    cancel: CancellationToken,
}

impl App {
    pub fn open(root: &Path, settings: Settings) -> anyhow::Result<Arc<Self>> {
        Self::open_internal(root, settings, None)
    }
    pub fn open_with_shared_config(root: &Path, path: &Path) -> anyhow::Result<Arc<Self>> {
        Self::open_internal(root, config::load(path)?, Some(path.to_owned()))
    }
    fn open_internal(
        root: &Path,
        settings: Settings,
        shared_config: Option<PathBuf>,
    ) -> anyhow::Result<Arc<Self>> {
        let mut store = Store::open(root)?;
        let project_files = store.project_files()?;
        let root = store.root().to_owned();
        let mut state: Workspace = match store.load_workspace()? {
            Some(v) => {
                serde_json::from_value(v).context("invalid workspace state; original retained")?
            }
            None => Workspace::new(settings.clone()),
        };
        let previous_ai = state.settings.ai.clone();
        // A stored initial snapshot is not an implicit override of later user config.
        if !state.settings_overridden && state.settings_revision == 0 {
            state.settings = settings.clone();
        }
        // Shared preferences are authoritative for AI settings; document paths
        // and all project/run history stay inside this workspace.
        if shared_config.is_some() {
            if previous_ai != settings.ai {
                state.settings_revision += 1;
            }
            state.settings.ai = settings.ai.clone();
        }
        let shared_ai_hash = shared_config
            .as_ref()
            .map(|_| config::ai_hash(&state.settings.ai))
            .transpose()?;
        // Executables/profile are always controlled by the current trusted user config.
        state.settings.ai.backends.claude.executable =
            settings.ai.backends.claude.executable.clone();
        state.settings.ai.backends.codex.executable = settings.ai.backends.codex.executable.clone();
        state.settings.ai.backends.claude.profile = settings.ai.backends.claude.profile.clone();
        state.settings.ai.backends.codex.profile = settings.ai.backends.codex.profile.clone();
        state.settings.validate()?;
        for r in &mut state.runs {
            if ["running", "cancelling", "queued"].contains(&r.status.as_str()) {
                r.status = "interrupted".into();
                r.process_termination = "unknown".into();
                r.error=Some(RunError{code:"interrupted".into(),message:"이전 실행의 종료를 확인한 뒤 수동 정리 확인을 선택하세요. AI 작업은 자동 재실행하지 않습니다.".into()});
            }
        }
        for entry in &mut state.exports {
            if ["applying", "preparing"].contains(&entry.status.as_str()) {
                let p: files::ExportPlan = serde_json::from_value(entry.plan.clone())?;
                if p.bundle_identity.is_some() {
                    let result = file_outcome(project_files.reconcile(&p));
                    entry.status = serde_json::to_value(&result.status)?
                        .as_str()
                        .unwrap_or("failed")
                        .to_owned();
                    entry.result = Some(serde_json::to_value(result)?);
                } else {
                    entry.status = "conflict".into();
                    entry.result = Some(
                        json!({"error":"저장 폴더의 소유 기록이 없어 자동 복구하지 않았습니다. 새 사본을 준비하세요."}),
                    );
                }
            }
        }
        store.commit_workspace(&serde_json::to_value(&state)?, None, None)?;
        let (events, _) = broadcast::channel(128);
        Ok(Arc::new(Self {
            root,
            workers: AtomicUsize::new(0),
            files: project_files,
            inner: Mutex::new(Inner {
                state,
                store,
                cancellations: HashMap::new(),
                shared_ai_hash,
            }),
            events,
            shutdown: CancellationToken::new(),
            diagnostic_gate: tokio::sync::Mutex::new(()),
            trusted: settings,
            shared_config,
        }))
    }
    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|p| p.into_inner())
    }
    fn checked_lock(&self) -> Result<std::sync::MutexGuard<'_, Inner>> {
        let mut inner = self.lock();
        if let Err(error) = inner.store.ensure_current() {
            for cancel in inner.cancellations.values() {
                cancel.cancel();
            }
            inner.cancellations.clear();
            inner.state.sessions.clear();
            inner.state.runs.clear();
            inner.state.exports.clear();
            return Err(error.into());
        }
        Ok(inner)
    }
    pub fn ensure_workspace_current(&self) -> Result<()> {
        drop(self.checked_lock()?);
        Ok(())
    }
    fn sync_shared_ai(&self, inner: &mut Inner) -> Result<()> {
        let Some(path) = &self.shared_config else {
            return Ok(());
        };
        let loaded = config::load(path)?;
        let fingerprint = config::ai_hash(&loaded.ai)?;
        if inner.shared_ai_hash.as_deref() != Some(&fingerprint) {
            inner.state.settings.ai = loaded.ai;
            inner.state.settings_revision += 1;
            inner.shared_ai_hash = Some(fingerprint);
        }
        Ok(())
    }
    pub fn workspace(&self) -> Result<Value> {
        let mut i = self.checked_lock()?;
        self.sync_shared_ai(&mut i)?;
        let scan = self.files.scan()?;
        let mut document_types =
            vec![json!({"kind":"architecture","filename":MAIN_FILENAME,"title":"아키텍처 설계"})];
        document_types.extend(
            DocumentKind::ALL
                .into_iter()
                .map(|kind| json!({"kind":kind,"filename":kind.filename(),"title":kind.title()})),
        );
        Ok(
            json!({"project_name":self.root.file_name().unwrap_or_default().to_string_lossy(),"root_display":self.root.display().to_string(),"sessions":i.state.sessions,"settings":i.state.settings,"settings_revision":i.state.settings_revision,"workspace_revision":i.state.workspace_revision,"context_files":scan.files,"document_types":document_types,"active_run":active_run(&i.state),"runs":i.state.runs,"exports":i.state.exports,"latest_run":i.state.runs.last(),"cursor":i.store.latest_cursor()?}),
        )
    }
    pub fn session(&self, id: &str) -> Result<Value> {
        let i = self.checked_lock()?;
        let s = find_session(&i.state, id)?;
        let mut v = serde_json::to_value(s)?;
        v["cursor"] = json!(i.store.latest_cursor()?);
        if let Some(d) = s.drafts.last() {
            v["readiness_issues"] = json!(domain::readiness(s, d));
        }
        Ok(v)
    }
    pub fn run(&self, id: &str) -> Result<Value> {
        let i = self.checked_lock()?;
        Ok(serde_json::to_value(
            i.state
                .runs
                .iter()
                .find(|r| r.id == id)
                .ok_or_else(|| AppError::new("not_found", "작업을 찾을 수 없습니다."))?,
        )?)
    }
    pub fn settings(&self) -> Result<Value> {
        let mut i = self.checked_lock()?;
        self.sync_shared_ai(&mut i)?;
        Ok(
            json!({"settings":i.state.settings,"revision":i.state.settings_revision,"diagnostics":[],"choices":{
                "claude":ai::Backend::Claude.choices(),
                "codex":ai::Backend::Codex.choices()
            }}),
        )
    }
    pub fn draft(&self, id: &str) -> Result<Value> {
        let i = self.checked_lock()?;
        let s = find_session(&i.state, id)?;
        Ok(
            json!({"drafts":s.drafts,"draft":s.drafts.last(),"findings":s.findings,"content_status":s.content_status,"readiness_issues":s.drafts.last().map(|d|domain::readiness(s,d)).unwrap_or_default()}),
        )
    }
    pub fn export(&self, sid: &str, eid: &str) -> Result<Value> {
        let i = self.checked_lock()?;
        Ok(serde_json::to_value(
            i.state
                .exports
                .iter()
                .find(|e| e.id == eid && e.session_id == sid)
                .ok_or_else(|| AppError::new("not_found", "저장 기록을 찾을 수 없습니다."))?,
        )?)
    }
    pub fn replay(&self, sid: Option<&str>, after: u64) -> Result<Vec<EventRecord>> {
        Ok(self.checked_lock()?.store.events_after(sid, after)?)
    }
    pub async fn diagnostics(&self) -> Result<Value> {
        let _gate = self.diagnostic_gate.lock().await;
        if self.shutdown.is_cancelled() {
            return Err(AppError::new("shutting_down", "종료 중입니다."));
        }
        // A help/version probe owns the same project process slot BEFORE spawn.
        let (settings, rid, sid) = {
            let mut i = self.checked_lock()?;
            self.sync_shared_ai(&mut i)?;
            check_idle(&i.state)?;
            let mut next = i.state.clone();
            let rid = id();
            let sid = next
                .sessions
                .last()
                .map(|s| s.id.clone())
                .unwrap_or_default();
            next.runs.push(Run {
                id: rid.clone(),
                session_id: sid.clone(),
                kind: "diagnostic".into(),
                status: "running".into(),
                input_revision: 0,
                created_at: now(),
                completed_at: None,
                effective_settings: json!({"kind":"help_only"}),
                error: None,
                process_termination: "pending".into(),
                progress: "설치·버전만 확인합니다. 모델은 호출하지 않습니다.".into(),
                resolved_model: None,
                cli_version: None,
            });
            if let Some(session) = next.sessions.iter_mut().find(|s| s.id == sid) {
                session.active_run_id = Some(rid.clone());
                session.revision += 1;
            }
            next.workspace_revision += 1;
            let event = i.store.commit_workspace(
                &serde_json::to_value(&next)?,
                None,
                Some(EventInput {
                    session_id: if sid.is_empty() {
                        None
                    } else {
                        Some(sid.clone())
                    },
                    kind: "diagnostic_started".into(),
                    payload: json!({"run_id":rid}),
                }),
            )?;
            let settings = next.settings.clone();
            i.state = next;
            drop(i);
            if let Some(e) = event {
                self.publish_record(e);
            }
            (settings, rid, sid)
        };
        let c = ai::diagnose_in_project(
            ai::Backend::Claude,
            &settings.ai.backends.claude.executable,
            &self.root,
        )
        .await;
        self.ensure_workspace_current()?;
        let results = if c.termination_unknown {
            vec![c]
        } else {
            let x = ai::diagnose_in_project(
                ai::Backend::Codex,
                &settings.ai.backends.codex.executable,
                &self.root,
            )
            .await;
            vec![c, x]
        };
        let unknown = results.iter().any(|d| d.termination_unknown);
        let mut i = self.checked_lock()?;
        let mut next = i.state.clone();
        let r = next
            .runs
            .iter_mut()
            .find(|r| r.id == rid)
            .ok_or_else(|| AppError::new("not_found", "진단 기록이 없습니다."))?;
        r.status = if unknown { "interrupted" } else { "completed" }.into();
        r.process_termination = if unknown { "unknown" } else { "confirmed" }.into();
        r.completed_at = Some(now());
        if unknown {
            r.error=Some(RunError{code:"process_termination_unknown".into(),message:"도구 진단의 프로세스 종료를 확인하지 못했습니다. 터미널에서 정리한 뒤 수동 확인하세요.".into()});
        }
        if let Some(session) = next.sessions.iter_mut().find(|s| s.id == sid) {
            if session.active_run_id.as_deref() == Some(&rid) && !unknown {
                session.active_run_id = None;
            }
            session.revision += 1;
        }
        next.workspace_revision += 1;
        let commit = i.store.commit_workspace(
            &serde_json::to_value(&next)?,
            None,
            Some(EventInput {
                session_id: if sid.is_empty() { None } else { Some(sid) },
                kind: "diagnostic_completed".into(),
                payload: json!({"run_id":rid,"termination_unknown":unknown}),
            }),
        );
        match commit {
            Ok(event) => {
                i.state = next;
                drop(i);
                if let Some(e) = event {
                    self.publish_record(e);
                }
            }
            Err(e) => {
                i.state = next;
                self.shutdown.cancel();
                return Err(e.into());
            }
        }
        Ok(json!({"diagnostics":results}))
    }

    pub fn command(
        self: &Arc<Self>,
        operation: &str,
        target: Option<&str>,
        command: Command,
    ) -> Result<Value> {
        if self.shutdown.is_cancelled() {
            return Err(AppError::new("shutting_down", "종료 중입니다."));
        }
        if command.command_id.is_empty()
            || command.command_id.len() > 100
            || command.command_id.chars().any(char::is_control)
        {
            return Err(AppError::new(
                "invalid_command",
                "명령 ID가 올바르지 않습니다.",
            ));
        }
        let payload_hash = hash(&serde_json::to_string(&json!([
            operation,
            target,
            command.payload
        ]))?);
        let mut i = self.checked_lock()?;
        self.sync_shared_ai(&mut i)?;
        if let Some(r) = i.store.find_receipt(&command.command_id)? {
            if r.payload_hash == payload_hash {
                return Ok(r.response);
            }
            return Err(AppError::new(
                "idempotency_conflict",
                "같은 명령 ID로 다른 요청을 보낼 수 없습니다.",
            ));
        }
        let mut next = i.state.clone();
        let mut launch = None;
        let mut event_sid = target.map(str::to_owned);
        let response = match operation {
            "create" => {
                check_revision(
                    command.expected_revision,
                    next.workspace_revision,
                    json!({"workspace_revision":next.workspace_revision}),
                )?;
                check_idle(&next)?;
                let title = input_string(&command.payload, "title", 120)?;
                let intro = input_string(&command.payload, "intro", 16_000)?;
                let paths: Vec<String> = serde_json::from_value(
                    command
                        .payload
                        .get("context_paths")
                        .cloned()
                        .unwrap_or(json!([])),
                )?;
                if paths.len() > 20 {
                    return Err(AppError::new(
                        "context_limit",
                        "참고 파일은 한 번에 20개까지 선택하세요.",
                    ));
                }
                let evidence = self
                    .files
                    .read_selected(&paths)?
                    .into_iter()
                    .map(|e| Evidence {
                        id: id(),
                        path: e.path,
                        hash: e.hash,
                        text: e.text,
                    })
                    .collect();
                let mut s = domain::new_session(title, intro, evidence);
                event_sid = Some(s.id.clone());
                next.workspace_revision += 1;
                let l = prepare_run(
                    &mut s,
                    &mut next.runs,
                    &next.settings,
                    "conversation",
                    &self.root,
                )?;
                let run = next.runs.last().cloned();
                let response = json!({"session":s,"run":run});
                next.sessions.push(s);
                launch = Some(l);
                response
            }
            "settings" => {
                check_revision(
                    command.expected_revision,
                    next.settings_revision,
                    json!({"settings_revision":next.settings_revision}),
                )?;
                let settings: Settings =
                    serde_json::from_value(command.payload.get("settings").cloned().ok_or_else(
                        || AppError::new("invalid_settings", "settings가 필요합니다."),
                    )?)?;
                let trusted = if let Some(path) = &self.shared_config {
                    config::load(path)?
                } else {
                    self.trusted.clone()
                };
                settings
                    .validate_web_change(&trusted)
                    .map_err(|e| AppError::new("invalid_settings", e.to_string()))?;
                if let Some(path) = &self.shared_config {
                    config::save_ai(
                        path,
                        &settings.ai,
                        i.shared_ai_hash.as_deref().unwrap_or_default(),
                    )
                    .map_err(|e| AppError::new("shared_settings_conflict", e.to_string()))?;
                }
                next.settings = settings;
                next.settings_overridden = true;
                next.settings_revision += 1;
                event_sid = None;
                json!({"settings":next.settings,"revision":next.settings_revision})
            }
            "cancel" | "recover" => {
                let rid = target.unwrap_or_default();
                let run = next
                    .runs
                    .iter()
                    .find(|r| r.id == rid)
                    .cloned()
                    .ok_or_else(|| AppError::new("not_found", "작업을 찾을 수 없습니다."))?;
                event_sid = Some(run.session_id.clone());
                if operation == "cancel" && run.kind == "diagnostic" {
                    return Err(AppError::new(
                        "diagnostic_running",
                        "설치 진단은 제한된 시간 안에 종료됩니다. 결과를 기다려 주세요.",
                    ));
                }
                let session_index = next.sessions.iter().position(|s| s.id == run.session_id);
                match session_index {
                    None => {
                        check_revision(
                            command.expected_revision,
                            next.workspace_revision,
                            json!({"workspace_revision":next.workspace_revision}),
                        )?;
                        if operation != "recover"
                            || run.process_termination != "unknown"
                            || command.payload.get("confirmed").and_then(Value::as_bool)
                                != Some(true)
                        {
                            return Err(AppError::new(
                                "invalid_recovery",
                                "진단 프로세스를 정리한 뒤 확인하세요.",
                            ));
                        }
                        let r = next.runs.iter_mut().find(|r| r.id == rid).unwrap();
                        r.process_termination = "manually_confirmed".into();
                        r.status = "interrupted".into();
                        next.workspace_revision += 1;
                        json!({"run":r,"session":null})
                    }
                    Some(session_index) => {
                        let s = &mut next.sessions[session_index];
                        check_revision(
                            command.expected_revision,
                            s.revision,
                            serde_json::to_value(&*s)?,
                        )?;
                        let r = next.runs.iter_mut().find(|r| r.id == rid).unwrap();
                        if operation == "recover" {
                            if r.process_termination != "unknown"
                                || command.payload.get("confirmed").and_then(Value::as_bool)
                                    != Some(true)
                            {
                                return Err(AppError::new(
                                    "invalid_recovery",
                                    "기존 프로세스 트리를 정리한 뒤 확인하세요.",
                                ));
                            }
                            r.process_termination = "manually_confirmed".into();
                            r.status = "interrupted".into();
                            s.active_run_id = None;
                        } else if ["running", "queued"].contains(&r.status.as_str()) {
                            r.status = "cancelling".into();
                            if let Some(token) = i.cancellations.get(rid) {
                                token.cancel();
                            } else {
                                r.process_termination = "unknown".into();
                            }
                        }
                        s.revision += 1;
                        json!({"run":r,"session":s})
                    }
                }
            }
            _ => {
                let sid = target.unwrap_or_default();
                let pos = next
                    .sessions
                    .iter()
                    .position(|s| s.id == sid)
                    .ok_or_else(|| AppError::new("not_found", "대화를 찾을 수 없습니다."))?;
                check_revision(
                    command.expected_revision,
                    next.sessions[pos].revision,
                    serde_json::to_value(&next.sessions[pos])?,
                )?;
                match operation {
                    "message" | "draft" | "critic" => {
                        check_idle(&next)?;
                        let s = &mut next.sessions[pos];
                        if operation == "message" {
                            domain::add_answer(
                                s,
                                input_string(&command.payload, "content", 16_000)?,
                            );
                        }
                        let kind = if operation == "message" {
                            "conversation"
                        } else if operation == "critic" {
                            "critic"
                        } else {
                            "design"
                        };
                        if kind == "critic" && s.drafts.is_empty() {
                            return Err(AppError::new("no_draft", "검토할 초안을 먼저 만드세요."));
                        }
                        let l = prepare_run(s, &mut next.runs, &next.settings, kind, &self.root)?;
                        let response = json!({"session":s,"run":next.runs.last()});
                        launch = Some(l);
                        response
                    }
                    "review" => {
                        check_idle(&next)?;
                        let s = &mut next.sessions[pos];
                        let action = command
                            .payload
                            .get("action")
                            .and_then(Value::as_str)
                            .unwrap_or("");
                        match action {
                            "approve" => {
                                let draft_id = input_string(&command.payload, "draft_id", 100)?;
                                let index =
                                    s.drafts.iter().position(|d| d.id == draft_id).ok_or_else(
                                        || AppError::new("not_found", "초안이 없습니다."),
                                    )?;
                                let issues = domain::readiness(s, &s.drafts[index]);
                                if !issues.is_empty() {
                                    return Err(AppError::new("not_ready", issues.join("\n")));
                                }
                                s.content_status = "approved".into();
                                s.drafts[index].content_status = "approved".into();
                            }
                            "confirm_claim" => {
                                let cid = input_string(&command.payload, "claim_id", 100)?;
                                let c =
                                    s.claims.iter_mut().find(|c| c.id == cid).ok_or_else(|| {
                                        AppError::new("not_found", "진술이 없습니다.")
                                    })?;
                                c.status = "confirmed".into();
                                s.design_revision += 1;
                                s.content_status = "draft".into();
                            }
                            "defer_coverage" | "exclude_coverage" => {
                                let cid = input_string(&command.payload, "coverage_id", 100)?;
                                let c = s.coverage.iter_mut().find(|c| c.id == cid).ok_or_else(
                                    || AppError::new("not_found", "확인 항목이 없습니다."),
                                )?;
                                c.reason = input_string(&command.payload, "reason", 2000)?;
                                if action == "exclude_coverage" {
                                    if c.blocker {
                                        return Err(AppError::new(
                                            "required_risk",
                                            "위험 신호의 적용 여부는 답변으로 확인해야 합니다.",
                                        ));
                                    }
                                    c.status = "not_applicable".into();
                                } else {
                                    c.owner = input_string(&command.payload, "owner", 200)?;
                                    c.follow_up =
                                        input_string(&command.payload, "follow_up", 2000)?;
                                    c.status = "deferred".into();
                                }
                                c.assessment_source = "user".into();
                                c.assessment_status = "user_confirmed".into();
                                s.design_revision += 1;
                                s.content_status = "draft".into();
                            }
                            "resolve_finding" => {
                                let fid = input_string(&command.payload, "finding_id", 100)?;
                                let disposition =
                                    input_string(&command.payload, "disposition", 50)?;
                                if !["accepted", "rejected", "deferred", "resolved"]
                                    .contains(&disposition.as_str())
                                {
                                    return Err(AppError::new(
                                        "invalid_disposition",
                                        "지적 처리 상태가 올바르지 않습니다.",
                                    ));
                                }
                                let reason = input_string(&command.payload, "reason", 4000)?;
                                let f = s.findings.iter_mut().find(|f| f.id == fid).ok_or_else(
                                    || AppError::new("not_found", "검토 지적이 없습니다."),
                                )?;
                                f.disposition = Some(disposition);
                                f.disposition_reason = Some(reason);
                            }
                            _ => {
                                return Err(AppError::new(
                                    "invalid_action",
                                    "지원하지 않는 검토 동작입니다.",
                                ));
                            }
                        }
                        if action == "resolve_finding" {
                            for index in 0..s.drafts.len() {
                                let blocked = !domain::readiness(s, &s.drafts[index]).is_empty();
                                if blocked && s.drafts[index].content_status == "approved" {
                                    s.drafts[index].content_status = "draft".into();
                                }
                            }
                            if let Some(d) = s.drafts.last() {
                                let ready = domain::readiness(s, d).is_empty();
                                s.content_status = if d.content_status == "approved" && ready {
                                    "approved"
                                } else if ready {
                                    "ready_for_review"
                                } else {
                                    "draft"
                                }
                                .into();
                            }
                        }
                        s.revision += 1;
                        json!({"session":s})
                    }
                    "export" => {
                        let mode = command
                            .payload
                            .get("mode")
                            .and_then(Value::as_str)
                            .unwrap_or("preview");
                        if mode == "preview" {
                            let did = input_string(&command.payload, "draft_id", 100)?;
                            let s = &next.sessions[pos];
                            let d = s.drafts.iter().find(|d| d.id == did).ok_or_else(|| {
                                AppError::new("not_found", "저장할 초안이 없습니다.")
                            })?;
                            let rendered = domain::render_export(s, d);
                            let documents: Vec<_> = d
                                .documents
                                .iter()
                                .map(|document| DesignDocument {
                                    kind: document.kind,
                                    markdown: domain::render_export_markdown(
                                        s,
                                        d,
                                        &document.markdown,
                                    ),
                                })
                                .collect();
                            let mut plan = self.files.preview_documents_in(
                                &next.settings.documents.directory,
                                &s.title,
                                &rendered,
                                &documents,
                                d.input_revision,
                                &s.id,
                            )?;
                            if let Some(dir) =
                                command.payload.get("directory").and_then(Value::as_str)
                            {
                                files::validate_relative_path(dir)?;
                                plan.relative_directory = dir.to_owned();
                            }
                            let entry = ExportEntry {
                                id: plan.id.clone(),
                                session_id: s.id.clone(),
                                document_revision: d.input_revision,
                                document_hash: plan.document_hash.clone(),
                                draft_id: d.id.clone(),
                                preview_revision: s.revision,
                                source_document_hash: d.hash.clone(),
                                status: "preview".into(),
                                plan: serde_json::to_value(&plan)?,
                                result: None,
                                created_at: now(),
                            };
                            let response = json!({"export":entry,"plan":plan});
                            next.exports.push(entry);
                            response
                        } else if mode == "apply" {
                            let pid = input_string(&command.payload, "plan_id", 100)?;
                            let index = next
                                .exports
                                .iter()
                                .position(|e| e.id == pid && e.session_id == sid)
                                .ok_or_else(|| {
                                    AppError::new("not_found", "저장 미리보기가 없습니다.")
                                })?;
                            if next.exports[index].status == "completed" {
                                json!({"export":next.exports[index],"session":next.sessions[pos]})
                            } else {
                                if next.exports[index].status == "preview"
                                    && (next.exports[index].preview_revision
                                        != next.sessions[pos].revision
                                        || !next.sessions[pos].drafts.iter().any(|d| {
                                            d.id == next.exports[index].draft_id
                                                && d.hash
                                                    == next.exports[index].source_document_hash
                                        }))
                                {
                                    return Err(AppError::new(
                                        "preview_stale",
                                        "미리보기 이후 대화나 검토 상태가 바뀌었습니다. 새 미리보기를 확인하세요.",
                                    ));
                                }
                                let mut plan: files::ExportPlan =
                                    serde_json::from_value(next.exports[index].plan.clone())?;
                                if next.exports[index].status == "preview" {
                                    plan = self.files.reserve(&plan).map_err(|e| {
                                        AppError::new("file_conflict", e.to_string())
                                    })?;
                                    next.exports[index].status = "applying".into();
                                    next.exports[index].plan = serde_json::to_value(&plan)?;
                                    i.store.commit_workspace(
                                        &serde_json::to_value(&next)?,
                                        None,
                                        Some(EventInput {
                                            session_id: Some(sid.into()),
                                            kind: "export_prepared".into(),
                                            payload: json!({"export_id":pid}),
                                        }),
                                    )?;
                                    i.state = next.clone();
                                }
                                let result = file_outcome(self.files.apply(&plan));
                                next.exports[index].status = serde_json::to_value(&result.status)?
                                    .as_str()
                                    .unwrap_or("failed")
                                    .to_owned();
                                next.exports[index].result = Some(serde_json::to_value(result)?);
                                next.sessions[pos].revision += 1;
                                json!({"export":next.exports[index],"session":next.sessions[pos]})
                            }
                        } else {
                            return Err(AppError::new(
                                "invalid_action",
                                "preview 또는 apply를 선택하세요.",
                            ));
                        }
                    }
                    _ => return Err(AppError::new("not_found", "지원하지 않는 명령입니다.")),
                }
            }
        };
        let receipt = ReceiptInput {
            command_id: command.command_id,
            payload_hash,
            response: response.clone(),
        };
        let event = i.store.commit_workspace(
            &serde_json::to_value(&next)?,
            Some(receipt),
            Some(EventInput {
                session_id: event_sid.clone(),
                kind: operation.into(),
                payload: json!({"session_id":event_sid}),
            }),
        )?;
        i.state = next;
        if operation == "settings" && self.shared_config.is_some() {
            i.shared_ai_hash = Some(config::ai_hash(&i.state.settings.ai)?);
        }
        if let Some(l) = &launch {
            i.cancellations.insert(l.run_id.clone(), l.cancel.clone());
        }
        drop(i);
        if let Some(e) = event {
            self.publish_record(e);
        }
        if let Some(l) = launch {
            self.spawn(l);
        }
        Ok(response)
    }
    fn publish_record(&self, e: EventRecord) {
        if self.ensure_workspace_current().is_err() {
            return;
        }
        let _ = self.events.send(LiveEvent {
            session_id: e.session_id.unwrap_or_default(),
            run_id: None,
            kind: e.kind,
            payload: json!({"event_id":e.cursor,"payload":e.payload}),
        });
    }
    fn spawn(self: &Arc<Self>, launch: Launch) {
        self.workers.fetch_add(1, Ordering::SeqCst);
        let observer = self.clone();
        let worker = self.clone();
        let rid = launch.run_id.clone();
        tokio::spawn(async move {
            let task = tokio::spawn(async move {
                worker.execute(launch).await;
            });
            if task.await.is_err() {
                tracing::error!(run_id=%rid,"generation worker panicked; completion gate retained");
                observer.fail_completion(
                    &rid,
                    "작업 태스크가 예기치 않게 종료됐습니다. 기존 실행을 확인한 뒤 복구하세요.",
                );
            }
            observer.workers.fetch_sub(1, Ordering::SeqCst);
        });
    }
    async fn execute(self: Arc<Self>, l: Launch) {
        if self.ensure_workspace_current().is_err() {
            return;
        }
        let monitor_done = CancellationToken::new();
        let _monitor_guard = monitor_done.clone().drop_guard();
        let monitor_app = Arc::downgrade(&self);
        let monitor_cancel = l.cancel.clone();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = monitor_done.cancelled() => break,
                    _ = tokio::time::sleep(std::time::Duration::from_millis(100)) => {
                        let Some(app) = monitor_app.upgrade() else { break; };
                        if app.ensure_workspace_current().is_err() {
                            monitor_cancel.cancel();
                            break;
                        }
                    }
                }
            }
        });
        let (tx, mut rx) = mpsc::channel(64);
        let app = self.clone();
        let sid = l.session.id.clone();
        let rid = l.run_id.clone();
        let forward = tokio::spawn(async move {
            while let Some(event) = rx.recv().await {
                if app.ensure_workspace_current().is_err() {
                    break;
                }
                let value = serde_json::to_value(event).unwrap_or(Value::Null);
                let kind = value
                    .get("type")
                    .and_then(Value::as_str)
                    .unwrap_or("progress")
                    .to_owned();
                let _ = app.events.send(LiveEvent {
                    session_id: sid.clone(),
                    run_id: Some(rid.clone()),
                    kind,
                    payload: value,
                });
            }
        });
        let kind = {
            let i = self.lock();
            i.state
                .runs
                .iter()
                .find(|r| r.id == l.run_id)
                .map(|r| r.kind.clone())
                .unwrap_or_default()
        };
        tracing::info!(run_id=%l.run_id,role=%kind,phase="ai_start","run phase");
        let spec = l.spec;
        let mut result = ai::generate(spec.clone(), l.cancel.clone(), tx.clone()).await;
        if self.ensure_workspace_current().is_err() {
            drop(tx);
            let _ = forward.await;
            return;
        }
        // Design validation failures must not trigger another paid generation.
        // Conversation/critic retain their one schema repair.
        if kind != "design" {
            if let Ok(out) = &result {
                if let Err(error) = validate_output(&l.session, &kind, &out.text) {
                    let mut repair = spec.clone();
                    repair.prompt = format!(
                        "{}\nYour previous final output failed validation: {}. Return the complete corrected JSON only. Do not change unknown facts.\nPREVIOUS_OUTPUT:\n{}",
                        repair.prompt,
                        error,
                        out.text.chars().take(24_000).collect::<String>()
                    );
                    result = ai::generate(repair, l.cancel.clone(), tx.clone()).await;
                }
            }
        }
        tracing::info!(run_id=%l.run_id,role=%kind,phase="ai_returned",success=result.is_ok(),"run phase");
        drop(tx);
        let _ = forward.await;
        tracing::info!(run_id=%l.run_id,phase="drain_complete","run phase");
        let outcome = self.complete(&l.run_id, &kind, result);
        if let Err(e) = outcome {
            tracing::error!(run_id=%l.run_id,error=%e.message,"run completion failed");
            self.fail_completion(&l.run_id, &e.message);
        } else {
            tracing::info!(run_id=%l.run_id,phase="committed","run phase");
        }
    }
    fn complete(
        self: &Arc<Self>,
        rid: &str,
        kind: &str,
        result: std::result::Result<ai::AiOutput, ai::AiError>,
    ) -> Result<()> {
        let mut i = self.checked_lock()?;
        let mut next = i.state.clone();
        let rpos = next
            .runs
            .iter()
            .position(|r| r.id == rid)
            .ok_or_else(|| AppError::new("not_found", "작업이 없습니다."))?;
        let sid = next.runs[rpos].session_id.clone();
        let spos = next.sessions.iter().position(|s| s.id == sid).unwrap();
        let cancelled = next.runs[rpos].status == "cancelling";
        let stale = next.sessions[spos].design_revision != next.runs[rpos].input_revision;
        let mut launch = None;
        match result {
            Ok(out) if !cancelled && !stale => {
                let mut s = next.sessions[spos].clone();
                let apply = match kind {
                    "design" => domain::validate_design_package(&out.text).map(|package| {
                        let markdown =
                            format!("{}{}", package.markdown, domain::trace_appendix(&s));
                        let d = Draft {
                            id: id(),
                            input_revision: s.design_revision,
                            hash: package_hash(&markdown, &package.documents),
                            markdown,
                            documents: package.documents,
                            created_at: now(),
                            content_status: "draft".into(),
                            reviewed: false,
                        };
                        s.drafts.push(d);
                        s.content_status = "draft".into();
                    }),
                    "critic" => domain::apply_critic(&mut s, &out.text),
                    _ => domain::parse_json::<domain::ConversationCandidate>(&out.text)
                        .and_then(|c| domain::apply_conversation(&mut s, c)),
                };
                if let Err(e) = apply {
                    next.runs[rpos].status = "failed".into();
                    next.runs[rpos].error = Some(RunError {
                        code: "invalid_model_output".into(),
                        message: e.to_string(),
                    });
                } else {
                    next.sessions[spos] = s;
                    next.runs[rpos].status = "completed".into();
                }
                next.runs[rpos].resolved_model = out.resolved_model;
                next.runs[rpos].cli_version = Some(out.cli_version);
                next.runs[rpos].process_termination = "confirmed".into();
            }
            Ok(_) => {
                next.runs[rpos].status = if cancelled { "cancelled" } else { "stale" }.into();
                next.runs[rpos].process_termination = "confirmed".into();
            }
            Err(e) => {
                next.runs[rpos].status = if cancelled || e.code == "cancelled" {
                    "cancelled"
                } else {
                    "failed"
                }
                .into();
                next.runs[rpos].process_termination = if e.termination_unknown {
                    "unknown"
                } else {
                    "confirmed"
                }
                .into();
                next.runs[rpos].error = Some(RunError {
                    code: e.code,
                    message: e.message,
                });
            }
        }
        next.runs[rpos].completed_at = Some(now());
        next.sessions[spos].revision += 1;
        if next.runs[rpos].process_termination != "unknown"
            && next.sessions[spos].active_run_id.as_deref() == Some(rid)
        {
            next.sessions[spos].active_run_id = None;
        }
        if kind == "design"
            && next.runs[rpos].status == "completed"
            && !self.shutdown.is_cancelled()
            && active_run(&next).is_none()
        {
            let settings = next.settings.clone();
            tracing::info!(
                run_id = rid,
                phase = "before_critic_prepare",
                draft_bytes = next.sessions[spos]
                    .drafts
                    .last()
                    .map(|d| d.markdown.len()
                        + d.documents
                            .iter()
                            .map(|document| document.markdown.len())
                            .sum::<usize>())
                    .unwrap_or(0),
                "run phase"
            );
            match prepare_run(
                &mut next.sessions[spos],
                &mut next.runs,
                &settings,
                "critic",
                &self.root,
            ) {
                Ok(l) => launch = Some(l),
                Err(error) => {
                    let spec = settings.run_spec("critic", String::new(), self.root.clone());
                    next.runs.push(Run {
                        id: id(),
                        session_id: sid.clone(),
                        kind: "critic".into(),
                        status: "failed".into(),
                        input_revision: next.sessions[spos].design_revision,
                        created_at: now(),
                        completed_at: Some(now()),
                        effective_settings: domain::effective_summary(&spec),
                        error: Some(RunError {
                            code: "review_preparation_failed".into(),
                            message: error.message,
                        }),
                        process_termination: "confirmed".into(),
                        progress: "초안은 보존했지만 검토를 준비하지 못했습니다.".into(),
                        resolved_model: None,
                        cli_version: None,
                    });
                }
            }
            tracing::info!(run_id = rid, phase = "after_critic_prepare", "run phase");
        }
        let event = i.store.commit_workspace(
            &serde_json::to_value(&next)?,
            None,
            Some(EventInput {
                session_id: Some(sid.clone()),
                kind: "run_completed".into(),
                payload: json!({"run_id":rid,"status":next.runs[rpos].status}),
            }),
        )?;
        i.state = next;
        i.cancellations.remove(rid);
        if let Some(l) = &launch {
            i.cancellations.insert(l.run_id.clone(), l.cancel.clone());
        }
        drop(i);
        if let Some(e) = event {
            self.publish_record(e);
        }
        if let Some(l) = launch {
            self.spawn(l);
        }
        Ok(())
    }
    fn fail_completion(&self, rid: &str, message: &str) {
        let Ok(mut i) = self.checked_lock() else {
            return;
        };
        let mut next = i.state.clone();
        if let Some(r) = next.runs.iter_mut().find(|r| r.id == rid) {
            r.status = "failed".into();
            r.process_termination = "unknown".into();
            r.error = Some(RunError {
                code: "completion_failed".into(),
                message: format!(
                    "결과 확정에 실패했습니다. 종료와 저장 상태를 확인하세요: {message}"
                ),
            });
        }
        let result = i.store.commit_workspace(
            &serde_json::to_value(&next).unwrap_or(Value::Null),
            None,
            Some(EventInput {
                session_id: None,
                kind: "completion_failed".into(),
                payload: json!({"run_id":rid}),
            }),
        );
        i.state = next;
        i.cancellations.remove(rid);
        if result.is_err() {
            self.shutdown.cancel();
        }
    }
    pub fn stop(&self) {
        self.shutdown.cancel();
        let Ok(mut i) = self.checked_lock() else {
            return;
        };
        let mut next = i.state.clone();
        for r in &mut next.runs {
            if r.status == "running" {
                r.status = "cancelling".into();
            }
        }
        if i.store
            .commit_workspace(
                &serde_json::to_value(&next).unwrap_or(Value::Null),
                None,
                None,
            )
            .is_ok()
        {
            i.state = next;
        }
        for t in i.cancellations.values() {
            t.cancel();
        }
    }
    pub fn is_idle(&self) -> bool {
        self.workers.load(Ordering::SeqCst) == 0
            && self
                .checked_lock()
                .map_or(true, |inner| active_run(&inner.state).is_none())
    }
}
fn validate_output(s: &Session, kind: &str, text: &str) -> anyhow::Result<()> {
    let mut s = s.clone();
    match kind {
        "design" => domain::validate_design_package(text).map(|_| ()),
        "critic" => domain::apply_critic(&mut s, text),
        _ => domain::apply_conversation(&mut s, domain::parse_json(text)?),
    }
}
fn find_session<'a>(w: &'a Workspace, id: &str) -> Result<&'a Session> {
    w.sessions
        .iter()
        .find(|s| s.id == id)
        .ok_or_else(|| AppError::new("not_found", "대화를 찾을 수 없습니다."))
}
fn active_run(w: &Workspace) -> Option<&Run> {
    w.runs.iter().find(|r| {
        ["queued", "running", "cancelling"].contains(&r.status.as_str())
            || r.process_termination == "unknown"
    })
}
fn check_idle(w: &Workspace) -> Result<()> {
    if active_run(w).is_some() {
        Err(AppError::new(
            "run_busy",
            "진행 중이거나 종료 확인이 필요한 작업이 있습니다. 중단/종료 확인 뒤 다시 시도하세요.",
        ))
    } else {
        Ok(())
    }
}
fn check_revision(expected: u64, current: u64, snapshot: Value) -> Result<()> {
    if expected != current {
        Err(AppError {
            code: "revision_conflict".into(),
            message: "다른 변경이 반영됐습니다. 최신 내용을 확인한 뒤 다시 시도하세요.".into(),
            snapshot: Some(snapshot),
        })
    } else {
        Ok(())
    }
}
fn input_string(v: &Value, key: &str, max: usize) -> Result<String> {
    let text = v.get(key).and_then(Value::as_str).unwrap_or("").trim();
    if text.is_empty() || text.len() > max || text.contains('\0') {
        Err(AppError::new(
            "invalid_input",
            format!("{key} 필드를 확인하세요 (최대 {max} bytes)."),
        ))
    } else {
        Ok(text.into())
    }
}
fn prepare_run(
    s: &mut Session,
    runs: &mut Vec<Run>,
    settings: &Settings,
    kind: &str,
    root: &Path,
) -> Result<Launch> {
    let prompt = domain::prompt(s, kind, &settings.documents.language)?;
    let spec = settings.run_spec(kind, prompt, root.to_owned());
    let rid = id();
    s.active_run_id = Some(rid.clone());
    s.revision += 1;
    runs.push(Run {
        id: rid.clone(),
        session_id: s.id.clone(),
        kind: kind.into(),
        status: "running".into(),
        input_revision: s.design_revision,
        created_at: now(),
        completed_at: None,
        effective_settings: domain::effective_summary(&spec),
        error: None,
        process_termination: "pending".into(),
        progress: "AI CLI를 시작합니다.".into(),
        resolved_model: None,
        cli_version: None,
    });
    Ok(Launch {
        run_id: rid,
        session: s.clone(),
        spec,
        cancel: CancellationToken::new(),
    })
}
impl From<serde_json::Error> for AppError {
    fn from(e: serde_json::Error) -> Self {
        Self::new("invalid_json", e.to_string())
    }
}

fn file_outcome(result: anyhow::Result<files::ExportResult>) -> files::ExportResult {
    result.unwrap_or_else(|error| files::ExportResult {
        status: files::ExportStatus::Failed,
        paths: vec![],
        files: vec![],
        error: Some(error.to_string()),
    })
}
