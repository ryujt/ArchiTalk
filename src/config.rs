use crate::ai::{Backend, RunSpec};
use anyhow::{Context, Result, bail};
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Settings {
    pub ai: AiConfig,
    pub documents: DocumentConfig,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct AiConfig {
    pub default_backend: Backend,
    pub timeout_seconds: u64,
    pub backends: Backends,
    pub roles: Roles,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Backends {
    pub claude: BackendConfig,
    pub codex: BackendConfig,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct BackendConfig {
    pub executable: String,
    pub model: Option<String>,
    pub effort: Option<String>,
    pub profile: Option<String>,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct RoleConfig {
    pub backend: Option<Backend>,
    pub model: Option<String>,
    pub effort: Option<String>,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Roles {
    pub conversation: RoleConfig,
    pub design: RoleConfig,
    pub critic: RoleConfig,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct DocumentConfig {
    pub directory: String,
    pub language: String,
}

impl Default for AiConfig {
    fn default() -> Self {
        Self {
            default_backend: Backend::Claude,
            timeout_seconds: 300,
            backends: Backends::default(),
            roles: Roles::default(),
        }
    }
}
impl Default for BackendConfig {
    fn default() -> Self {
        Self {
            executable: "claude".into(),
            model: None,
            effort: None,
            profile: None,
        }
    }
}
impl Default for Backends {
    fn default() -> Self {
        Self {
            claude: BackendConfig::default(),
            codex: BackendConfig {
                executable: "codex".into(),
                ..BackendConfig::default()
            },
        }
    }
}
impl Default for DocumentConfig {
    fn default() -> Self {
        Self {
            directory: "docs/architecture".into(),
            language: "ko".into(),
        }
    }
}

impl Settings {
    pub fn backend(&self, b: Backend) -> &BackendConfig {
        match b {
            Backend::Claude => &self.ai.backends.claude,
            Backend::Codex => &self.ai.backends.codex,
        }
    }
    pub fn role(&self, name: &str) -> &RoleConfig {
        match name {
            "design" => &self.ai.roles.design,
            "critic" => &self.ai.roles.critic,
            _ => &self.ai.roles.conversation,
        }
    }
    pub fn run_spec(&self, role: &str, prompt: String, project_root: PathBuf) -> RunSpec {
        let r = self.role(role);
        let backend = r.backend.unwrap_or(self.ai.default_backend);
        let b = self.backend(backend);
        RunSpec {
            backend,
            executable: b.executable.clone(),
            model: r.model.clone().or(b.model.clone()),
            effort: r.effort.clone().or(b.effort.clone()),
            profile: b.profile.clone(),
            timeout_seconds: self.ai.timeout_seconds,
            prompt,
            project_root,
        }
    }
    pub fn validate(&self) -> Result<()> {
        if !(10..=600).contains(&self.ai.timeout_seconds) {
            bail!("timeout_seconds must be between 10 and 600");
        }
        for b in [&self.ai.backends.claude, &self.ai.backends.codex] {
            if b.executable.trim().is_empty()
                || b.executable.len() > 4096
                || b.executable.contains('\0')
            {
                bail!("invalid executable");
            }
            validate_option(&b.model)?;
            validate_option(&b.effort)?;
            validate_option(&b.profile)?;
        }
        for r in [
            &self.ai.roles.conversation,
            &self.ai.roles.design,
            &self.ai.roles.critic,
        ] {
            validate_option(&r.model)?;
            validate_option(&r.effort)?;
        }
        let p = Path::new(&self.documents.directory);
        if p.is_absolute()
            || p.components()
                .any(|c| !matches!(c, std::path::Component::Normal(_)))
            || self.documents.directory.is_empty()
        {
            bail!("document directory must be a project-relative path");
        }
        if !["ko", "en"].contains(&self.documents.language.as_str()) {
            bail!("document language must be ko or en");
        }
        Ok(())
    }
    /// The web/project settings cannot replace trusted local executables or CLI profiles.
    pub fn validate_web_change(&self, trusted: &Self) -> Result<()> {
        self.validate()?;
        for (a, b) in [
            (&self.ai.backends.claude, &trusted.ai.backends.claude),
            (&self.ai.backends.codex, &trusted.ai.backends.codex),
        ] {
            if a.executable != b.executable || a.profile != b.profile {
                bail!("executable/profile can only be changed in the trusted user config");
            }
        }
        Ok(())
    }
}
fn validate_option(v: &Option<String>) -> Result<()> {
    if let Some(v) = v {
        if v.is_empty() || v.len() > 256 || v.chars().any(char::is_control) || v.starts_with('-') {
            bail!("invalid model, effort or profile");
        }
    }
    Ok(())
}
pub fn default_path() -> PathBuf {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
        .unwrap_or_else(std::env::temp_dir);
    base.join("architalk/config.toml")
}
pub fn load(path: &Path) -> Result<Settings> {
    let settings = if path.exists() {
        let text =
            fs::read_to_string(path).with_context(|| format!("read config {}", path.display()))?;
        toml::from_str(&text).context("invalid ArchiTalk config")?
    } else {
        Settings::default()
    };
    settings.validate()?;
    Ok(settings)
}

pub fn ai_hash(ai: &AiConfig) -> Result<String> {
    Ok(hex::encode(Sha256::digest(serde_json::to_vec(ai)?)))
}

/// Only generic AI preferences are shared. Project output paths, conversation
/// content and run records never enter the common configuration file.
pub fn save_ai(path: &Path, ai: &AiConfig, expected_hash: &str) -> Result<String> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    fs::create_dir_all(parent).context("create shared settings directory")?;
    let name = path.file_name().context("settings file needs a filename")?;
    let mut lock_name = name.to_os_string();
    lock_name.push(".lock");
    let mut options = fs::OpenOptions::new();
    options.read(true).write(true).create(true).truncate(false);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
    }
    let lock = options
        .open(parent.join(lock_name))
        .context("lock shared settings")?;
    lock.lock_exclusive()?;
    let mut current = load(path)?;
    let current_hash = ai_hash(&current.ai)?;
    let desired_hash = ai_hash(ai)?;
    if current_hash != expected_hash && current_hash != desired_hash {
        bail!(
            "shared_settings_conflict: another process changed the common AI settings; reload settings before saving"
        );
    }
    if current_hash != desired_hash {
        current.ai = ai.clone();
        current.validate()?;
        let mut file = tempfile::NamedTempFile::new_in(parent)?;
        file.write_all(toml::to_string_pretty(&current)?.as_bytes())?;
        file.as_file().sync_all()?;
        file.persist(path).map_err(|e| e.error)?;
        fs::File::open(parent)?.sync_all()?;
    }
    Ok(desired_hash)
}
