//! Version-gated, text-only local CLI adapters. User source and published files
//! are not modified here. Only private runtime scratch is writable. The controller alone
//! decides whether a terminal result commits.
use crate::files::Directory;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    path::{Path, PathBuf},
    process::Stdio,
    sync::{
        Arc,
        atomic::{AtomicU64, AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWriteExt},
    process::{Child, Command},
    sync::mpsc,
    task::JoinHandle,
};
use tokio_util::sync::CancellationToken;

const FRAME_LIMIT: usize = 1024 * 1024;
const TOTAL_LIMIT: usize = 16 * 1024 * 1024;
const STDERR_RETAIN: usize = 64 * 1024;
const GRACE: Duration = Duration::from_secs(3);
const CODEX_DISABLED: &[&str] = &[
    "shell_tool",
    "unified_exec",
    "hooks",
    "plugins",
    "apps",
    "multi_agent",
    "multi_agent_v2",
    "browser_use",
    "computer_use",
    "image_generation",
    "code_mode",
    "code_mode_host",
    "in_app_browser",
    "shell_snapshot",
    "memories",
    "skill_search",
    "skill_mcp_dependency_install",
    "remote_plugin",
    "workspace_dependencies",
];

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Backend {
    Claude,
    Codex,
}
#[derive(Clone, Debug, Serialize)]
pub struct BackendChoices {
    pub models: Vec<String>,
    pub efforts: Vec<String>,
}
impl Backend {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::Codex => "codex",
        }
    }
    fn versions(self) -> &'static [&'static str] {
        match self {
            Self::Claude => &["2.1.326", "2.1.327"],
            Self::Codex => &["0.169.0"],
        }
    }
    fn efforts(self) -> Vec<String> {
        match self {
            Self::Claude => ["low", "medium", "high", "xhigh", "max"].as_slice(),
            Self::Codex => ["minimal", "low", "medium", "high", "xhigh", "max", "ultra"].as_slice(),
        }
        .iter()
        .map(|s| (*s).to_owned())
        .collect()
    }
    /// Known examples, not an allowlist or proof of account/model availability.
    /// Claude aliases/efforts came from installed help; Codex IDs and max/ultra
    /// came from local model catalog metadata inspected on 2026-11-18.
    pub fn choices(self) -> BackendChoices {
        let models = match self {
            Self::Claude => ["sonnet", "opus", "fable"].as_slice(),
            Self::Codex => [
                "gpt-6.2-sol",
                "gpt-6.2-astra",
                "gpt-6.1-terra",
                "gpt-6.1-sol",
                "gpt-6-astra",
                "gpt-6-sol",
            ]
            .as_slice(),
        };
        BackendChoices {
            models: models.iter().map(|model| (*model).to_owned()).collect(),
            efforts: self.efforts(),
        }
    }
}
impl std::fmt::Display for Backend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Clone, Debug)]
pub struct RunSpec {
    pub backend: Backend,
    pub executable: String,
    pub model: Option<String>,
    pub effort: Option<String>,
    pub profile: Option<String>,
    pub timeout_seconds: u64,
    pub prompt: String,
    pub project_root: PathBuf,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Diagnostic {
    pub backend: Backend,
    pub available: bool,
    pub compatible: bool,
    pub version: Option<String>,
    pub message: String,
    pub efforts: Vec<String>,
    #[serde(default)]
    pub termination_unknown: bool,
}
#[derive(Clone, Debug)]
pub struct AiOutput {
    pub text: String,
    pub cli_version: String,
    pub resolved_model: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AiEvent {
    Progress { message: String },
    Delta { text: String },
}
#[derive(Debug, thiserror::Error)]
#[error("{message}")]
pub struct AiError {
    pub code: String,
    pub message: String,
    pub termination_unknown: bool,
}
impl AiError {
    fn new(code: &str, message: impl Into<String>) -> Self {
        Self {
            code: code.to_owned(),
            message: message.into(),
            termination_unknown: false,
        }
    }
}
/// Return fixed vocabulary only: never copy stderr, paths, prompts or secrets
/// into logs/UI. The retained input is already bounded by STDERR_RETAIN.
fn failure_summary(bytes: &[u8]) -> String {
    let text = String::from_utf8_lossy(bytes).to_lowercase();
    let has = |markers: &[&str]| markers.iter().any(|marker| text.contains(marker));
    let reason = if text.is_empty() {
        "none"
    } else if has(&[
        "operation not permitted",
        "permission denied",
        "os error 1)",
        "os error 13)",
        "readonly database",
        "read-only file system",
    ]) {
        "permission_denied"
    } else if has(&[
        "unauthorized",
        "not logged in",
        "login required",
        "token expired",
        "refresh token",
        "invalid api key",
        "401",
    ]) {
        "authentication"
    } else if has(&[
        "unexpected argument",
        "unrecognized",
        "invalid value",
        "unknown field",
        "error loading configuration",
        "invalid configuration",
    ]) {
        "configuration"
    } else if has(&[
        "connection refused",
        "network is unreachable",
        "failed to connect",
        "dns",
        "request failed",
        "connection failed",
    ]) {
        "network"
    } else if has(&["no prompt provided", "prompt cannot be empty"]) {
        "empty_input"
    } else if has(&["failed to create", "no such file", "i/o error"]) {
        "startup_io"
    } else {
        "unclassified"
    };
    let component = if has(&[
        "models_cache",
        "model catalog",
        "models-manager",
        "models_manager",
    ]) {
        "model_catalog_cache"
    } else if has(&["arg0", "path aliases"]) {
        "path_alias_cache"
    } else if has(&["skills"]) {
        "skills_cache"
    } else if has(&["sqlite", "database"]) {
        "database"
    } else if has(&["history", "rollout", "session index", "session_index"]) {
        "session_history"
    } else if has(&["log_dir", "logs_", "log file", "logging"]) {
        "logs"
    } else if has(&["auth.json", "credentials", "refresh token"]) {
        "authentication"
    } else if has(&["config.toml", "configuration"]) {
        "configuration"
    } else {
        "unknown"
    };
    format!("reason={reason}, component={component}")
}

fn failure_line_summaries(bytes: &[u8]) -> String {
    let text = String::from_utf8_lossy(bytes);
    let mut summaries = Vec::new();
    for line in text.lines() {
        let lower = line.to_lowercase();
        let severity = if lower.contains("warn") {
            "warning"
        } else if lower.contains("error") {
            "error"
        } else {
            "notice"
        };
        let summary = format!("level={severity}, {}", failure_summary(line.as_bytes()));
        if !summaries.contains(&summary) {
            summaries.push(summary);
        }
        if summaries.len() == 8 {
            break;
        }
    }
    if summaries.is_empty() {
        failure_summary(bytes)
    } else {
        summaries.join("; ")
    }
}

fn io_error(error: std::io::Error) -> AiError {
    AiError::new("cli_io_error", format!("AI CLI I/O failed: {error}"))
}

/// This command never invokes a model or changes login state. Capability and
/// parser profiles are deliberately pinned; help does not prove authentication.
pub async fn diagnose(backend: Backend, executable: &str) -> Diagnostic {
    diagnose_at(backend, executable, None).await
}

/// Project-aware preparation never puts a project prompt or CLI runtime in a
/// shared temporary directory. Authentication remains read-only in its normal home.
pub async fn diagnose_in_project(backend: Backend, executable: &str, project: &Path) -> Diagnostic {
    diagnose_at(backend, executable, Some(project)).await
}

async fn diagnose_at(backend: Backend, executable: &str, project: Option<&Path>) -> Diagnostic {
    let mut diagnostic = Diagnostic {
        backend,
        available: false,
        compatible: false,
        version: None,
        message: String::new(),
        efforts: backend.efforts(),
        termination_unknown: false,
    };
    let path = match executable_path(executable) {
        Ok(path) => path,
        Err(error) => {
            diagnostic.termination_unknown = error.termination_unknown;
            diagnostic.message = error.message;
            return diagnostic;
        }
    };
    diagnostic.available = true;
    let temp = match RunDirectory::create(project) {
        Ok(temp) => temp,
        Err(error) => {
            diagnostic.termination_unknown = error.termination_unknown;
            diagnostic.message = error.message;
            return diagnostic;
        }
    };
    let version = match capture_info(&path, &["--version"], &temp, backend).await {
        Ok(version) => version,
        Err(error) => {
            diagnostic.termination_unknown = error.termination_unknown;
            diagnostic.message = error.message;
            return diagnostic;
        }
    };
    let observed = version.trim();
    diagnostic.version = Some(observed.to_owned());
    let supported = backend.versions().iter().any(|version| {
        observed
            == match backend {
                Backend::Claude => format!("{version} (Claude Code)"),
                Backend::Codex => format!("codex-cli {version}"),
            }
    });
    if !supported {
        let observed: String = observed.chars().take(256).collect();
        diagnostic.message = format!(
            "Unverified {backend} version {observed:?} at {}. Supported profiles: {}. No model was called.",
            path.display(),
            backend.versions().join(", ")
        );
        return diagnostic;
    }
    let args: &[&str] = match backend {
        Backend::Claude => &["--help"],
        Backend::Codex => &["exec", "--help"],
    };
    let help = match capture_info(&path, args, &temp, backend).await {
        Ok(help) => help,
        Err(error) => {
            diagnostic.termination_unknown = error.termination_unknown;
            diagnostic.message = error.message;
            return diagnostic;
        }
    };
    let required: &[&str] = match backend {
        Backend::Claude => &[
            "--print",
            "--output-format",
            "--include-partial-messages",
            "--safe-mode",
            "--restricted",
            "--strict-mcp-config",
            "--mcp-config",
            "--setting-sources",
            "--settings",
            "--tools",
            "--permission-mode",
            "--disable-slash-commands",
            "--no-session-persistence",
            "--effort",
        ],
        Backend::Codex => &[
            "--json",
            "--ignore-user-config",
            "--ignore-rules",
            "--ephemeral",
            "--sandbox",
            "--skip-git-repo-check",
            "--disable",
        ],
    };
    if let Some(missing) = required.iter().find(|required| !help.contains(**required)) {
        diagnostic.message =
            format!("Required restriction option missing: {missing}. No model was called.");
        return diagnostic;
    }
    if !cfg!(target_os = "macos") || !Path::new("/usr/bin/sandbox-exec").is_file() {
        diagnostic.message = "restricted_execution_unavailable: OS project-write isolation is only verified on macOS with sandbox-exec. No model was called.".to_owned();
        return diagnostic;
    }
    if managed_policy_present(backend) {
        diagnostic.message = "restricted_execution_unavailable: managed CLI policy exists and its tool/hook restrictions are not verified. Review the policy before enabling this backend.".to_owned();
        return diagnostic;
    }
    diagnostic.compatible = true;
    diagnostic.message = format!(
        "Verified {backend} {} option/parser profile at {}; project writes are OS-denied during generation. Login and per-model effort support require a requested model call. Named Codex profiles are not supported.",
        observed,
        path.display()
    );
    diagnostic
}

fn diagnostic_failure(diagnostic: Diagnostic) -> AiError {
    AiError {
        code: if diagnostic.termination_unknown {
            "process_termination_unknown"
        } else if !diagnostic.available {
            "cli_not_found"
        } else {
            "restricted_execution_unavailable"
        }
        .to_owned(),
        message: diagnostic.message,
        termination_unknown: diagnostic.termination_unknown,
    }
}

fn policy_path_present(path: &Path) -> bool {
    match std::fs::symlink_metadata(path) {
        Ok(_) => true,
        Err(error) => error.kind() != std::io::ErrorKind::NotFound,
    }
}
fn managed_policy_present(backend: Backend) -> bool {
    // Both vendors retain managed policy above ordinary user-config restrictions.
    // We reject local/MDM layers whose effective tool/hook settings are unknown.
    let paths: &[&str] = match backend {
        Backend::Claude => &[
            "/Library/Application Support/ClaudeCode/managed-settings.json",
            "/Library/Application Support/ClaudeCode/managed-mcp.json",
            "/Library/Application Support/ClaudeCode/managed-settings.d",
        ],
        Backend::Codex => &[
            "/etc/codex/config.toml",
            "/etc/codex/requirements.toml",
            "/etc/codex/managed_config.toml",
        ],
    };
    if paths
        .iter()
        .any(|path| policy_path_present(Path::new(path)))
    {
        return true;
    }
    let domain = match backend {
        Backend::Claude => "com.anthropic.claudecode.plist",
        Backend::Codex => "com.openai.codex.plist",
    };
    let managed = Path::new("/Library/Managed Preferences");
    if policy_path_present(&managed.join(domain)) {
        return true;
    }
    if let Ok(users) = std::fs::read_dir(managed) {
        for entry in users.take(1024) {
            match entry {
                Ok(entry) if policy_path_present(&entry.path().join(domain)) => return true,
                Err(_) => return true,
                _ => {}
            }
        }
    } else if policy_path_present(managed) {
        return true;
    }
    if let Some(home) = std::env::var_os("HOME") {
        let home = PathBuf::from(home);
        if backend == Backend::Claude
            && policy_path_present(&home.join("Library/Preferences").join(domain))
        {
            return true;
        }
        if backend == Backend::Codex
            && ["config_toml_base64", "requirements_toml_base64"]
                .iter()
                .any(|key| mac_preference_key_present("com.openai.codex", key))
        {
            return true;
        }
        if backend == Backend::Claude {
            let cache = home.join(".claude/remote-settings.json");
            if policy_path_present(&cache) {
                // The observed personal-account cache contains this single
                // non-executable feature flag. Unknown remote policy fails closed.
                let mut options = std::fs::OpenOptions::new();
                options.read(true);
                #[cfg(unix)]
                {
                    use std::os::unix::fs::OpenOptionsExt;
                    options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
                }
                let Ok(file) = options.open(cache) else {
                    return true;
                };
                let Ok(metadata) = file.metadata() else {
                    return true;
                };
                if !metadata.is_file() || metadata.len() > FRAME_LIMIT as u64 {
                    return true;
                }
                let mut bytes = Vec::new();
                if std::io::Read::read_to_end(
                    &mut std::io::Read::take(file, (FRAME_LIMIT + 1) as u64),
                    &mut bytes,
                )
                .is_err()
                    || bytes.len() > FRAME_LIMIT
                {
                    return true;
                }
                let Ok(Value::Object(policy)) = serde_json::from_slice::<Value>(&bytes) else {
                    return true;
                };
                if policy
                    .iter()
                    .any(|(key, value)| key != "channelsEnabled" || !value.is_boolean())
                {
                    return true;
                }
            }
        }
    }
    false
}

#[cfg(target_os = "macos")]
fn mac_preference_key_present(domain: &str, key: &str) -> bool {
    use std::ffi::{CString, c_char, c_void};
    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        fn CFStringCreateWithCString(
            allocator: *const c_void,
            value: *const c_char,
            encoding: u32,
        ) -> *const c_void;
        fn CFPreferencesCopyAppValue(
            key: *const c_void,
            application: *const c_void,
        ) -> *const c_void;
        fn CFRelease(value: *const c_void);
    }
    let domain = CString::new(domain).unwrap();
    let key = CString::new(key).unwrap();
    unsafe {
        let application = CFStringCreateWithCString(std::ptr::null(), domain.as_ptr(), 0x08000100);
        let key = CFStringCreateWithCString(std::ptr::null(), key.as_ptr(), 0x08000100);
        if application.is_null() || key.is_null() {
            if !application.is_null() {
                CFRelease(application);
            }
            if !key.is_null() {
                CFRelease(key);
            }
            return true;
        }
        let value = CFPreferencesCopyAppValue(key, application);
        let present = !value.is_null();
        if present {
            CFRelease(value);
        }
        CFRelease(key);
        CFRelease(application);
        present
    }
}
#[cfg(not(target_os = "macos"))]
fn mac_preference_key_present(_domain: &str, _key: &str) -> bool {
    false
}

fn executable_path(executable: &str) -> Result<PathBuf, AiError> {
    if executable.is_empty() || executable.contains(['\0', '\n', '\r']) {
        return Err(AiError::new(
            "cli_not_found",
            "AI executable is empty or invalid.",
        ));
    }
    let path = which::which(executable).map_err(|_| AiError::new("cli_not_found", format!("{executable} is not installed or executable; install and authenticate it in your terminal.")))?;
    let path = path.canonicalize().map_err(io_error)?;
    if !path.is_file() {
        return Err(AiError::new(
            "cli_not_found",
            "AI executable is not a regular file.",
        ));
    }
    Ok(path)
}
fn private_run_dir() -> Result<tempfile::TempDir, AiError> {
    let temp = tempfile::Builder::new()
        .prefix("architalk-run-")
        .tempdir()
        .map_err(io_error)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        let meta = std::fs::symlink_metadata(temp.path()).map_err(io_error)?;
        if !meta.is_dir()
            || meta.file_type().is_symlink()
            || meta.uid() != unsafe { libc::geteuid() }
        {
            return Err(AiError::new(
                "restricted_execution_unavailable",
                "Run directory ownership is not safe.",
            ));
        }
        std::fs::set_permissions(temp.path(), std::fs::Permissions::from_mode(0o700))
            .map_err(io_error)?;
    }
    Ok(temp)
}
fn runtime_error(error: impl std::fmt::Display) -> AiError {
    AiError::new(
        "restricted_execution_unavailable",
        format!("Private project runtime is unavailable: {error}. No model was called."),
    )
}

struct OwnedRunDirectory {
    project: PathBuf,
    path: PathBuf,
    name: String,
    root: Directory,
    state: Directory,
    runtime: Directory,
    run: Directory,
}
impl OwnedRunDirectory {
    fn create(project: &Path) -> Result<Self, AiError> {
        let project = project.canonicalize().map_err(io_error)?;
        let root = Directory::root(&project).map_err(runtime_error)?;
        let ensure = |parent: &Directory, name: &str| -> Result<Directory, AiError> {
            match parent.mkdir(name, 0o700) {
                Ok(()) => {}
                Err(error)
                    if error
                        .downcast_ref::<std::io::Error>()
                        .is_some_and(|error| error.kind() == std::io::ErrorKind::AlreadyExists) => {
                }
                Err(error) => return Err(runtime_error(error)),
            }
            let directory = parent.open_dir(name).map_err(runtime_error)?;
            directory.make_private().map_err(runtime_error)?;
            Ok(directory)
        };
        // Store owns state creation. A deleted workspace must never be revived
        // by a delayed diagnostic or generation task.
        let state = root.open_dir(".architalk").map_err(runtime_error)?;
        state.make_private().map_err(runtime_error)?;
        let runtime = ensure(&state, "runtime")?;
        let name = format!("run-{}", uuid::Uuid::new_v4());
        runtime.mkdir(&name, 0o700).map_err(runtime_error)?;
        let run = runtime.open_dir(&name).map_err(runtime_error)?;
        run.make_private().map_err(runtime_error)?;
        let path = project.join(".architalk/runtime").join(&name);
        let owned = Self {
            project,
            path,
            name,
            root,
            state,
            runtime,
            run,
        };
        owned.validate()?;
        for directory in ["tmp", "codex-state", "log", "cache", "codex-home"] {
            owned.run.mkdir(directory, 0o700).map_err(runtime_error)?;
        }
        Ok(owned)
    }
    fn validate(&self) -> Result<(), AiError> {
        let root = Directory::root(&self.project).map_err(runtime_error)?;
        let state = root.open_dir(".architalk").map_err(runtime_error)?;
        let runtime = state.open_dir("runtime").map_err(runtime_error)?;
        let run = runtime.open_dir(&self.name).map_err(runtime_error)?;
        for (actual, expected) in [
            (&root, &self.root),
            (&state, &self.state),
            (&runtime, &self.runtime),
            (&run, &self.run),
        ] {
            if actual.identity().map_err(runtime_error)?
                != expected.identity().map_err(runtime_error)?
            {
                return Err(runtime_error("directory identity changed"));
            }
        }
        Ok(())
    }
    fn link_codex_auth(&self, shared_home: &Path) -> Result<bool, AiError> {
        use std::{
            ffi::CString,
            os::unix::{
                ffi::OsStrExt,
                fs::{MetadataExt, PermissionsExt},
            },
        };
        self.validate()?;
        let shared_home = match shared_home.canonicalize() {
            Ok(path) => path,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
            Err(error) => return Err(runtime_error(error)),
        };
        let shared = Directory::root(&shared_home).map_err(runtime_error)?;
        let auth = match shared.open_file("auth.json", false) {
            Ok(file) => file,
            Err(error)
                if error
                    .downcast_ref::<std::io::Error>()
                    .is_some_and(|error| error.kind() == std::io::ErrorKind::NotFound) =>
            {
                return Ok(false);
            }
            Err(_) => {
                return Err(runtime_error(
                    "shared Codex authentication must be a regular, non-symlink file",
                ));
            }
        };
        let metadata = auth.metadata().map_err(io_error)?;
        if !metadata.is_file()
            || metadata.uid() != unsafe { libc::geteuid() }
            || metadata.permissions().mode() & 0o077 != 0
            || metadata.len() == 0
            || metadata.len() > FRAME_LIMIT as u64
        {
            return Err(runtime_error(
                "shared Codex authentication ownership, permissions or size is unsafe",
            ));
        }
        let target = shared_home.join("auth.json");
        let local_home = self.run.open_dir("codex-home").map_err(runtime_error)?;
        let destination = self.path.join("codex-home/auth.json");
        match std::fs::read_link(&destination) {
            Ok(existing) if existing == target => return Ok(true),
            Ok(_) => return Err(runtime_error("private Codex authentication link changed")),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(runtime_error(error)),
        }
        let target = CString::new(target.as_os_str().as_bytes()).map_err(runtime_error)?;
        let name = CString::new("auth.json").unwrap();
        if unsafe { libc::symlinkat(target.as_ptr(), local_home.raw_fd(), name.as_ptr()) } != 0 {
            return Err(runtime_error(std::io::Error::last_os_error()));
        }
        Ok(true)
    }

    fn write_private(&self, name: &str, content: &[u8]) -> Result<(), AiError> {
        use std::io::Write;
        self.validate()?;
        let mut file = self.run.create_private_file(name).map_err(runtime_error)?;
        file.write_all(content).map_err(io_error)?;
        Ok(())
    }
}
impl Drop for OwnedRunDirectory {
    fn drop(&mut self) {
        if self.validate().is_ok() {
            if let Ok(identity) = self.run.identity() {
                if self
                    .runtime
                    .remove_private_tree(&self.name, &identity)
                    .is_err()
                {
                    tracing::warn!(
                        phase = "runtime_cleanup",
                        "Private run cleanup failed; original retained"
                    );
                }
            }
        } else {
            tracing::warn!(
                phase = "runtime_cleanup",
                "Private run directory identity changed; retained without external cleanup"
            );
        }
    }
}

enum RunDirectory {
    Generic(tempfile::TempDir),
    Project(OwnedRunDirectory),
}
impl RunDirectory {
    fn create(project: Option<&Path>) -> Result<Self, AiError> {
        match project {
            Some(project) => OwnedRunDirectory::create(project).map(Self::Project),
            None => private_run_dir().map(Self::Generic),
        }
    }
    fn path(&self) -> &Path {
        match self {
            Self::Generic(temp) => temp.path(),
            Self::Project(run) => &run.path,
        }
    }
    fn configure_codex_home(&self, command: &mut Command) -> Result<(), AiError> {
        if let Self::Project(run) = self {
            let shared_home = std::env::var_os("CODEX_HOME")
                .map(PathBuf::from)
                .or_else(|| {
                    std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".codex"))
                });
            if let Some(shared_home) = shared_home {
                run.link_codex_auth(&shared_home)?;
            }
            command.env("CODEX_HOME", run.path.join("codex-home"));
        }
        Ok(())
    }

    fn command(&self, executable: impl AsRef<std::ffi::OsStr>) -> Result<Command, AiError> {
        let mut command = base_command(executable, self.path());
        if let Self::Project(run) = self {
            run.validate()?;
            let fd = run.run.raw_fd();
            // The handle outlives the child and makes cwd independent of path swaps.
            unsafe {
                command.pre_exec(move || {
                    if libc::fchdir(fd) == 0 {
                        Ok(())
                    } else {
                        Err(std::io::Error::last_os_error())
                    }
                });
            }
            command.env("TMPDIR", run.path.join("tmp"));
            command.env("CLAUDE_CODE_TMPDIR", run.path.join("tmp"));
            command.env("CODEX_SQLITE_HOME", run.path.join("codex-state"));
            command.env("XDG_CACHE_HOME", run.path.join("cache"));
        }
        command.env("CLAUDE_CODE_SAFE_MODE", "1");
        command.env("DISABLE_AUTOUPDATER", "1");
        command.env("CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC", "1");
        Ok(command)
    }
}

fn base_command(executable: impl AsRef<std::ffi::OsStr>, cwd: &Path) -> Command {
    let mut command = Command::new(executable);
    command
        .current_dir(cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    // Prevent this application's agent/session environment becoming CLI settings.
    // Authentication continues through the CLI's own normal HOME/keychain paths.
    let allowed = [
        "HOME",
        "USER",
        "LOGNAME",
        "PATH",
        "SHELL",
        "TMPDIR",
        "LANG",
        "LC_ALL",
        "SSL_CERT_FILE",
        "SSL_CERT_DIR",
        "HTTP_PROXY",
        "HTTPS_PROXY",
        "ALL_PROXY",
        "NO_PROXY",
        "http_proxy",
        "https_proxy",
        "all_proxy",
        "no_proxy",
        "CODEX_HOME",
        "ANTHROPIC_API_KEY",
        "ANTHROPIC_AUTH_TOKEN",
        "CLAUDE_CODE_OAUTH_TOKEN",
        "OPENAI_API_KEY",
        "OPENAI_BASE_URL",
    ];
    let env: Vec<_> = allowed
        .iter()
        .filter_map(|key| std::env::var_os(key).map(|value| (*key, value)))
        .collect();
    command.env_clear().envs(env);
    #[cfg(unix)]
    {
        command.process_group(0);
    }
    command
}

struct GroupGuard {
    pid: u32,
    armed: bool,
}
impl Drop for GroupGuard {
    fn drop(&mut self) {
        if self.armed {
            signal_group(self.pid, libc::SIGKILL);
        }
    }
}
fn signal_group(pid: u32, signal: i32) {
    #[cfg(unix)]
    unsafe {
        libc::kill(-(pid as i32), signal);
    }
    #[cfg(not(unix))]
    {
        let _ = (pid, signal);
    }
}
fn group_alive(pid: u32) -> bool {
    #[cfg(unix)]
    unsafe {
        if libc::kill(-(pid as i32), 0) == 0 {
            return true;
        }
        std::io::Error::last_os_error().raw_os_error() != Some(libc::ESRCH)
    }
    #[cfg(not(unix))]
    {
        let _ = pid;
        true
    }
}
async fn stop_group(child: &mut Child, guard: &mut GroupGuard) -> bool {
    signal_group(guard.pid, libc::SIGTERM);
    let deadline = tokio::time::Instant::now() + GRACE;
    loop {
        let _ = child.try_wait();
        if !group_alive(guard.pid) {
            guard.armed = false;
            return true;
        }
        if tokio::time::Instant::now() >= deadline {
            break;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    signal_group(guard.pid, libc::SIGKILL);
    let _ = tokio::time::timeout(GRACE, child.wait()).await;
    let deadline = tokio::time::Instant::now() + GRACE;
    loop {
        if !group_alive(guard.pid) {
            guard.armed = false;
            return true;
        }
        if tokio::time::Instant::now() >= deadline {
            return false;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
}
async fn capture_info(
    executable: &Path,
    args: &[&str],
    run: &RunDirectory,
    backend: Backend,
) -> Result<String, AiError> {
    let mut command = if matches!(run, RunDirectory::Project(_)) {
        let profile = sandbox_profile(run.path())?;
        if let RunDirectory::Project(owned) = run {
            if !run.path().join("diagnostic.sb").exists() {
                owned.write_private("diagnostic.sb", profile.as_bytes())?;
            }
        }
        let mut command = run.command("/usr/bin/sandbox-exec")?;
        command
            .arg("-f")
            .arg(run.path().join("diagnostic.sb"))
            .arg(executable);
        command
    } else {
        run.command(executable)?
    };
    if backend == Backend::Codex {
        run.configure_codex_home(&mut command)?;
    }
    let mut child = command.args(args).spawn().map_err(io_error)?;
    let mut guard = GroupGuard {
        pid: child
            .id()
            .ok_or_else(|| AiError::new("cli_spawn_failed", "No child PID."))?,
        armed: true,
    };
    drop(child.stdin.take());
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();
    let out = read_small(stdout, 256 * 1024);
    let err = read_small(stderr, 256 * 1024);
    let result = tokio::time::timeout(Duration::from_secs(5), async {
        let (out, _err, status) =
            tokio::try_join!(out, err, async { child.wait().await.map_err(io_error) })?;
        if !status.success() {
            return Err(AiError::new(
                "cli_diagnostic_failed",
                "AI CLI version/help command failed.",
            ));
        }
        String::from_utf8(out).map_err(|_| {
            AiError::new(
                "cli_diagnostic_failed",
                "AI CLI diagnostic output is not UTF-8.",
            )
        })
    })
    .await;
    let stopped = stop_group(&mut child, &mut guard).await;
    if !stopped {
        return Err(AiError {
            code: "process_termination_unknown".into(),
            message: "Diagnostic child process termination could not be confirmed.".into(),
            termination_unknown: true,
        });
    }
    result.map_err(|_| {
        AiError::new(
            "cli_diagnostic_timeout",
            "AI CLI diagnostic exceeded 5 seconds.",
        )
    })?
}
async fn read_small(mut reader: impl AsyncRead + Unpin, cap: usize) -> Result<Vec<u8>, AiError> {
    let mut result = Vec::new();
    let mut chunk = [0u8; 4096];
    loop {
        let count = reader.read(&mut chunk).await.map_err(io_error)?;
        if count == 0 {
            return Ok(result);
        }
        if result.len() + count > cap {
            return Err(AiError::new(
                "output_limit",
                "CLI diagnostic output exceeded its limit.",
            ));
        }
        result.extend_from_slice(&chunk[..count]);
    }
}
fn validate_spec(spec: &RunSpec) -> Result<(), AiError> {
    if spec.timeout_seconds == 0 || spec.timeout_seconds > 600 {
        return Err(AiError::new(
            "invalid_settings",
            "Timeout must be between 1 and 600 seconds.",
        ));
    }
    if spec.prompt.is_empty() || spec.prompt.len() > FRAME_LIMIT {
        return Err(AiError::new(
            "invalid_prompt",
            "Prompt must contain 1–1048576 UTF-8 bytes.",
        ));
    }
    if let Some(model) = &spec.model {
        if model.is_empty()
            || model.len() > 200
            || model.starts_with('-')
            || !model
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || "-_.:/[]".contains(c))
        {
            return Err(AiError::new(
                "invalid_settings",
                "Model ID has unsupported characters or length.",
            ));
        }
    }
    if let Some(effort) = &spec.effort {
        // Effort support is decided by the CLI/model, not these known examples.
        // A safe token also cannot escape Codex's quoted TOML override argument.
        if effort.is_empty()
            || effort.len() > 32
            || effort.starts_with('-')
            || !effort
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"_-".contains(&c))
        {
            return Err(AiError::new(
                "invalid_settings",
                "Effort must be a 1–32 character ASCII token using letters, digits, '_' or '-', without a leading '-'.",
            ));
        }
    }
    if spec.profile.is_some() {
        return Err(AiError::new(
            "restricted_execution_unavailable",
            "Named profiles are disabled because their merged permissions are not verified. Select model and effort explicitly.",
        ));
    }
    Ok(())
}
fn cli_args(spec: &RunSpec) -> Vec<String> {
    let mut args: Vec<String> = match spec.backend {
        Backend::Claude => [
            "-p",
            "--output-format",
            "stream-json",
            "--verbose",
            "--include-partial-messages",
            "--no-session-persistence",
            "--safe-mode",
            "--restricted",
            "--tools",
            "",
            "--strict-mcp-config",
            "--mcp-config",
            "{\"mcpServers\":{}}",
            "--disable-slash-commands",
            "--permission-mode",
            "dontAsk",
            "--setting-sources",
            "",
            "--settings",
            "{\"disableAllHooks\":true}",
            "--max-turns",
            "1",
        ]
        .iter()
        .map(|s| (*s).to_owned())
        .collect(),
        Backend::Codex => [
            "--no-daemon",
            "-a",
            "never",
            "-s",
            "read-only",
            "exec",
            "--ignore-user-config",
            "--ignore-rules",
            "--skip-git-repo-check",
            "--ephemeral",
            "--json",
            "--color",
            "never",
            "-c",
            "project_doc_max_bytes=0",
            "-c",
            "web_search=\"disabled\"",
            "-c",
            "developer_instructions=\"\"",
            "-c",
            "cli_auth_credentials_store=\"file\"",
        ]
        .iter()
        .map(|s| (*s).to_owned())
        .collect(),
    };
    if spec.backend == Backend::Codex {
        for feature in CODEX_DISABLED {
            args.extend(["--disable".to_owned(), (*feature).to_owned()]);
        }
    }
    if let Some(model) = &spec.model {
        args.extend(["--model".to_owned(), model.clone()]);
    }
    if let Some(effort) = &spec.effort {
        match spec.backend {
            Backend::Claude => args.extend(["--effort".to_owned(), effort.clone()]),
            Backend::Codex => args.extend([
                "-c".to_owned(),
                format!("model_reasoning_effort=\"{effort}\""),
            ]),
        }
    }
    if spec.backend == Backend::Codex {
        args.push("-".to_owned());
    }
    args
}
fn sandbox_profile(runtime: &Path) -> Result<String, AiError> {
    let path = runtime.to_str().ok_or_else(|| {
        AiError::new(
            "restricted_execution_unavailable",
            "Runtime path must be valid UTF-8 for OS sandboxing.",
        )
    })?;
    // JSON string escaping matches Seatbelt quoted-path escaping for these
    // printable paths. Control bytes are rejected instead of interpreted.
    if path.chars().any(char::is_control) {
        return Err(AiError::new(
            "restricted_execution_unavailable",
            "Runtime path contains unsupported control characters.",
        ));
    }
    let escaped = serde_json::to_string(path).map_err(|_| {
        AiError::new(
            "restricted_execution_unavailable",
            "Unable to encode project root.",
        )
    })?;
    Ok(format!(
        "(version 1)\n(allow default)\n(deny file-write*)\n(allow file-write* (subpath {escaped}) (literal \"/dev/null\"))\n"
    ))
}

/// Runs exactly one paid attempt; never retries or switches model/provider.
/// Cancellation returns only after process-group termination and pipe cleanup,
/// or explicitly reports `termination_unknown` so the controller keeps its lock.
pub async fn generate(
    spec: RunSpec,
    cancel: CancellationToken,
    events: mpsc::Sender<AiEvent>,
) -> Result<AiOutput, AiError> {
    validate_spec(&spec)?;
    if cancel.is_cancelled() {
        return Err(AiError::new(
            "cancelled",
            "Generation was cancelled before starting.",
        ));
    }
    let diagnostic = diagnose_in_project(spec.backend, &spec.executable, &spec.project_root).await;
    if !diagnostic.compatible {
        return Err(diagnostic_failure(diagnostic));
    }
    if cancel.is_cancelled() {
        return Err(AiError::new(
            "cancelled",
            "Generation was cancelled before starting.",
        ));
    }
    let project = spec.project_root.canonicalize().map_err(io_error)?;
    if !project.is_dir() {
        return Err(AiError::new(
            "invalid_project",
            "Project root is not a directory.",
        ));
    }
    let run = RunDirectory::create(Some(&project))?;
    let cwd = run.path();
    let profile_path = cwd.join("runtime-write-only.sb");
    if let RunDirectory::Project(owned) = &run {
        owned.write_private("runtime-write-only.sb", sandbox_profile(cwd)?.as_bytes())?;
    }
    verify_write_denial(&run, &profile_path, &project).await?;
    let executable = executable_path(&spec.executable)?;
    let mut command = run.command("/usr/bin/sandbox-exec")?;
    let mut args = cli_args(&spec);
    if spec.backend == Backend::Codex {
        args.pop();
    }
    command
        .arg("-f")
        .arg(&profile_path)
        .arg(executable)
        .args(args);
    if spec.backend == Backend::Codex {
        run.configure_codex_home(&mut command)?;
        for (key, value) in [
            ("sqlite_home", cwd.join("codex-state")),
            ("log_dir", cwd.join("log")),
        ] {
            command.arg("-c").arg(format!(
                "{key}={}",
                serde_json::to_string(&value.to_string_lossy()).map_err(runtime_error)?
            ));
        }
        command
            .arg("-c")
            .arg("history.persistence=\"none\"")
            .arg("-");
    }
    let mut child = command.spawn().map_err(io_error)?;
    let mut guard = GroupGuard {
        pid: child
            .id()
            .ok_or_else(|| AiError::new("cli_spawn_failed", "No CLI process PID."))?,
        armed: true,
    };
    let started = Instant::now();
    let total = Arc::new(AtomicUsize::new(0));
    let valid_at = Arc::new(AtomicU64::new(0));
    let mut stdout = tokio::spawn(read_stdout(
        child.stdout.take().unwrap(),
        spec.backend,
        total.clone(),
        valid_at.clone(),
        started,
        events.clone(),
    ));
    let mut stderr = tokio::spawn(read_stderr(child.stderr.take().unwrap(), total));
    let mut stdin = child.stdin.take().unwrap();
    let prompt = spec.prompt.into_bytes();
    let mut writer = tokio::spawn(async move {
        stdin.write_all(&prompt).await.map_err(io_error)?;
        stdin.shutdown().await.map_err(io_error)
    });
    let deadline = tokio::time::sleep(Duration::from_secs(spec.timeout_seconds));
    tokio::pin!(deadline);
    let mut interval = tokio::time::interval(Duration::from_secs(5));
    let mut delayed = false;
    let mut parsed = None;
    let mut retained = None;
    let mut stdout_done = false;
    let mut stderr_done = false;
    let mut write_done = false;
    let mut status = None;
    let mut failure = None;
    loop {
        if stdout_done && stderr_done && status.is_some() && write_done {
            break;
        }
        tokio::select! {
            biased;
            _ = cancel.cancelled() => { failure = Some(AiError::new("cancelled", "Generation was cancelled.")); break; }
            _ = &mut deadline => { failure = Some(AiError::new("timeout", "AI CLI reached its wall-time limit.")); break; }
            result = &mut stdout, if !stdout_done => {
                stdout_done = true;
                match task_result(result) { Ok(value) => parsed = Some(value), Err(error) => { failure = Some(error); break; } }
            },
            result = &mut stderr, if !stderr_done => {
                stderr_done = true;
                match task_result(result) { Ok(value) => retained = Some(value), Err(error) => { failure = Some(error); break; } }
            },
            result = &mut writer, if !write_done => {
                write_done = true;
                match task_result(result) { Ok(()) => {}, Err(error) => { failure = Some(error); break; } }
            },
            result = child.wait(), if status.is_none() => {
                match result { Ok(value) => status = Some(value), Err(error) => { failure = Some(io_error(error)); break; } }
                // Ensure surviving descendants cannot retain pipe ownership or
                // continue a background attempt after their CLI parent exits.
                signal_group(guard.pid, libc::SIGTERM);
            },
            _ = interval.tick() => {
                let quiet = started.elapsed().as_secs().saturating_sub(valid_at.load(Ordering::Relaxed));
                if quiet >= 60 && !delayed {
                    let _ = events.try_send(AiEvent::Progress { message: "AI CLI has emitted no valid event for 60 seconds; waiting within the configured timeout.".to_owned() });
                    delayed = true;
                }
                if quiet < 60 { delayed = false; }
            }
        }
    }
    let stopped = stop_group(&mut child, &mut guard).await;
    // Join handles are bounded on all terminal paths. A pipe held by a process
    // outside the known group never leaves this future blocked indefinitely.
    let mut drains_clean = true;
    if !stdout_done {
        drains_clean &= cleanup_task(&mut stdout).await;
    }
    if !stderr_done {
        match tokio::time::timeout(GRACE, &mut stderr).await {
            Ok(result) => match task_result(result) {
                Ok(bytes) => retained = Some(bytes),
                Err(error) => {
                    failure.get_or_insert(error);
                }
            },
            Err(_) => {
                stderr.abort();
                let _ = stderr.await;
                drains_clean = false;
            }
        }
    }
    if !write_done {
        drains_clean &= cleanup_task(&mut writer).await;
    }
    if !stopped || !drains_clean {
        let mut error = failure.unwrap_or_else(|| {
            AiError::new(
                "process_termination_unknown",
                "AI process termination or pipe cleanup could not be confirmed.",
            )
        });
        error.termination_unknown = true;
        error.message.push_str(if !stopped {
            " Process-group termination could not be confirmed."
        } else {
            " I/O pipe cleanup could not be confirmed."
        });
        tracing::warn!(
            pid = guard.pid,
            group_stopped = stopped,
            drains_clean,
            "AI termination requires manual confirmation"
        );
        return Err(error);
    }
    if let Some(mut error) = failure {
        error.message.push_str(&format!(
            " [stderr: {}]",
            failure_line_summaries(retained.as_deref().unwrap_or_default())
        ));
        return Err(error);
    }
    if !status.unwrap().success() {
        let diagnostic_text = String::from_utf8_lossy(retained.as_ref().unwrap()).to_lowercase();
        let auth = [
            "unauthorized",
            "not logged in",
            "authentication",
            "login required",
            "token expired",
            "401",
        ]
        .iter()
        .any(|marker| diagnostic_text.contains(marker));
        return Err(AiError::new(
            if auth {
                "authentication_required"
            } else {
                "cli_failed"
            },
            if auth {
                "AI CLI authentication failed. Use the installed CLI's official login command in your terminal, then explicitly retry."
            } else {
                "AI CLI exited unsuccessfully. Inspect the installed CLI in your terminal and explicitly retry; no automatic retry was made."
            }.to_owned() + &format!(" [stderr: {}; exit={:?}]", failure_line_summaries(retained.as_deref().unwrap_or_default()), status.unwrap().code()),
        ));
    }
    let parsed = parsed.unwrap();
    let text = parsed.finish()?;
    Ok(AiOutput {
        text,
        cli_version: diagnostic.version.unwrap(),
        resolved_model: parsed.model,
    })
}
fn task_result<T>(value: Result<Result<T, AiError>, tokio::task::JoinError>) -> Result<T, AiError> {
    value.map_err(|_| AiError::new("cli_io_error", "AI I/O task stopped unexpectedly."))?
}
async fn cleanup_task<T>(task: &mut JoinHandle<T>) -> bool {
    if tokio::time::timeout(GRACE, &mut *task).await.is_ok() {
        true
    } else {
        task.abort();
        let _ = task.await;
        false
    }
}
async fn verify_write_denial(
    run: &RunDirectory,
    profile: &Path,
    project: &Path,
) -> Result<(), AiError> {
    let target = project.join(format!(".architalk-denied-probe-{}", uuid::Uuid::new_v4()));
    let mut child = run
        .command("/usr/bin/sandbox-exec")?
        .arg("-f")
        .arg(profile)
        .arg("/usr/bin/touch")
        .arg(&target)
        .spawn()
        .map_err(io_error)?;
    let mut guard = GroupGuard {
        pid: child.id().unwrap(),
        armed: true,
    };
    drop(child.stdin.take());
    let output = tokio::time::timeout(Duration::from_secs(3), child.wait()).await;
    let stopped = stop_group(&mut child, &mut guard).await;
    write_denial_outcome(
        stopped,
        matches!(output, Ok(Ok(status)) if !status.success()),
        target.exists(),
    )
}
fn write_denial_outcome(
    stopped: bool,
    nonzero_exit: bool,
    target_exists: bool,
) -> Result<(), AiError> {
    if !stopped {
        return Err(AiError {
            code: "process_termination_unknown".into(),
            message:
                "OS sandbox probe process termination could not be confirmed; no model was called."
                    .into(),
            termination_unknown: true,
        });
    }
    if nonzero_exit && !target_exists {
        Ok(())
    } else {
        Err(AiError::new(
            "restricted_execution_unavailable",
            "OS sandbox project-write denial probe failed; no model was called.",
        ))
    }
}
fn count_bytes(total: &AtomicUsize, count: usize) -> Result<(), AiError> {
    if total
        .fetch_add(count, Ordering::Relaxed)
        .saturating_add(count)
        > TOTAL_LIMIT
    {
        Err(AiError::new(
            "output_limit",
            "AI CLI output exceeded 16 MiB.",
        ))
    } else {
        Ok(())
    }
}
async fn read_stderr(
    mut reader: impl AsyncRead + Unpin,
    total: Arc<AtomicUsize>,
) -> Result<Vec<u8>, AiError> {
    let mut retained = Vec::new();
    let mut chunk = [0u8; 8192];
    loop {
        let count = reader.read(&mut chunk).await.map_err(io_error)?;
        if count == 0 {
            return Ok(retained);
        }
        count_bytes(&total, count)?;
        let keep = count.min(STDERR_RETAIN - retained.len());
        retained.extend_from_slice(&chunk[..keep]);
    }
}
async fn read_stdout(
    mut reader: impl AsyncRead + Unpin,
    backend: Backend,
    total: Arc<AtomicUsize>,
    valid_at: Arc<AtomicU64>,
    started: Instant,
    events: mpsc::Sender<AiEvent>,
) -> Result<Parsed, AiError> {
    let mut parser = Parsed::new(backend);
    let mut frame = Vec::new();
    let mut chunk = [0u8; 8192];
    loop {
        let count = reader.read(&mut chunk).await.map_err(io_error)?;
        if count == 0 {
            break;
        }
        count_bytes(&total, count)?;
        for byte in &chunk[..count] {
            if *byte == b'\n' {
                if frame.last() == Some(&b'\r') {
                    frame.pop();
                }
                if !frame.is_empty() {
                    let event = parser.line(&frame)?;
                    valid_at.store(started.elapsed().as_secs(), Ordering::Relaxed);
                    if let Some(event) = event {
                        let _ = events.try_send(event);
                    }
                }
                frame.clear();
            } else {
                if frame.len() == FRAME_LIMIT {
                    return Err(AiError::new(
                        "output_limit",
                        "AI CLI JSONL frame exceeded 1 MiB.",
                    ));
                }
                frame.push(*byte);
            }
        }
    }
    if !frame.is_empty() {
        return Err(AiError::new(
            "invalid_cli_output",
            "AI CLI ended with an incomplete JSONL frame.",
        ));
    }
    Ok(parser)
}

struct Parsed {
    backend: Backend,
    terminal: bool,
    text: Option<String>,
    model: Option<String>,
    initialized: bool,
    turn_started: bool,
}
impl Parsed {
    fn new(backend: Backend) -> Self {
        Self {
            backend,
            terminal: false,
            text: None,
            model: None,
            initialized: false,
            turn_started: false,
        }
    }
    fn invalid(message: &str) -> AiError {
        AiError::new("invalid_cli_output", message)
    }
    fn safe_subtype(value: Option<&str>) -> &str {
        value
            .filter(|value| {
                !value.is_empty()
                    && value.len() <= 64
                    && value
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b == b'_')
            })
            .unwrap_or("unknown")
    }
    fn terminal_failure(event: &Value) -> AiError {
        let message = event.to_string().to_lowercase();
        let kind = match event["type"].as_str() {
            Some("turn.failed") => "turn.failed",
            Some("result") => "result",
            Some("error") => "error",
            _ => "unknown",
        };
        let summary = format!(
            " [stdout: event={kind}, {}]",
            failure_summary(message.as_bytes())
        );
        if [
            "unauthorized",
            "authentication",
            "not logged in",
            "login required",
            "token expired",
            "refresh token",
            "invalid api key",
            "/login",
            "401",
        ]
        .iter()
        .any(|marker| message.contains(marker))
        {
            AiError::new(
                "authentication_required",
                "AI CLI authentication failed. Use the installed CLI's official login procedure in your terminal, then explicitly retry.".to_owned() + &summary,
            )
        } else {
            AiError::new(
                "cli_failed",
                "AI CLI reported a failed result. Check CLI authentication/model settings in your terminal and explicitly retry.".to_owned() + &summary,
            )
        }
    }
    fn line(&mut self, bytes: &[u8]) -> Result<Option<AiEvent>, AiError> {
        let event: Value = serde_json::from_slice(bytes)
            .map_err(|_| Self::invalid("AI CLI emitted malformed or non-UTF-8 JSONL."))?;
        let kind = event
            .get("type")
            .and_then(Value::as_str)
            .ok_or_else(|| Self::invalid("AI CLI event has no type."))?;
        if self.terminal {
            return Err(Self::invalid(
                "AI CLI emitted events after a terminal event.",
            ));
        }
        match self.backend {
            Backend::Claude => self.claude(kind, &event),
            Backend::Codex => self.codex(kind, &event),
        }
    }
    fn claude(&mut self, kind: &str, event: &Value) -> Result<Option<AiEvent>, AiError> {
        match kind {
            "system" => match event["subtype"].as_str() {
                Some("init") => {
                    if self.initialized
                        || event["tools"].as_array().is_none_or(|x| !x.is_empty())
                        || event["mcp_servers"]
                            .as_array()
                            .is_none_or(|x| !x.is_empty())
                        || event["permissionMode"] != "dontAsk"
                    {
                        return Err(AiError::new(
                            "restricted_execution_unavailable",
                            "Claude initialized with unexpected tools, MCP, or permissions.",
                        ));
                    }
                    self.initialized = true;
                    self.model = event["model"].as_str().map(str::to_owned);
                    Ok(Some(AiEvent::Progress {
                        message: "Claude is generating a response.".to_owned(),
                    }))
                }
                Some("status") => Ok(None),
                Some("thinking_tokens") => {
                    // Verified 2.1.326 reasoning-progress telemetry, not model
                    // content or a terminal result. Never expose thinking text.
                    if !self.initialized
                        || event["estimated_tokens"].as_u64().is_none()
                        || event["estimated_tokens_delta"].as_u64().is_none()
                    {
                        return Err(Self::invalid(
                            "Malformed or out-of-order Claude thinking-token metadata.",
                        ));
                    }
                    Ok(None)
                }
                _ => Err(Self::invalid(&format!(
                    "Unsupported Claude system event: {}.",
                    Self::safe_subtype(event["subtype"].as_str())
                ))),
            },
            "stream_event" => {
                if !self.initialized {
                    return Err(Self::invalid("Claude content arrived before init."));
                }
                let inner = &event["event"];
                match inner["type"].as_str() {
                    Some("content_block_start") if inner["content_block"]["type"] == "tool_use" => {
                        Err(AiError::new(
                            "restricted_execution_unavailable",
                            "Claude attempted a tool call in text-only mode.",
                        ))
                    }
                    Some("content_block_delta") if inner["delta"]["type"] == "text_delta" => {
                        let text = inner["delta"]["text"]
                            .as_str()
                            .ok_or_else(|| Self::invalid("Claude text delta is missing text."))?;
                        Ok(Some(AiEvent::Delta {
                            text: text.to_owned(),
                        }))
                    }
                    Some(
                        "message_start"
                        | "content_block_start"
                        | "content_block_delta"
                        | "content_block_stop"
                        | "message_delta"
                        | "message_stop",
                    ) => Ok(None),
                    _ => Err(Self::invalid("Unsupported Claude stream event.")),
                }
            }
            "assistant" => {
                if !self.initialized {
                    return Err(Self::invalid("Claude message arrived before init."));
                }
                let blocks = event["message"]["content"]
                    .as_array()
                    .ok_or_else(|| Self::invalid("Claude assistant content is missing."))?;
                if blocks.iter().any(|block| block["type"] == "tool_use") {
                    return Err(AiError::new(
                        "restricted_execution_unavailable",
                        "Claude attempted a tool call.",
                    ));
                }
                let text: String = blocks
                    .iter()
                    .filter_map(|block| {
                        if block["type"] == "text" {
                            block["text"].as_str()
                        } else {
                            None
                        }
                    })
                    .collect();
                if !text.is_empty() {
                    self.text = Some(text);
                }
                Ok(None)
            }
            "result" => {
                if !self.initialized {
                    return Err(Self::invalid("Claude result arrived before init."));
                }
                if event["is_error"] != false || event["subtype"] != "success" {
                    return Err(Self::terminal_failure(event));
                }
                let result = event["result"]
                    .as_str()
                    .ok_or_else(|| Self::invalid("Claude success result has no final text."))?;
                if self.text.as_deref().is_some_and(|text| text != result) {
                    return Err(Self::invalid(
                        "Claude terminal and assistant final messages disagree.",
                    ));
                }
                self.text = Some(result.to_owned());
                self.terminal = true;
                Ok(None)
            }
            "rate_limit_event" => Ok(None),
            "error" => Err(Self::terminal_failure(event)),
            _ => Err(Self::invalid("Unsupported Claude event type.")),
        }
    }
    fn codex(&mut self, kind: &str, event: &Value) -> Result<Option<AiEvent>, AiError> {
        match kind {
            "thread.started" => {
                if self.initialized {
                    return Err(Self::invalid("Duplicate Codex thread."));
                }
                self.initialized = true;
                Ok(Some(AiEvent::Progress {
                    message: "Codex is generating a response.".to_owned(),
                }))
            }
            "turn.started" if self.initialized && !self.turn_started => {
                self.turn_started = true;
                Ok(None)
            }
            "item.started" | "item.updated" | "item.completed" if self.initialized => {
                let item = &event["item"];
                match item["type"].as_str() {
                    Some("agent_message") => {
                        if !self.turn_started {
                            return Err(Self::invalid(
                                "Codex agent message arrived before turn.started.",
                            ));
                        }
                        let text = item["text"]
                            .as_str()
                            .ok_or_else(|| Self::invalid("Codex agent message is missing text."))?;
                        if kind == "item.completed" {
                            self.text = Some(text.to_owned());
                            return Ok(Some(AiEvent::Delta {
                                text: text.to_owned(),
                            }));
                        }
                        Ok(None)
                    }
                    Some("reasoning") if self.turn_started => Ok(None),
                    Some("error") => {
                        // 0.169.0 emits this exact capability diagnostic before
                        // turn.started when code-mode host is intentionally off.
                        let message = item["message"].as_str().unwrap_or("");
                        if message.starts_with(
                            "Code Mode is unavailable because code-mode host is disabled.",
                        ) {
                            Ok(Some(AiEvent::Progress {
                                message: "Codex가 답변을 생성하고 있어요.".to_owned(),
                            }))
                        } else {
                            Err(AiError::new(
                                "cli_failed",
                                format!(
                                    "Codex reported an item error. [stdout: event=item.error, {}]",
                                    failure_summary(message.as_bytes())
                                ),
                            ))
                        }
                    }
                    Some(_) => Err(AiError::new(
                        "restricted_execution_unavailable",
                        "Codex emitted a tool/action item in text-only mode.",
                    )),
                    None => Err(Self::invalid("Codex item has no type.")),
                }
            }
            "turn.completed" if self.initialized && self.turn_started => {
                self.terminal = true;
                Ok(None)
            }
            "turn.failed" | "error" => Err(Self::terminal_failure(event)),
            _ => Err(Self::invalid(
                "Unsupported or out-of-order Codex event type.",
            )),
        }
    }
    fn finish(&self) -> Result<String, AiError> {
        if !self.terminal {
            return Err(Self::invalid(
                "AI CLI exited without a terminal success event.",
            ));
        }
        self.text
            .clone()
            .filter(|text| !text.trim().is_empty())
            .ok_or_else(|| Self::invalid("AI CLI terminal success has no final message."))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn feed(parser: &mut Parsed, value: Value) -> Result<Option<AiEvent>, AiError> {
        parser.line(&serde_json::to_vec(&value).unwrap())
    }
    fn init(parser: &mut Parsed) {
        let event = match parser.backend {
            Backend::Claude => {
                serde_json::json!({"type":"system","subtype":"init","tools":[],"mcp_servers":[],"permissionMode":"dontAsk","model":"test-model"})
            }
            Backend::Codex => serde_json::json!({"type":"thread.started","thread_id":"test"}),
        };
        feed(parser, event).unwrap();
        if parser.backend == Backend::Codex {
            feed(parser, serde_json::json!({"type":"turn.started"})).unwrap();
        }
    }
    #[test]
    fn parser_requires_terminal_and_final_text() {
        for backend in [Backend::Claude, Backend::Codex] {
            let mut parser = Parsed::new(backend);
            init(&mut parser);
            assert_eq!(parser.finish().unwrap_err().code, "invalid_cli_output");
            if backend == Backend::Claude {
                feed(&mut parser, serde_json::json!({"type":"assistant","message":{"content":[{"type":"text","text":"안녕"}]}})).unwrap();
                feed(&mut parser, serde_json::json!({"type":"result","subtype":"success","is_error":false,"result":"안녕"})).unwrap();
            } else {
                feed(&mut parser, serde_json::json!({"type":"item.completed","item":{"type":"agent_message","text":"안녕"}})).unwrap();
                feed(
                    &mut parser,
                    serde_json::json!({"type":"turn.completed","usage":{}}),
                )
                .unwrap();
            }
            assert_eq!(parser.finish().unwrap(), "안녕");
            assert!(feed(&mut parser, serde_json::json!({"type":"extra"})).is_err());
        }
    }
    #[test]
    fn parser_rejects_tool_actions_and_unknown_schemas() {
        let mut claude = Parsed::new(Backend::Claude);
        assert_eq!(feed(&mut claude, serde_json::json!({"type":"system","subtype":"init","tools":["Bash"],"mcp_servers":[],"permissionMode":"dontAsk"})).unwrap_err().code, "restricted_execution_unavailable");
        init(&mut claude);
        assert!(feed(&mut claude, serde_json::json!({"type":"assistant","message":{"content":[{"type":"tool_use","name":"Bash"}]}})).is_err());
        let mut codex = Parsed::new(Backend::Codex);
        init(&mut codex);
        assert_eq!(feed(&mut codex, serde_json::json!({"type":"item.completed","item":{"type":"command_execution","command":"touch x"}})).unwrap_err().code, "restricted_execution_unavailable");
        assert!(codex.line(b"not JSON").is_err());
        assert!(codex.line(b"{\"type\":\"future.schema\"}").is_err());
    }
    #[test]
    fn parser_rejects_disagreeing_terminal_text_and_terminal_failures() {
        let mut parser = Parsed::new(Backend::Claude);
        init(&mut parser);
        feed(&mut parser, serde_json::json!({"type":"assistant","message":{"content":[{"type":"text","text":"first"}]}})).unwrap();
        assert!(feed(&mut parser, serde_json::json!({"type":"result","subtype":"success","is_error":false,"result":"changed"})).is_err());
        let mut parser = Parsed::new(Backend::Codex);
        init(&mut parser);
        assert_eq!(
            feed(
                &mut parser,
                serde_json::json!({"type":"turn.failed","error":{"message":"bad"}})
            )
            .unwrap_err()
            .code,
            "cli_failed"
        );
    }
    #[test]
    fn codex_known_startup_notice_is_not_terminal_success() {
        let mut parser = Parsed::new(Backend::Codex);
        init(&mut parser);
        assert!(feed(&mut parser, serde_json::json!({"type":"item.completed","item":{"type":"error","message":"Code Mode is unavailable because code-mode host is disabled. Code mode will fail closed; enable `features.code_mode_host`."}})).unwrap().is_some());
        assert!(parser.finish().is_err());
        assert!(feed(&mut parser, serde_json::json!({"type":"item.completed","item":{"type":"error","message":"model failed"}})).is_err());
    }
    #[tokio::test]
    async fn stdout_handles_fragmented_utf8_crlf_and_rejects_truncation() {
        let jsonl = "{\"type\":\"thread.started\"}\r\n{\"type\":\"turn.started\"}\r\n{\"type\":\"item.completed\",\"item\":{\"type\":\"agent_message\",\"text\":\"한글\"}}\r\n{\"type\":\"turn.completed\"}\r\n";
        let (mut write, read) = tokio::io::duplex(16);
        tokio::spawn(async move {
            for byte in jsonl.as_bytes() {
                write.write_all(&[*byte]).await.unwrap();
            }
        });
        let (tx, _rx) = mpsc::channel(1);
        let result = read_stdout(
            read,
            Backend::Codex,
            Arc::new(AtomicUsize::new(0)),
            Arc::new(AtomicU64::new(0)),
            Instant::now(),
            tx,
        )
        .await
        .unwrap();
        assert_eq!(result.finish().unwrap(), "한글");
        let (tx, _rx) = mpsc::channel(1);
        let error = read_stdout(
            &b"{\"type\":\"thread.started\"}"[..],
            Backend::Codex,
            Arc::new(AtomicUsize::new(0)),
            Arc::new(AtomicU64::new(0)),
            Instant::now(),
            tx,
        )
        .await
        .err()
        .unwrap();
        assert_eq!(error.code, "invalid_cli_output");
    }
    #[tokio::test]
    async fn stdout_and_stderr_have_enforced_byte_limits() {
        let (tx, _rx) = mpsc::channel(1);
        let oversized = vec![b'a'; FRAME_LIMIT + 1];
        let error = read_stdout(
            oversized.as_slice(),
            Backend::Codex,
            Arc::new(AtomicUsize::new(0)),
            Arc::new(AtomicU64::new(0)),
            Instant::now(),
            tx,
        )
        .await
        .err()
        .unwrap();
        assert_eq!(error.code, "output_limit");
        let error = read_stderr(&b"xx"[..], Arc::new(AtomicUsize::new(TOTAL_LIMIT - 1)))
            .await
            .unwrap_err();
        assert_eq!(error.code, "output_limit");
        let bytes = vec![b'x'; STDERR_RETAIN + 99];
        assert_eq!(
            read_stderr(bytes.as_slice(), Arc::new(AtomicUsize::new(0)))
                .await
                .unwrap()
                .len(),
            STDERR_RETAIN
        );
    }
    #[test]
    fn command_arguments_preserve_backend_specific_boundaries() {
        let mut spec = RunSpec {
            backend: Backend::Codex,
            executable: "codex".into(),
            model: Some("model-id".into()),
            effort: Some("high".into()),
            profile: None,
            timeout_seconds: 180,
            prompt: "untrusted $(touch x); --dangerously-bypass-approvals-and-sandbox".into(),
            project_root: PathBuf::from("/project"),
        };
        validate_spec(&spec).unwrap();
        let args = cli_args(&spec);
        assert!(
            args.windows(2)
                .any(|a| a == ["-c", "model_reasoning_effort=\"high\""])
        );
        assert!(args.contains(&"exec".into()));
        assert_eq!(args.last().unwrap(), "-");
        assert!(!args.contains(&spec.prompt));
        assert!(!args.contains(&"-p".into()));
        spec.backend = Backend::Claude;
        let args = cli_args(&spec);
        assert_eq!(args[0], "-p");
        assert!(args.windows(2).any(|a| a == ["--tools", ""]));
        assert!(args.windows(2).any(|a| a == ["--effort", "high"]));
        spec.model = Some("vendor/future-model:2[1m]".into());
        for backend in [Backend::Claude, Backend::Codex] {
            spec.backend = backend;
            for effort in ["max", "ultra", "future_2-high", &"x".repeat(32)] {
                spec.effort = Some(effort.into());
                validate_spec(&spec).unwrap();
                let args = cli_args(&spec);
                assert!(
                    args.windows(2)
                        .any(|a| a == ["--model", "vendor/future-model:2[1m]"])
                );
                match backend {
                    Backend::Claude => assert!(args.windows(2).any(|a| a == ["--effort", effort])),
                    Backend::Codex => {
                        let assignment = &args
                            .windows(2)
                            .find(|a| a[0] == "-c" && a[1].starts_with("model_reasoning_effort="))
                            .unwrap()[1];
                        let parsed: toml::Value = toml::from_str(assignment).unwrap();
                        assert_eq!(parsed.as_table().unwrap().len(), 1);
                        assert_eq!(parsed["model_reasoning_effort"].as_str(), Some(effort));
                    }
                }
            }
        }
        spec.profile = Some("unsafe".into());
        assert!(validate_spec(&spec).is_err());
        spec.profile = None;
        spec.model = Some("--bad".into());
        assert!(validate_spec(&spec).is_err());
    }
    #[test]
    fn effort_tokens_reject_argument_and_toml_injection() {
        let mut spec = RunSpec {
            backend: Backend::Codex,
            executable: "codex".into(),
            model: None,
            effort: None,
            profile: None,
            timeout_seconds: 180,
            prompt: "test".into(),
            project_root: PathBuf::from("/project"),
        };
        for effort in [
            "",
            "-high",
            "high level",
            "high\"",
            "high\\",
            "high\nmodel=\"other\"",
            "high\r",
            "high\t",
            "high\0",
            "high;touch",
            "$(touch)",
            "`touch`",
            "높음",
            &"x".repeat(33),
        ] {
            spec.effort = Some(effort.into());
            for backend in [Backend::Claude, Backend::Codex] {
                spec.backend = backend;
                assert_eq!(validate_spec(&spec).unwrap_err().code, "invalid_settings");
            }
        }
    }
    #[cfg(target_os = "macos")]
    #[tokio::test]
    #[ignore = "installed Codex startup diagnostic; no prompt and OS network denied, no model request"]
    async fn installed_codex_empty_input_startup_without_network() {
        installed_codex_startup_probe(false, false).await;
    }

    #[cfg(target_os = "macos")]
    #[tokio::test]
    #[ignore = "installed Codex startup diagnostic; synthetic input and OS network denied, no model request"]
    async fn installed_codex_synthetic_startup_without_network() {
        installed_codex_startup_probe(true, false).await;
    }

    #[cfg(target_os = "macos")]
    #[tokio::test]
    #[ignore = "installed Codex local-home startup diagnostic; OS IP network denied, no model request"]
    async fn installed_codex_private_home_startup_without_network() {
        installed_codex_startup_probe(true, true).await;
    }

    #[cfg(target_os = "macos")]
    async fn installed_codex_startup_probe(with_prompt: bool, private_home: bool) {
        let project = tempfile::tempdir().unwrap();
        std::fs::create_dir(project.path().join(".architalk")).unwrap();
        let run = RunDirectory::create(Some(project.path())).unwrap();
        let profile = sandbox_profile(run.path()).unwrap()
            + "\n(deny network*)\n(allow network-bind (local unix-socket))\n(allow network-outbound (remote unix-socket))\n";
        if let RunDirectory::Project(owned) = &run {
            owned
                .write_private("no-network.sb", profile.as_bytes())
                .unwrap();
        }
        let spec = RunSpec {
            backend: Backend::Codex,
            executable: "codex".into(),
            model: None,
            effort: Some("low".into()),
            profile: None,
            timeout_seconds: 20,
            prompt: String::new(),
            project_root: project.path().into(),
        };
        let mut args = cli_args(&spec);
        args.pop();
        for (key, value) in [
            ("sqlite_home", run.path().join("codex-state")),
            ("log_dir", run.path().join("log")),
        ] {
            args.extend([
                "-c".to_owned(),
                format!(
                    "{key}={}",
                    serde_json::to_string(&value.to_string_lossy()).unwrap()
                ),
            ]);
        }
        args.extend([
            "-c".into(),
            "history.persistence=\"none\"".into(),
            "-".into(),
        ]);
        let mut command = run.command("/usr/bin/sandbox-exec").unwrap();
        if private_home {
            run.configure_codex_home(&mut command).unwrap();
        }
        let mut child = command
            .arg("-f")
            .arg(run.path().join("no-network.sb"))
            .arg(executable_path("codex").unwrap())
            .args(args)
            .stdin(if with_prompt {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .spawn()
            .unwrap();
        if with_prompt {
            let mut stdin = child.stdin.take().unwrap();
            stdin.write_all(b"Return ONLY the exact JSON object {\"ok\":true}. Synthetic startup probe; do not use tools.").await.unwrap();
            drop(stdin);
        }
        let mut guard = GroupGuard {
            pid: child.id().unwrap(),
            armed: true,
        };
        let stdout = child.stdout.take().unwrap();
        let stderr = child.stderr.take().unwrap();
        let out_task = tokio::spawn(read_small(stdout, STDERR_RETAIN));
        let err_task = tokio::spawn(read_small(stderr, STDERR_RETAIN));
        let output = tokio::time::timeout(Duration::from_secs(20), child.wait()).await;
        let timed_out = output.is_err();
        assert!(
            stop_group(&mut child, &mut guard).await,
            "startup probe process must stop"
        );
        let out = tokio::time::timeout(GRACE, out_task)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        let err = tokio::time::timeout(GRACE, err_task)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        let status = if let Ok(status) = output {
            status.unwrap()
        } else {
            child.try_wait().unwrap().unwrap()
        };
        let events = String::from_utf8_lossy(&out)
            .lines()
            .filter_map(|line| serde_json::from_str::<Value>(line).ok())
            .map(|event| match event["type"].as_str() {
                Some("thread.started") => "thread.started",
                Some("turn.started") => "turn.started",
                Some("error") => "error",
                Some("turn.failed") => "turn.failed",
                _ => "other",
            })
            .collect::<Vec<_>>();
        let stdout_failures = String::from_utf8_lossy(&out)
            .lines()
            .filter_map(|line| serde_json::from_str::<Value>(line).ok())
            .filter(|event| matches!(event["type"].as_str(), Some("error" | "turn.failed")))
            .map(|event| failure_summary(event.to_string().as_bytes()))
            .collect::<Vec<_>>();
        println!(
            "Codex no-network startup with_prompt={with_prompt}, private_home={private_home}, stopped_at_timeout={timed_out}: exit={:?}; {}; events={events:?}; terminal_categories={stdout_failures:?}",
            status.code(),
            failure_line_summaries(&err)
        );
        assert!(
            !status.success(),
            "no-network diagnostic must not produce a successful model result"
        );
    }

    #[cfg(target_os = "macos")]
    #[tokio::test]
    async fn private_codex_home_reads_auth_pointer_without_copy_or_shared_write() {
        use std::os::unix::fs::{PermissionsExt, symlink};
        let project = tempfile::tempdir().unwrap();
        std::fs::create_dir(project.path().join(".architalk")).unwrap();
        let shared = tempfile::tempdir().unwrap();
        let auth = shared.path().join("auth.json");
        std::fs::write(&auth, b"synthetic credential fixture").unwrap();
        std::fs::set_permissions(&auth, std::fs::Permissions::from_mode(0o600)).unwrap();
        let run = RunDirectory::create(Some(project.path())).unwrap();
        let local_home = run.path().join("codex-home");
        if let RunDirectory::Project(owned) = &run {
            assert!(owned.link_codex_auth(shared.path()).unwrap());
        }
        assert!(
            std::fs::symlink_metadata(local_home.join("auth.json"))
                .unwrap()
                .file_type()
                .is_symlink()
        );
        let profile = sandbox_profile(run.path()).unwrap();
        if let RunDirectory::Project(owned) = &run {
            owned
                .write_private("auth-pointer.sb", profile.as_bytes())
                .unwrap();
        }
        let code = "import os,json\np=os.path.join(os.environ['CODEX_HOME'],'auth.json')\nread_ok=open(p).read()=='synthetic credential fixture'\nblocked=False\ntry:\n open(p,'w').write('forbidden')\nexcept PermissionError:\n blocked=True\nprint(json.dumps({'read_ok':read_ok,'shared_write_blocked':blocked}))";
        let out = run
            .command("/usr/bin/sandbox-exec")
            .unwrap()
            .arg("-f")
            .arg(run.path().join("auth-pointer.sb"))
            .arg("/usr/bin/python3")
            .args(["-c", code])
            .env("CODEX_HOME", &local_home)
            .output()
            .await
            .unwrap();
        assert!(out.status.success());
        assert_eq!(
            serde_json::from_slice::<Value>(&out.stdout).unwrap(),
            serde_json::json!({"read_ok":true,"shared_write_blocked":true})
        );
        drop(run);
        assert!(!local_home.exists());
        assert_eq!(
            std::fs::read(&auth).unwrap(),
            b"synthetic credential fixture"
        );
        std::fs::remove_file(&auth).unwrap();
        symlink(shared.path().join("other-auth"), &auth).unwrap();
        let run = OwnedRunDirectory::create(project.path()).unwrap();
        assert!(
            run.link_codex_auth(shared.path()).is_err(),
            "shared credential symlinks must be refused"
        );
    }

    #[test]
    fn failure_diagnostics_never_echo_paths_prompts_or_credentials() {
        let raw = b"Permission denied writing /private/SYNTHETIC_SECRET/.codex/models_cache.json with token=SYNTHETIC_TOKEN prompt=SYNTHETIC_PROMPT";
        let summary = failure_summary(raw);
        assert_eq!(
            summary,
            "reason=permission_denied, component=model_catalog_cache"
        );
        for forbidden in [
            "SYNTHETIC_SECRET",
            "SYNTHETIC_TOKEN",
            "SYNTHETIC_PROMPT",
            "/private/",
        ] {
            assert!(!summary.contains(forbidden));
        }
        let error = Parsed::terminal_failure(
            &serde_json::json!({"type":"turn.failed","error":{"message":String::from_utf8_lossy(raw)}}),
        );
        assert!(error.message.contains("stdout: event=turn.failed"));
        assert!(!error.message.contains("SYNTHETIC_"));
    }

    #[test]
    fn sandbox_profile_escapes_quoted_paths_and_rejects_controls() {
        let profile = sandbox_profile(Path::new("/tmp/project \"quoted\"\\name")).unwrap();
        assert!(profile.contains("\\\"quoted\\\"\\\\name"));
        assert!(sandbox_profile(Path::new("/tmp/line\nbreak")).is_err());
    }
    #[test]
    fn diagnostic_uncertainty_is_preserved_and_authentication_is_distinct() {
        let error = diagnostic_failure(Diagnostic {
            backend: Backend::Claude,
            available: true,
            compatible: false,
            version: None,
            message: "unknown diagnostic child".into(),
            efforts: Vec::new(),
            termination_unknown: true,
        });
        assert!(error.termination_unknown);
        assert_eq!(error.code, "process_termination_unknown");
        let mut parser = Parsed::new(Backend::Claude);
        init(&mut parser);
        assert_eq!(feed(&mut parser, serde_json::json!({"type":"result","subtype":"error_during_execution","is_error":true,"errors":["Invalid API key. Run /login"]})).unwrap_err().code, "authentication_required");
        let mut parser = Parsed::new(Backend::Codex);
        init(&mut parser);
        assert_eq!(feed(&mut parser, serde_json::json!({"type":"turn.failed","error":{"message":"refresh token expired"}})).unwrap_err().code, "authentication_required");
    }
    #[test]
    fn probe_uncertainty_keeps_the_process_gate_and_codex_requires_turn_start() {
        let error = write_denial_outcome(false, true, false).unwrap_err();
        assert_eq!(error.code, "process_termination_unknown");
        assert!(error.termination_unknown);
        let mut parser = Parsed::new(Backend::Codex);
        feed(&mut parser, serde_json::json!({"type":"thread.started"})).unwrap();
        assert!(feed(&mut parser,serde_json::json!({"type":"item.completed","item":{"type":"agent_message","text":"early"}})).is_err());
        assert!(feed(&mut parser, serde_json::json!({"type":"turn.completed"})).is_err());
        feed(&mut parser, serde_json::json!({"type":"turn.started"})).unwrap();
        assert!(feed(&mut parser, serde_json::json!({"type":"turn.started"})).is_err());
    }
    #[test]
    fn claude_thinking_token_metadata_is_nonterminal_and_unknown_subtypes_fail_closed() {
        let telemetry = serde_json::json!({"type":"system","subtype":"thinking_tokens","estimated_tokens":195,"estimated_tokens_delta":145});
        let mut before_init = Parsed::new(Backend::Claude);
        assert!(feed(&mut before_init, telemetry.clone()).is_err());
        let mut parser = Parsed::new(Backend::Claude);
        init(&mut parser);
        assert!(feed(&mut parser, telemetry.clone()).unwrap().is_none());
        assert!(parser.finish().is_err());
        for bad in [serde_json::json!("195"), serde_json::json!(-1), Value::Null] {
            let mut event = telemetry.clone();
            event["estimated_tokens"] = bad;
            assert!(feed(&mut parser, event).is_err());
        }
        let future=feed(&mut parser,serde_json::json!({"type":"system","subtype":"future_metadata","estimated_tokens":1,"estimated_tokens_delta":1})).unwrap_err();
        assert_eq!(future.code, "invalid_cli_output");
        assert!(future.message.contains("future_metadata"));
        let unsafe_subtype = "unexpected\nprivate-value";
        let error = feed(
            &mut parser,
            serde_json::json!({"type":"system","subtype":unsafe_subtype}),
        )
        .unwrap_err();
        assert!(error.message.contains("unknown"));
        assert!(!error.message.contains("private-value"));
        feed(
            &mut parser,
            serde_json::json!({"type":"result","subtype":"success","is_error":false,"result":"OK"}),
        )
        .unwrap();
        assert_eq!(parser.finish().unwrap(), "OK");
        assert!(feed(&mut parser, telemetry).is_err());
    }
}
