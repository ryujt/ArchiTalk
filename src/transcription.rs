//! Opt-in audio transcription. Secrets and audio never enter workspace storage.
use crate::files::Directory;
use fs2::FileExt;
use reqwest::{Client, multipart};
use serde::{Deserialize, Serialize};
use std::{
    io::Read,
    os::unix::fs::MetadataExt,
    path::{Component, Path, PathBuf},
    sync::Arc,
    time::Duration,
};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

pub const MAX_AUDIO_BYTES: usize = 20 * 1024 * 1024;
pub const MAX_REQUEST_BYTES: usize = MAX_AUDIO_BYTES + 64 * 1024;
pub const TIMEOUT_SECONDS: u64 = 120;
const ENDPOINT: &str = "https://api.openai.com/v1/audio/transcriptions";
const DEFAULT_MODEL: &str = "gpt-transcribe";
const SECRET_NAME: &str = "transcription-secrets.toml";
const MAX_SECRET_BYTES: usize = 16 * 1024;
const MAX_RESPONSE_BYTES: usize = 1024 * 1024;

#[derive(Debug, thiserror::Error)]
#[error("{message}")]
pub struct Error {
    pub code: &'static str,
    pub message: &'static str,
}
fn error(code: &'static str, message: &'static str) -> Error {
    Error { code, message }
}
fn settings_error(_: impl std::fmt::Display) -> Error {
    error(
        "transcription_settings_failed",
        "전사 비밀 설정 파일의 소유권·권한·경로를 확인하세요.",
    )
}
#[derive(Serialize)]
pub struct Settings {
    pub configured: bool,
    pub model: String,
    pub max_audio_bytes: usize,
    pub timeout_seconds: u64,
    pub key_source: &'static str,
}
// Deliberately no Debug/Serialize implementation for live credentials.
pub struct Credentials {
    key: String,
    pub model: String,
    source: &'static str,
}
#[derive(Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct Secrets {
    api_key: Option<String>,
    model: String,
}
impl Default for Secrets {
    fn default() -> Self {
        Self {
            api_key: None,
            model: DEFAULT_MODEL.into(),
        }
    }
}
pub struct Service {
    secret_path: Option<PathBuf>,
    client: Client,
    gate: Arc<Semaphore>,
}
pub struct Job {
    credentials: Credentials,
    _permit: OwnedSemaphorePermit,
}
#[derive(Serialize, Deserialize)]
pub struct Transcript {
    pub text: String,
    pub model: String,
}

impl Service {
    /// None is the isolated App::open mode: no shared files or environment keys.
    pub fn new(shared_config: Option<&Path>) -> anyhow::Result<Self> {
        Ok(Self {
            secret_path: shared_config.map(normalize_secret_path).transpose()?,
            client: Client::builder()
                .connect_timeout(Duration::from_secs(10))
                .timeout(Duration::from_secs(TIMEOUT_SECONDS))
                .redirect(reqwest::redirect::Policy::none())
                .no_proxy()
                .build()?,
            gate: Arc::new(Semaphore::new(1)),
        })
    }
    pub fn settings(&self) -> Result<Settings, Error> {
        let credentials = self.credentials()?;
        Ok(Settings {
            configured: !credentials.key.is_empty(),
            model: credentials.model,
            max_audio_bytes: MAX_AUDIO_BYTES,
            timeout_seconds: TIMEOUT_SECONDS,
            key_source: credentials.source,
        })
    }
    fn credentials(&self) -> Result<Credentials, Error> {
        let secrets = if let Some(path) = &self.secret_path {
            read_secrets(path).map_err(settings_error)?
        } else {
            Secrets::default()
        };
        let (key, source) = if let Some(key) = secrets.api_key.filter(|k| !k.is_empty()) {
            (key, "file")
        } else if self.secret_path.is_some() {
            match std::env::var("OPENAI_API_KEY")
                .ok()
                .filter(|k| !k.is_empty())
            {
                Some(key) => (key, "environment"),
                None => (String::new(), "none"),
            }
        } else {
            (String::new(), "none")
        };
        if !key.is_empty() {
            validate_key(&key)?;
        }
        validate_model(&secrets.model)?;
        Ok(Credentials {
            key,
            model: secrets.model,
            source,
        })
    }
    pub fn update(&self, value: &serde_json::Value) -> Result<Settings, Error> {
        let object = value.as_object().ok_or_else(|| {
            error(
                "invalid_transcription_settings",
                "전사 설정은 JSON 객체여야 합니다.",
            )
        })?;
        if object
            .keys()
            .any(|k| !matches!(k.as_str(), "api_key" | "model"))
        {
            return Err(error(
                "invalid_transcription_settings",
                "허용되지 않은 전사 설정입니다.",
            ));
        }
        let path = self.secret_path.as_ref().ok_or_else(|| {
            error(
                "transcription_settings_unavailable",
                "공용 설정을 사용하는 서버에서 전사 키를 설정하세요.",
            )
        })?;
        let parent = secret_parent(path, true)
            .map_err(settings_error)?
            .ok_or_else(|| settings_error("missing parent"))?;
        let lock = parent
            .open_file("transcription-secrets.lock", true)
            .map_err(settings_error)?;
        check_private(&lock.metadata().map_err(settings_error)?).map_err(settings_error)?;
        lock.lock_exclusive().map_err(settings_error)?;
        let mut secrets = read_secrets(path).map_err(settings_error)?;
        if let Some(key) = object.get("api_key") {
            secrets.api_key = match key {
                serde_json::Value::Null => None,
                serde_json::Value::String(k) if k.is_empty() => None,
                serde_json::Value::String(k) => {
                    validate_key(k)?;
                    Some(k.clone())
                }
                _ => {
                    return Err(error(
                        "invalid_transcription_settings",
                        "API 키는 문자열 또는 null이어야 합니다.",
                    ));
                }
            };
        }
        if let Some(model) = object.get("model") {
            let model = model.as_str().ok_or_else(|| {
                error(
                    "invalid_transcription_settings",
                    "전사 모델 이름을 확인하세요.",
                )
            })?;
            validate_model(model)?;
            secrets.model = model.into();
        }
        let current = secret_parent(path, false)
            .map_err(settings_error)?
            .ok_or_else(|| settings_error("missing parent"))?;
        if current.identity().map_err(settings_error)?
            != parent.identity().map_err(settings_error)?
        {
            return Err(settings_error("parent changed"));
        }
        let bytes = toml::to_string(&secrets).map_err(settings_error)?;
        parent
            .replace_private_file(SECRET_NAME, bytes.as_bytes())
            .map_err(settings_error)?;
        drop(lock);
        self.settings()
    }
    /// Called before multipart extraction so absent keys never consume audio.
    pub fn begin(&self) -> Result<Job, Error> {
        let credentials = self.credentials()?;
        if credentials.key.is_empty() {
            return Err(error(
                "transcription_key_missing",
                "OpenAI API 키를 설정한 뒤 녹음을 시작하세요.",
            ));
        }
        let permit = self.gate.clone().try_acquire_owned().map_err(|_| {
            error(
                "transcription_busy",
                "다른 음성 전사가 진행 중입니다. 완료 후 다시 시도하세요.",
            )
        })?;
        Ok(Job {
            credentials,
            _permit: permit,
        })
    }
    pub async fn transcribe(
        &self,
        job: Job,
        bytes: Vec<u8>,
        mime: &str,
    ) -> Result<Transcript, Error> {
        self.send(job, bytes, mime, ENDPOINT).await
    }
    async fn send(
        &self,
        job: Job,
        bytes: Vec<u8>,
        mime: &str,
        endpoint: &str,
    ) -> Result<Transcript, Error> {
        let extension = validate_audio(&bytes, mime)?;
        let model = job.credentials.model.clone();
        let part = multipart::Part::bytes(bytes)
            .file_name(format!("recording.{extension}"))
            .mime_str(mime.split(';').next().unwrap_or(mime))
            .map_err(|_| error("invalid_audio", "지원되는 오디오 형식을 사용하세요."))?;
        let form = multipart::Form::new()
            .text("model", model.clone())
            .part("file", part);
        let mut authorization =
            reqwest::header::HeaderValue::from_str(&format!("Bearer {}", job.credentials.key))
                .map_err(|_| {
                    error(
                        "invalid_transcription_settings",
                        "API 키 형식을 확인하세요.",
                    )
                })?;
        authorization.set_sensitive(true);
        let mut response = self
            .client
            .post(endpoint)
            .header(reqwest::header::AUTHORIZATION, authorization)
            .multipart(form)
            .send()
            .await
            .map_err(transport_error)?;
        if !response.status().is_success() {
            return Err(match response.status().as_u16() {
                401 | 403 => error(
                    "transcription_auth_failed",
                    "OpenAI API 키와 접근 권한을 확인하세요.",
                ),
                429 => error(
                    "transcription_rate_limited",
                    "OpenAI 사용 한도에 도달했습니다. 잠시 후 다시 시도하세요.",
                ),
                _ => error(
                    "transcription_upstream_failed",
                    "OpenAI 음성 전사 요청에 실패했습니다.",
                ),
            });
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(transport_error)? {
            if bytes.len().saturating_add(chunk.len()) > MAX_RESPONSE_BYTES {
                return Err(error(
                    "transcription_response_invalid",
                    "전사 응답이 허용 크기를 넘었습니다.",
                ));
            }
            bytes.extend_from_slice(&chunk);
        }
        #[derive(Deserialize)]
        struct Response {
            text: String,
        }
        let parsed: Response = serde_json::from_slice(&bytes).map_err(|_| {
            error(
                "transcription_response_invalid",
                "전사 응답을 읽을 수 없습니다.",
            )
        })?;
        if parsed.text.contains('\0') {
            return Err(error(
                "transcription_response_invalid",
                "전사 응답을 읽을 수 없습니다.",
            ));
        }
        Ok(Transcript {
            text: parsed.text,
            model,
        })
    }
}
fn transport_error(cause: reqwest::Error) -> Error {
    if cause.is_timeout() {
        error(
            "transcription_timeout",
            "음성 전사 요청이 120초 제한을 초과했습니다.",
        )
    } else {
        error(
            "transcription_connection_failed",
            "음성 전사 서버에 연결할 수 없습니다.",
        )
    }
}
// Never attach reqwest errors, response bodies, or configuration text to errors.
fn validate_model(model: &str) -> Result<(), Error> {
    if model.is_empty()
        || model.len() > 120
        || !model.as_bytes()[0].is_ascii_alphanumeric()
        || !model
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
    {
        return Err(error(
            "invalid_transcription_settings",
            "전사 모델 이름은 영문·숫자·점·밑줄·하이픈 120자 이내로 입력하세요.",
        ));
    }
    Ok(())
}
fn validate_key(key: &str) -> Result<(), Error> {
    if key.is_empty() || key.len() > 8192 || !key.bytes().all(|b| b.is_ascii_graphic()) {
        return Err(error(
            "invalid_transcription_settings",
            "API 키 형식을 확인하세요.",
        ));
    }
    Ok(())
}
pub fn validate_audio(bytes: &[u8], mime: &str) -> Result<&'static str, Error> {
    if bytes.is_empty() || bytes.len() > MAX_AUDIO_BYTES {
        return Err(error("invalid_audio", "오디오는 20 MiB 이내여야 합니다."));
    }
    let mime = mime.split(';').next().unwrap_or(mime).trim();
    let kind = match mime {
        "audio/webm" if bytes.starts_with(&[0x1a, 0x45, 0xdf, 0xa3]) => Some("webm"),
        "audio/ogg" if bytes.starts_with(b"OggS") => Some("ogg"),
        "audio/wav" | "audio/x-wav"
            if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WAVE") =>
        {
            Some("wav")
        }
        "audio/mp4" | "audio/x-m4a" if bytes.get(4..8) == Some(b"ftyp") => Some("m4a"),
        "audio/mpeg" | "audio/mp3"
            if bytes.starts_with(b"ID3")
                || bytes
                    .get(..2)
                    .is_some_and(|b| b[0] == 0xff && b[1] & 0xe0 == 0xe0) =>
        {
            Some("mp3")
        }
        "audio/flac" if bytes.starts_with(b"fLaC") => Some("flac"),
        _ => None,
    };
    kind.ok_or_else(|| {
        error(
            "invalid_audio",
            "지원되는 오디오 형식과 파일 내용을 확인하세요.",
        )
    })
}
fn check_private(metadata: &std::fs::Metadata) -> anyhow::Result<()> {
    anyhow::ensure!(
        metadata.is_file()
            && metadata.uid() == unsafe { libc::geteuid() }
            && metadata.nlink() == 1
            && metadata.mode() & 0o777 == 0o600,
        "unsafe secret file"
    );
    Ok(())
}
// Resolve only the trusted configuration location once, including macOS's
// system /var alias. Child files and all later operations remain NOFOLLOW.
fn normalize_secret_path(config: &Path) -> anyhow::Result<PathBuf> {
    let absolute = if config.is_absolute() {
        config.to_owned()
    } else {
        std::env::current_dir()?.join(config)
    };
    let mut ancestor = absolute
        .parent()
        .ok_or_else(|| anyhow::anyhow!("missing config parent"))?
        .to_owned();
    let mut missing = Vec::new();
    let mut canonical = loop {
        match ancestor.canonicalize() {
            Ok(path) => break path,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                missing.push(
                    ancestor
                        .file_name()
                        .ok_or_else(|| anyhow::anyhow!("missing ancestor"))?
                        .to_owned(),
                );
                ancestor.pop();
            }
            Err(e) => return Err(e.into()),
        }
    };
    for part in missing.into_iter().rev() {
        canonical.push(part);
    }
    Ok(canonical.join(SECRET_NAME))
}
fn secret_parent(path: &Path, create: bool) -> anyhow::Result<Option<Directory>> {
    let absolute = if path.is_absolute() {
        path.to_owned()
    } else {
        std::env::current_dir()?.join(path)
    };
    let mut parent = Directory::root(Path::new("/"))?;
    for part in absolute
        .parent()
        .ok_or_else(|| anyhow::anyhow!("missing parent"))?
        .components()
    {
        match part {
            Component::RootDir | Component::CurDir => {}
            Component::Normal(name) => {
                let name = name
                    .to_str()
                    .ok_or_else(|| anyhow::anyhow!("non UTF-8 config path"))?;
                parent = match parent.open_dir(name) {
                    Ok(next) => next,
                    Err(e)
                        if e.downcast_ref::<std::io::Error>()
                            .is_some_and(|e| e.kind() == std::io::ErrorKind::NotFound) =>
                    {
                        if !create {
                            return Ok(None);
                        }
                        parent.mkdir(name, 0o700)?;
                        parent.open_dir(name)?
                    }
                    Err(e) => return Err(e),
                };
            }
            _ => anyhow::bail!("unsafe config path"),
        }
    }
    let m = parent.metadata()?;
    anyhow::ensure!(
        m.uid() == unsafe { libc::geteuid() } && m.mode() & 0o022 == 0,
        "unsafe config directory"
    );
    Ok(Some(parent))
}
fn read_secrets(path: &Path) -> anyhow::Result<Secrets> {
    let Some(parent) = secret_parent(path, false)? else {
        return Ok(Secrets::default());
    };
    let file = match parent.open_file(SECRET_NAME, false) {
        Ok(file) => file,
        Err(e)
            if e.downcast_ref::<std::io::Error>()
                .is_some_and(|e| e.kind() == std::io::ErrorKind::NotFound) =>
        {
            return Ok(Secrets::default());
        }
        Err(e) => return Err(e),
    };
    check_private(&file.metadata()?)?;
    let mut text = String::new();
    file.take(MAX_SECRET_BYTES as u64 + 1)
        .read_to_string(&mut text)?;
    anyhow::ensure!(
        text.len() <= MAX_SECRET_BYTES,
        "oversized secret configuration"
    );
    Ok(toml::from_str(&text)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        Router,
        extract::{Request, State},
        http::StatusCode,
        response::IntoResponse,
        routing::post,
    };
    use std::sync::{
        Mutex,
        atomic::{AtomicUsize, Ordering},
    };
    use tokio::net::TcpListener;
    #[derive(Clone)]
    struct Fake {
        calls: Arc<AtomicUsize>,
        body: Arc<Mutex<Vec<u8>>>,
        status: StatusCode,
        response: String,
        location: Option<String>,
        delay: Duration,
    }
    async fn fake(State(state): State<Fake>, request: Request) -> axum::response::Response {
        state.calls.fetch_add(1, Ordering::SeqCst);
        assert_eq!(
            request.headers()["authorization"],
            "Bearer synthetic-unit-key"
        );
        let bytes = axum::body::to_bytes(request.into_body(), MAX_REQUEST_BYTES)
            .await
            .unwrap();
        *state.body.lock().unwrap() = bytes.to_vec();
        tokio::time::sleep(state.delay).await;
        let mut response = (state.status, state.response).into_response();
        if let Some(location) = state.location {
            response
                .headers_mut()
                .insert("location", location.parse().unwrap());
        }
        response
    }
    async fn upstream(
        status: StatusCode,
        response: String,
        delay: Duration,
    ) -> (String, Fake, tokio::task::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!(
            "http://{}/v1/audio/transcriptions",
            listener.local_addr().unwrap()
        );
        let state = Fake {
            calls: Arc::default(),
            body: Arc::default(),
            status,
            response,
            location: Some(endpoint.clone()),
            delay,
        };
        let router = Router::new()
            .route("/v1/audio/transcriptions", post(fake))
            .with_state(state.clone());
        let task = tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });
        (endpoint, state, task)
    }
    fn service_job(service: &Service) -> Job {
        Job {
            credentials: Credentials {
                key: "synthetic-unit-key".into(),
                model: DEFAULT_MODEL.into(),
                source: "none",
            },
            _permit: service.gate.clone().try_acquire_owned().unwrap(),
        }
    }
    fn webm() -> Vec<u8> {
        vec![0x1a, 0x45, 0xdf, 0xa3, 0, 0, 0, 0]
    }
    #[tokio::test]
    async fn fake_upstream_receives_only_audio_and_model_and_text_is_returned_without_fake_segments()
     {
        let (endpoint, state, task) = upstream(
            StatusCode::OK,
            "{\"text\":\"전사 결과\",\"languages\":[{\"code\":\"ko\"}]}".into(),
            Duration::ZERO,
        )
        .await;
        let service = Service::new(None).unwrap();
        let result = service
            .send(
                service_job(&service),
                webm(),
                "audio/webm;codecs=opus",
                &endpoint,
            )
            .await
            .unwrap();
        assert_eq!(result.text, "전사 결과");
        assert_eq!(result.model, DEFAULT_MODEL);
        let wire = state.body.lock().unwrap().clone();
        let wire = String::from_utf8_lossy(&wire);
        assert!(wire.contains("name=\"model\"\r\n\r\ngpt-transcribe"));
        assert!(wire.contains("name=\"file\"; filename=\"recording.webm\""));
        assert_eq!(wire.matches("Content-Disposition:").count(), 2);
        assert!(!wire.contains("events"));
        assert!(!wire.contains("synthetic-unit-key"));
        assert_eq!(state.calls.load(Ordering::SeqCst), 1);
        assert!(service.gate.clone().try_acquire_owned().is_ok());
        task.abort();
    }
    #[tokio::test]
    async fn redirects_and_upstream_error_bodies_are_redacted_and_never_retried() {
        for (status, code) in [
            (
                StatusCode::TEMPORARY_REDIRECT,
                "transcription_upstream_failed",
            ),
            (StatusCode::UNAUTHORIZED, "transcription_auth_failed"),
            (StatusCode::TOO_MANY_REQUESTS, "transcription_rate_limited"),
        ] {
            let (endpoint, state, task) = upstream(
                status,
                "synthetic-upstream-secret-and-audio".into(),
                Duration::ZERO,
            )
            .await;
            let service = Service::new(None).unwrap();
            let failure = service
                .send(service_job(&service), webm(), "audio/webm", &endpoint)
                .await
                .err()
                .unwrap();
            assert_eq!(failure.code, code);
            assert!(!format!("{failure:?} {failure}").contains("synthetic"));
            assert_eq!(state.calls.load(Ordering::SeqCst), 1);
            task.abort();
        }
    }
    #[tokio::test]
    async fn invalid_audio_never_reaches_upstream_and_invalid_oversized_responses_fail_closed() {
        let (endpoint, state, task) = upstream(
            StatusCode::OK,
            "{\"text\":\"unused\"}".into(),
            Duration::ZERO,
        )
        .await;
        let service = Service::new(None).unwrap();
        for (bytes, mime) in [
            (webm(), "text/html"),
            (b"not webm".to_vec(), "audio/webm"),
            (vec![0; MAX_AUDIO_BYTES + 1], "audio/webm"),
        ] {
            assert_eq!(
                service
                    .send(service_job(&service), bytes, mime, &endpoint)
                    .await
                    .err()
                    .unwrap()
                    .code,
                "invalid_audio"
            );
        }
        assert_eq!(state.calls.load(Ordering::SeqCst), 0);
        task.abort();
        for response in [
            "not JSON synthetic-secret".into(),
            "x".repeat(MAX_RESPONSE_BYTES + 1),
        ] {
            let (endpoint, state, task) = upstream(StatusCode::OK, response, Duration::ZERO).await;
            let failure = service
                .send(service_job(&service), webm(), "audio/webm", &endpoint)
                .await
                .err()
                .unwrap();
            assert_eq!(failure.code, "transcription_response_invalid");
            assert!(!failure.message.contains("synthetic"));
            assert_eq!(state.calls.load(Ordering::SeqCst), 1);
            task.abort();
        }
    }
    #[tokio::test]
    async fn transport_timeout_and_dropped_future_release_the_single_slot_without_retry() {
        let (endpoint, state, task) = upstream(
            StatusCode::OK,
            "{\"text\":\"late\"}".into(),
            Duration::from_secs(1),
        )
        .await;
        let mut service = Service::new(None).unwrap();
        service.client = Client::builder()
            .timeout(Duration::from_millis(40))
            .build()
            .unwrap();
        let failure = service
            .send(service_job(&service), webm(), "audio/webm", &endpoint)
            .await
            .err()
            .unwrap();
        assert_eq!(failure.code, "transcription_timeout");
        assert!(service.gate.clone().try_acquire_owned().is_ok());
        assert_eq!(state.calls.load(Ordering::SeqCst), 1);
        let mut future =
            Box::pin(service.send(service_job(&service), webm(), "audio/webm", &endpoint));
        tokio::select! { _ = &mut future => panic!("expected pending request"), _ = tokio::time::sleep(Duration::from_millis(5)) => {} }
        drop(future);
        assert!(service.gate.clone().try_acquire_owned().is_ok());
        // Dropping the owned future, as the HTTP reset/shutdown select does,
        // releases both its credential-bearing request and semaphore permit.
        task.abort();
    }
    #[test]
    fn defaults_and_supported_container_signatures_are_bounded() {
        let service = Service::new(None).unwrap();
        let settings = service.settings().unwrap();
        assert!(!settings.configured);
        assert_eq!(settings.key_source, "none");
        assert_eq!(settings.max_audio_bytes, 20 * 1024 * 1024);
        assert_eq!(settings.timeout_seconds, 120);
        assert_eq!(validate_audio(b"OggS0000", "audio/ogg").unwrap(), "ogg");
        assert_eq!(validate_audio(b"0000ftyp0000", "audio/mp4").unwrap(), "m4a");
        assert_eq!(validate_audio(b"RIFF0000WAVE", "audio/wav").unwrap(), "wav");
    }
}
