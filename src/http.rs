use crate::{
    app::{App, AppError},
    model::Command,
};
use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, Path, Query, Request, State},
    http::{HeaderValue, StatusCode},
    middleware::{self, Next},
    response::{
        Html, IntoResponse, Response,
        sse::{Event, KeepAlive, Sse},
    },
    routing::{get, post},
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{convert::Infallible, sync::Arc, time::Duration};

#[derive(Clone)]
pub struct ServerState {
    pub app: Arc<App>,
    pub token: String,
    pub authority: String,
}
impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = match self.code.as_str() {
            "not_found" => StatusCode::NOT_FOUND,
            "workspace_reset_required" => StatusCode::GONE,
            "revision_conflict" | "idempotency_conflict" | "run_busy" | "file_conflict" => {
                StatusCode::CONFLICT
            }
            "unauthorized" => StatusCode::UNAUTHORIZED,
            "forbidden" => StatusCode::FORBIDDEN,
            "operation_failed" => StatusCode::INTERNAL_SERVER_ERROR,
            _ => StatusCode::BAD_REQUEST,
        };
        (
            status,
            Json(
                json!({"error":{"code":self.code,"message":self.message},"snapshot":self.snapshot}),
            ),
        )
            .into_response()
    }
}
pub fn router(state: ServerState) -> Router {
    let api = Router::new()
        .route("/workspace", get(workspace))
        .route("/sessions", post(create))
        .route("/sessions/{id}", get(session))
        .route("/sessions/{id}/messages", post(message))
        .route(
            "/sessions/{id}/conversation/retry",
            post(retry_conversation),
        )
        .route("/sessions/{id}/draft", get(draft).post(generate_draft))
        .route("/sessions/{id}/review", post(review))
        .route("/sessions/{id}/critic", post(critic))
        .route("/sessions/{id}/exports", post(export))
        .route("/sessions/{id}/exports/{eid}", get(export_status))
        .route("/sessions/{id}/events", get(session_events))
        .route("/runs/{id}", get(run))
        .route("/runs/{id}/cancel", post(cancel))
        .route("/runs/{id}/recover", post(recover))
        .route("/runs/{id}/events", get(run_events))
        .route("/settings", get(settings).patch(update_settings))
        .route("/settings/diagnostics", post(diagnostics))
        .layer(middleware::from_fn_with_state(state.clone(), authorize));
    Router::new()
        .route("/", get(index))
        .route("/favicon.ico", get(favicon))
        .route("/app.js", get(script))
        .route("/style.css", get(style))
        .route("/document-renderer.js", get(document_renderer))
        .route("/document.css", get(document_style))
        .route("/diagrams/{name}", get(diagram_asset))
        .route("/vendor/{name}", get(vendor_asset))
        .nest("/api/v1", api)
        .layer(DefaultBodyLimit::max(128 * 1024))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            security_headers,
        ))
        .with_state(state)
}
async fn index() -> Html<&'static str> {
    Html(include_str!("../web/index.html"))
}
async fn favicon() -> impl IntoResponse {
    (
        [("content-type", "image/x-icon")],
        include_bytes!("../web/favicon.ico").as_slice(),
    )
}
async fn script() -> impl IntoResponse {
    (
        [("content-type", "text/javascript; charset=utf-8")],
        include_str!("../web/app.js"),
    )
}
async fn style() -> impl IntoResponse {
    (
        [("content-type", "text/css; charset=utf-8")],
        include_str!("../web/style.css"),
    )
}
async fn document_renderer() -> impl IntoResponse {
    javascript(include_str!("../web/document-renderer.js"))
}
async fn document_style() -> impl IntoResponse {
    (
        [("content-type", "text/css; charset=utf-8")],
        include_str!("../web/document.css"),
    )
}
fn javascript(source: &'static str) -> Response {
    ([("content-type", "text/javascript; charset=utf-8")], source).into_response()
}
async fn diagram_asset(Path(name): Path<String>) -> Response {
    if name == "provenance.txt" {
        return (
            [("content-type", "text/plain; charset=utf-8")],
            include_str!("../web/diagrams/README.md"),
        )
            .into_response();
    }
    let source = match name.as_str() {
        "smartmd.js" => include_str!("../web/diagrams/smartmd.js"),
        "block-parser.js" => include_str!("../web/diagrams/block-parser.js"),
        "cjk-emphasis.js" => include_str!("../web/diagrams/cjk-emphasis.js"),
        "jobflow.js" => include_str!("../web/diagrams/jobflow.js"),
        "layout.js" => include_str!("../web/diagrams/layout.js"),
        "state.js" => include_str!("../web/diagrams/state.js"),
        "navigation.js" => include_str!("../web/diagrams/navigation.js"),
        _ => return StatusCode::NOT_FOUND.into_response(),
    };
    javascript(source)
}
async fn vendor_asset(Path(name): Path<String>) -> Response {
    let notice = match name.as_str() {
        "manifest.json" => Some((
            "application/json; charset=utf-8",
            include_str!("../web/vendor/manifest.json"),
        )),
        "THIRD-PARTY-NOTICES.txt" => Some((
            "text/plain; charset=utf-8",
            include_str!("../web/vendor/THIRD-PARTY-NOTICES.txt"),
        )),
        "marked.LICENSE.txt" => Some((
            "text/plain; charset=utf-8",
            include_str!("../web/vendor/marked.LICENSE.txt"),
        )),
        "dompurify.LICENSE.txt" => Some((
            "text/plain; charset=utf-8",
            include_str!("../web/vendor/dompurify.LICENSE.txt"),
        )),
        "mermaid.LICENSE.txt" => Some((
            "text/plain; charset=utf-8",
            include_str!("../web/vendor/mermaid.LICENSE.txt"),
        )),
        "dompurify.js.LEGAL.txt" => Some((
            "text/plain; charset=utf-8",
            include_str!("../web/vendor/dompurify.js.LEGAL.txt"),
        )),
        "mermaid.js.LEGAL.txt" => Some((
            "text/plain; charset=utf-8",
            include_str!("../web/vendor/mermaid.js.LEGAL.txt"),
        )),
        _ => None,
    };
    if let Some((mime, source)) = notice {
        return ([("content-type", mime)], source).into_response();
    }
    let source = match name.as_str() {
        "marked.js" => include_str!("../web/vendor/marked.js"),
        "dompurify.js" => include_str!("../web/vendor/dompurify.js"),
        "mermaid.js" => include_str!("../web/vendor/mermaid.js"),
        _ => return StatusCode::NOT_FOUND.into_response(),
    };
    javascript(source)
}
fn same_origin(state: &ServerState, request: &Request) -> bool {
    if request.headers().get("host").and_then(|h| h.to_str().ok()) != Some(state.authority.as_str())
    {
        return false;
    }
    if let Some(origin) = request.headers().get("origin") {
        if origin.to_str().ok() != Some(format!("http://{}", state.authority).as_str()) {
            return false;
        }
    }
    !request
        .headers()
        .get("sec-fetch-site")
        .and_then(|h| h.to_str().ok())
        .is_some_and(|s| ["cross-site", "same-site"].contains(&s))
}
async fn security_headers(
    State(state): State<ServerState>,
    request: Request,
    next: Next,
) -> Response {
    if !same_origin(&state, &request) {
        return AppError::new(
            "forbidden",
            "로컬 워크스페이스의 접속 주소와 출처를 확인하세요.",
        )
        .into_response();
    }
    let mut response = next.run(request).await;
    let h = response.headers_mut();
    h.insert("content-security-policy",HeaderValue::from_static("default-src 'none'; script-src 'self'; style-src 'self' 'unsafe-inline'; connect-src 'self'; img-src 'self' data:; base-uri 'none'; frame-ancestors 'none'; form-action 'none'"));
    h.insert(
        "x-content-type-options",
        HeaderValue::from_static("nosniff"),
    );
    h.insert("referrer-policy", HeaderValue::from_static("no-referrer"));
    h.insert("cache-control", HeaderValue::from_static("no-store"));
    response
}
fn constant_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |diff, (a, b)| diff | (a ^ b)) == 0
}
async fn authorize(State(state): State<ServerState>, request: Request, next: Next) -> Response {
    let auth = request
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .unwrap_or("");
    if !constant_eq(auth.as_bytes(), state.token.as_bytes()) {
        return AppError::new(
            "unauthorized",
            "CLI에 표시된 주소로 이 워크스페이스를 열어 주세요.",
        )
        .into_response();
    }
    if request.method() != axum::http::Method::GET
        && request
            .headers()
            .get("origin")
            .and_then(|v| v.to_str().ok())
            != Some(format!("http://{}", state.authority).as_str())
    {
        return AppError::new("forbidden", "동일한 로컬 화면에서만 변경할 수 있습니다.")
            .into_response();
    }
    if let Err(error) = state.app.ensure_workspace_current() {
        return error.into_response();
    }
    next.run(request).await
}
async fn workspace(State(s): State<ServerState>) -> Result<Json<Value>, AppError> {
    Ok(Json(s.app.workspace()?))
}
async fn session(
    State(s): State<ServerState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, AppError> {
    Ok(Json(s.app.session(&id)?))
}
async fn draft(
    State(s): State<ServerState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, AppError> {
    Ok(Json(s.app.draft(&id)?))
}
async fn run(
    State(s): State<ServerState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, AppError> {
    Ok(Json(s.app.run(&id)?))
}
async fn settings(State(s): State<ServerState>) -> Result<Json<Value>, AppError> {
    Ok(Json(s.app.settings()?))
}
async fn diagnostics(State(s): State<ServerState>) -> Result<Json<Value>, AppError> {
    Ok(Json(s.app.diagnostics().await?))
}
async fn create(
    State(s): State<ServerState>,
    Json(c): Json<Command>,
) -> Result<(StatusCode, Json<Value>), AppError> {
    Ok((
        StatusCode::ACCEPTED,
        Json(s.app.command("create", None, c)?),
    ))
}
async fn update_settings(
    State(s): State<ServerState>,
    Json(c): Json<Command>,
) -> Result<Json<Value>, AppError> {
    Ok(Json(s.app.command("settings", None, c)?))
}
macro_rules! mutation {
    ($name:ident,$operation:literal,$status:expr) => {
        async fn $name(
            State(s): State<ServerState>,
            Path(id): Path<String>,
            Json(c): Json<Command>,
        ) -> Result<(StatusCode, Json<Value>), AppError> {
            Ok(($status, Json(s.app.command($operation, Some(&id), c)?)))
        }
    };
}
mutation!(message, "message", StatusCode::ACCEPTED);
mutation!(
    retry_conversation,
    "retry_conversation",
    StatusCode::ACCEPTED
);
mutation!(generate_draft, "draft", StatusCode::ACCEPTED);
mutation!(critic, "critic", StatusCode::ACCEPTED);
mutation!(review, "review", StatusCode::OK);
mutation!(export, "export", StatusCode::OK);
mutation!(cancel, "cancel", StatusCode::OK);
mutation!(recover, "recover", StatusCode::OK);
async fn export_status(
    State(s): State<ServerState>,
    Path((id, eid)): Path<(String, String)>,
) -> Result<Json<Value>, AppError> {
    Ok(Json(s.app.export(&id, &eid)?))
}
#[derive(Default, Deserialize)]
pub struct StreamQuery {
    #[serde(default)]
    after: u64,
}
async fn session_events(
    State(s): State<ServerState>,
    Path(id): Path<String>,
    Query(q): Query<StreamQuery>,
) -> Result<Response, AppError> {
    s.app.session(&id)?;
    Ok(event_stream(s, id, None, q.after))
}
async fn run_events(
    State(s): State<ServerState>,
    Path(id): Path<String>,
    Query(q): Query<StreamQuery>,
) -> Result<Response, AppError> {
    let run = s.app.run(&id)?;
    let sid = run["session_id"].as_str().unwrap().to_owned();
    Ok(event_stream(s, sid, Some(id), q.after))
}
fn event_stream(state: ServerState, sid: String, rid: Option<String>, mut cursor: u64) -> Response {
    let mut receiver = state.app.events.subscribe();
    let stream = async_stream::stream! {
        let mut tick=tokio::time::interval(Duration::from_secs(5));
        loop {
            match state.app.replay(Some(&sid),cursor) {
                Ok(events)=>{for e in events {
                    if let Err(error)=state.app.ensure_workspace_current() {
                        yield Ok::<Event,Infallible>(Event::default().event(crate::storage::RESET_CODE).data(json!({"type":crate::storage::RESET_CODE,"error":{"code":error.code,"message":error.message}}).to_string()));
                        return;
                    }
                    cursor=e.cursor;let value=json!({"event_id":cursor,"session_id":e.session_id,"run_id":rid,"type":"state","payload":{"kind":e.kind,"data":e.payload}});yield Ok::<Event,Infallible>(Event::default().id(cursor.to_string()).event("state").data(value.to_string()));
                }},
                Err(error)=>{
                    if error.code==crate::storage::RESET_CODE {
                        yield Ok(Event::default().event(crate::storage::RESET_CODE).data(json!({"type":crate::storage::RESET_CODE,"error":{"code":error.code,"message":error.message}}).to_string()));
                    } else {
                        yield Ok(Event::default().event("error").data(json!({"type":"error","message":"상태 조회에 실패했습니다. 다시 연결하세요."}).to_string()));
                    }
                    break;
                }
            }
            tokio::select!{
                _=state.app.shutdown.cancelled()=>{
                    if let Err(error)=state.app.ensure_workspace_current() {
                        yield Ok(Event::default().event(crate::storage::RESET_CODE).data(json!({"type":crate::storage::RESET_CODE,"error":{"code":error.code,"message":error.message}}).to_string()));
                    }
                    break;
                },
                _=tick.tick()=>{},
                event=receiver.recv()=>{match event{
                    Ok(e) if e.session_id==sid && e.run_id.is_some() && (rid.is_none()||e.run_id==rid)=>{
                        if let Err(error)=state.app.ensure_workspace_current() {
                            yield Ok(Event::default().event(crate::storage::RESET_CODE).data(json!({"type":crate::storage::RESET_CODE,"error":{"code":error.code,"message":error.message}}).to_string()));
                            break;
                        }
                        let value=json!({"session_id":sid,"run_id":e.run_id,"type":e.kind,"payload":e.payload});yield Ok(Event::default().event(&e.kind).data(value.to_string()));
                    },
                    Ok(_)=>{},
                    Err(broadcast_error)=>{if matches!(broadcast_error,tokio::sync::broadcast::error::RecvError::Closed){break;}yield Ok(Event::default().event("resync_required").data(json!({"type":"resync_required","session_id":sid}).to_string()));}
                }}
            }
        }
    };
    Sse::new(stream)
        .keep_alive(KeepAlive::new().interval(Duration::from_secs(15)))
        .into_response()
}
