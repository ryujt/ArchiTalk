//! Root-relative project evidence and immutable export publication.
//!
//! `reserve` must be persisted by the controller before `apply`. A directory
//! without a persisted identity is deliberately never adopted after a crash.
use crate::design_package::{DesignDocument, MAIN_FILENAME};
use anyhow::{Context, Result, anyhow, bail};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    ffi::CString,
    fs::{self, File},
    io::{Read, Write},
    os::{
        fd::{AsRawFd, FromRawFd, IntoRawFd},
        unix::fs::{MetadataExt, OpenOptionsExt},
    },
    path::{Component, Path, PathBuf},
};
use unicode_normalization::UnicodeNormalization;

pub const FILE_LIMIT: usize = 128 * 1024;
pub const CONTEXT_LIMIT: usize = 1024 * 1024;
const MANIFEST: &str = ".architalk-export.json";

pub fn hash_bytes(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct DirectoryIdentity {
    pub device: u64,
    pub inode: u64,
}

/// A pinned directory handle; all child access refuses symlinks.
pub(crate) struct Directory {
    file: File,
}
impl Directory {
    pub(crate) fn root(root: &Path) -> Result<Self> {
        let before = fs::symlink_metadata(root).context("project root does not exist")?;
        if !before.is_dir() || before.file_type().is_symlink() {
            bail!("unsafe project root");
        }
        let canonical = root.canonicalize().context("project root does not exist")?;
        let file = fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open("/")?;
        let mut current = Self { file };
        for component in canonical.components() {
            if let Component::Normal(name) = component {
                current = current.open_dir(
                    name.to_str()
                        .ok_or_else(|| anyhow!("root path is not UTF-8"))?,
                )?;
            }
        }
        let after = current.file.metadata()?;
        if before.dev() != after.dev() || before.ino() != after.ino() {
            bail!("project root changed while opening");
        }
        Ok(current)
    }
    pub(crate) fn try_clone(&self) -> Result<Self> {
        self.open_dir(".")
    }
    pub(crate) fn lock_exclusive(&self) -> Result<()> {
        fs2::FileExt::try_lock_exclusive(&self.file)
            .context("project_locked: another ArchiTalk process owns this project inode")?;
        Ok(())
    }
    pub(crate) fn unlock(&self) -> Result<()> {
        fs2::FileExt::unlock(&self.file)?;
        Ok(())
    }
    fn name(name: &str) -> Result<CString> {
        if name.is_empty() || name == ".." || name.contains('/') || name.contains('\\') {
            bail!("unsafe child filename");
        }
        Ok(CString::new(name)?)
    }
    pub(crate) fn open_dir(&self, name: &str) -> Result<Self> {
        let name = Self::name(name)?;
        let fd = unsafe {
            libc::openat(
                self.file.as_raw_fd(),
                name.as_ptr(),
                libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            )
        };
        if fd < 0 {
            return Err(std::io::Error::last_os_error().into());
        }
        Ok(Self {
            file: unsafe { File::from_raw_fd(fd) },
        })
    }
    pub(crate) fn mkdir(&self, name: &str, mode: u32) -> Result<()> {
        let name = Self::name(name)?;
        if unsafe { libc::mkdirat(self.file.as_raw_fd(), name.as_ptr(), mode as libc::mode_t) } != 0
        {
            return Err(std::io::Error::last_os_error().into());
        }
        self.sync()?;
        Ok(())
    }
    pub(crate) fn open_file(&self, name: &str, create: bool) -> Result<File> {
        let name = Self::name(name)?;
        let flags = if create {
            libc::O_RDWR | libc::O_CREAT
        } else {
            libc::O_RDONLY
        };
        let fd = unsafe {
            libc::openat(
                self.file.as_raw_fd(),
                name.as_ptr(),
                flags | libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK,
                0o600,
            )
        };
        if fd < 0 {
            return Err(std::io::Error::last_os_error().into());
        }
        let file = unsafe { File::from_raw_fd(fd) };
        if !file.metadata()?.is_file() {
            bail!("not a regular file");
        }
        Ok(file)
    }
    pub(crate) fn identity(&self) -> Result<DirectoryIdentity> {
        let m = self.file.metadata()?;
        Ok(DirectoryIdentity {
            device: m.dev(),
            inode: m.ino(),
        })
    }
    pub(crate) fn raw_fd(&self) -> std::os::fd::RawFd {
        self.file.as_raw_fd()
    }
    pub(crate) fn metadata(&self) -> Result<fs::Metadata> {
        Ok(self.file.metadata()?)
    }
    /// Replace a private shared secret atomically, never traversing a child link.
    pub(crate) fn replace_private_file(&self, name: &str, bytes: &[u8]) -> Result<()> {
        let temporary = format!(".architalk-secret-{}", uuid::Uuid::new_v4());
        let mut file = self.create_private_file(&temporary)?;
        let temporary_c = Self::name(&temporary)?;
        let name_c = Self::name(name)?;
        let result = (|| -> Result<()> {
            file.write_all(bytes)?;
            file.sync_all()?;
            if unsafe {
                libc::renameat(
                    self.raw_fd(),
                    temporary_c.as_ptr(),
                    self.raw_fd(),
                    name_c.as_ptr(),
                )
            } != 0
            {
                return Err(std::io::Error::last_os_error().into());
            }
            self.sync()
        })();
        unsafe {
            libc::unlinkat(self.raw_fd(), temporary_c.as_ptr(), 0);
        }
        result
    }
    /// Remove an owned scratch subtree through directory descriptors only.
    /// A replaced top-level name is preserved, and symlinks are unlinked rather
    /// than followed. Callers also verify their pinned ancestor identities.
    pub(crate) fn remove_private_tree(
        &self,
        name: &str,
        identity: &DirectoryIdentity,
    ) -> Result<()> {
        let child = self.open_dir(name)?;
        if &child.identity()? != identity {
            bail!("scratch directory identity changed");
        }
        for entry in child.names()? {
            match child.open_dir(&entry) {
                Ok(directory) => child.remove_private_tree(&entry, &directory.identity()?)?,
                Err(error)
                    if error.downcast_ref::<std::io::Error>().is_some_and(|error| {
                        matches!(error.raw_os_error(), Some(libc::ENOTDIR | libc::ELOOP))
                    }) =>
                {
                    let entry = Self::name(&entry)?;
                    if unsafe { libc::unlinkat(child.file.as_raw_fd(), entry.as_ptr(), 0) } != 0 {
                        return Err(std::io::Error::last_os_error().into());
                    }
                }
                Err(error) => return Err(error),
            }
        }
        if &self.open_dir(name)?.identity()? != identity {
            bail!("scratch directory identity changed during cleanup");
        }
        let name = Self::name(name)?;
        if unsafe { libc::unlinkat(self.file.as_raw_fd(), name.as_ptr(), libc::AT_REMOVEDIR) } != 0
        {
            return Err(std::io::Error::last_os_error().into());
        }
        self.sync()?;
        Ok(())
    }
    pub(crate) fn make_private(&self) -> Result<()> {
        let metadata = self.file.metadata()?;
        if metadata.uid() != unsafe { libc::geteuid() } {
            bail!("state directory belongs to another user");
        }
        self.file
            .set_permissions(std::os::unix::fs::PermissionsExt::from_mode(0o700))?;
        Ok(())
    }
    pub(crate) fn create_private_file(&self, name: &str) -> Result<File> {
        let name_c = Self::name(name)?;
        let fd = unsafe {
            libc::openat(
                self.file.as_raw_fd(),
                name_c.as_ptr(),
                libc::O_RDWR | libc::O_CREAT | libc::O_EXCL | libc::O_NOFOLLOW | libc::O_CLOEXEC,
                0o600,
            )
        };
        if fd < 0 {
            return Err(std::io::Error::last_os_error().into());
        }
        let output = unsafe { File::from_raw_fd(fd) };
        self.sync()?;
        Ok(output)
    }
    pub(crate) fn sync(&self) -> Result<()> {
        self.file.sync_all()?;
        Ok(())
    }
    pub(crate) fn fd_path(&self) -> PathBuf {
        PathBuf::from(format!("/__architalk_fd__/{}", self.file.as_raw_fd()))
    }
    fn names(&self) -> Result<Vec<String>> {
        // /dev/fd/<dirfd>/child is not traversable on macOS. fdopendir is.
        let independent = self.open_dir(".")?.file.into_raw_fd();
        let stream = unsafe { libc::fdopendir(independent) };
        if stream.is_null() {
            unsafe {
                libc::close(independent);
            }
            return Err(std::io::Error::last_os_error().into());
        }
        let result = (|| -> Result<Vec<String>> {
            let mut names = Vec::new();
            loop {
                let entry = unsafe { libc::readdir(stream) };
                if entry.is_null() {
                    break;
                }
                let bytes =
                    unsafe { std::ffi::CStr::from_ptr((*entry).d_name.as_ptr()) }.to_bytes();
                if bytes == b"." || bytes == b".." {
                    continue;
                }
                if let Ok(name) = std::str::from_utf8(bytes) {
                    names.push(name.to_owned());
                }
                if names.len() > 10_000 {
                    bail!("directory has too many entries");
                }
            }
            names.sort();
            Ok(names)
        })();
        unsafe {
            libc::closedir(stream);
        }
        result
    }
    fn publish(&self, name: &str, bytes: &[u8]) -> Result<()> {
        let temporary = format!(".architalk-tmp-{}", uuid::Uuid::new_v4());
        let temp_c = Self::name(&temporary)?;
        let name_c = Self::name(name)?;
        let fd = unsafe {
            libc::openat(
                self.file.as_raw_fd(),
                temp_c.as_ptr(),
                libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_NOFOLLOW | libc::O_CLOEXEC,
                0o644,
            )
        };
        if fd < 0 {
            return Err(std::io::Error::last_os_error().into());
        }
        let result = (|| -> Result<()> {
            let mut file = unsafe { File::from_raw_fd(fd) };
            file.write_all(bytes)?;
            file.sync_all()?;
            if unsafe {
                libc::linkat(
                    self.file.as_raw_fd(),
                    temp_c.as_ptr(),
                    self.file.as_raw_fd(),
                    name_c.as_ptr(),
                    0,
                )
            } != 0
            {
                return Err(std::io::Error::last_os_error().into());
            }
            self.sync()?;
            Ok(())
        })();
        unsafe {
            libc::unlinkat(self.file.as_raw_fd(), temp_c.as_ptr(), 0);
        }
        result
    }
}

fn folded(value: &str) -> String {
    value
        .nfc()
        .flat_map(char::to_uppercase)
        .flat_map(char::to_lowercase)
        .nfc()
        .collect()
}
fn validate_component(name: &str) -> Result<()> {
    if name.is_empty()
        || name == "."
        || name == ".."
        || name.len() > 160
        || name.chars().any(|c| {
            c.is_control() || matches!(c, '/' | '\\' | ':' | '<' | '>' | '"' | '|' | '?' | '*')
        })
        || name.ends_with(['.', ' '])
        || name.nfc().collect::<String>() != name
    {
        bail!("invalid_path: unsafe or non-NFC path component");
    }
    let stem = name.split('.').next().unwrap_or("").to_ascii_uppercase();
    if matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || (stem.len() == 4
            && (stem.starts_with("COM") || stem.starts_with("LPT"))
            && matches!(stem.as_bytes()[3], b'1'..=b'9'))
    {
        bail!("invalid_path: reserved name");
    }
    Ok(())
}
pub fn validate_relative_path(path: &str) -> Result<Vec<String>> {
    if path.is_empty()
        || path.len() > 1024
        || path.starts_with('/')
        || path.contains('\\')
        || path.split('/').any(str::is_empty)
    {
        bail!("invalid_path: expected root-relative path");
    }
    let mut parts = Vec::new();
    for part in Path::new(path).components() {
        match part {
            Component::Normal(s) => {
                let name = s.to_str().ok_or_else(|| anyhow!("invalid UTF-8 path"))?;
                validate_component(name)?;
                if name == ".architalk" {
                    bail!("invalid_path: private state directory");
                }
                parts.push(name.to_owned());
            }
            _ => bail!("invalid_path: traversal"),
        }
    }
    if path.split('/').any(|p| p == "." || p == "..") {
        bail!("invalid_path: traversal");
    }
    Ok(parts)
}
fn assert_spelling(dir: &Directory, name: &str, allow_exact: bool) -> Result<bool> {
    let mut exact = false;
    for existing in dir.names()? {
        if folded(&existing) == folded(name) {
            if existing == name && allow_exact {
                exact = true;
            } else {
                bail!("path_conflict: case or normalization collision");
            }
        }
    }
    Ok(exact)
}
fn descend(root: &Directory, parts: &[String], create: bool) -> Result<Directory> {
    let mut current = root.open_dir(".")?;
    for part in parts {
        let exists = assert_spelling(&current, part, true)?;
        if !exists && create {
            current.mkdir(part, 0o755)?;
        }
        current = current
            .open_dir(part)
            .with_context(|| format!("unsafe or inaccessible directory: {part}"))?;
    }
    Ok(current)
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExportFile {
    pub relative_path: String,
    pub content: String,
    pub hash: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExportPlan {
    pub id: String,
    pub document_revision: u64,
    pub document_hash: String,
    pub session_id: String,
    pub title: String,
    pub relative_directory: String,
    pub files: Vec<ExportFile>,
    pub bundle_identity: Option<DirectoryIdentity>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExportStatus {
    Completed,
    Conflict,
    Failed,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExportResult {
    pub status: ExportStatus,
    pub paths: Vec<String>,
    pub error: Option<String>,
    pub files: Vec<ExportFileResult>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExportFileResult {
    pub path: String,
    pub status: String,
    pub actual_hash: Option<String>,
}
#[derive(Serialize, Deserialize)]
struct Manifest {
    version: u32,
    export_id: String,
    session_id: String,
    document_revision: u64,
    document_hash: String,
    files: Vec<ManifestFile>,
}
#[derive(Serialize, Deserialize)]
struct ManifestFile {
    path: String,
    hash: String,
}

/// A lifetime-pinned project boundary. Obtain this from `Store::project_files`
/// so SQLite and files use exactly the same root inode, even after path swaps.
pub struct ProjectFiles {
    root: Directory,
}
impl ProjectFiles {
    pub fn open(root: &Path) -> Result<Self> {
        Ok(Self {
            root: Directory::root(root)?,
        })
    }
    pub(crate) fn from_directory(root: Directory) -> Self {
        Self { root }
    }
    pub fn scan(&self) -> Result<ProjectScan> {
        scan_at(&self.root)
    }
    pub fn read_selected(&self, selected: &[String]) -> Result<Vec<EvidenceFile>> {
        read_selected_at(&self.root, selected)
    }
    /// Viewer browsing is deliberately separate from AI evidence selection.
    pub fn viewer_tree(&self) -> Result<ViewerTree> {
        let mut budget = ViewerBudget::default();
        let entries = viewer_walk(&self.root, "", 0, &mut budget)?;
        Ok(ViewerTree {
            entries,
            files: budget.files,
            truncated: budget.truncated,
        })
    }
    pub fn viewer_read(&self, path: &str) -> Result<ViewerContent> {
        let parts = viewer_parts(path)?;
        let mut current = self.root.try_clone()?;
        let mut canonical = Vec::new();
        for part in &parts[..parts.len() - 1] {
            let part = viewer_spelling(&current, part)?;
            current = current.open_dir(&part)?;
            canonical.push(part);
        }
        let name = viewer_spelling(&current, parts.last().unwrap())?;
        let kind = viewer_kind(&name).ok_or_else(|| anyhow!("file is not viewable"))?;
        let content = viewer_text(current.open_file(&name, false)?)?;
        canonical.push(name.clone());
        Ok(ViewerContent {
            path: canonical.join("/"),
            name,
            kind,
            size: content.len() as u64,
            hash: hash_bytes(content.as_bytes()),
            content,
        })
    }
    pub fn preview(
        &self,
        title: &str,
        markdown: &str,
        document_revision: u64,
        session_id: &str,
    ) -> Result<ExportPlan> {
        self.preview_in(
            "docs/architecture",
            title,
            markdown,
            document_revision,
            session_id,
        )
    }
    pub fn preview_in(
        &self,
        base_directory: &str,
        title: &str,
        markdown: &str,
        document_revision: u64,
        session_id: &str,
    ) -> Result<ExportPlan> {
        self.preview_documents_in(
            base_directory,
            title,
            markdown,
            &[],
            document_revision,
            session_id,
        )
    }
    /// Build one immutable bundle. Artifact kinds, rather than model paths,
    /// determine filenames; a legacy draft supplies an empty document slice.
    pub fn preview_documents_in(
        &self,
        base_directory: &str,
        title: &str,
        markdown: &str,
        documents: &[DesignDocument],
        document_revision: u64,
        session_id: &str,
    ) -> Result<ExportPlan> {
        preview_in_at(
            &self.root,
            base_directory,
            title,
            markdown,
            documents,
            document_revision,
            session_id,
        )
    }
    pub fn reserve(&self, plan: &ExportPlan) -> Result<ExportPlan> {
        reserve_at(&self.root, plan)
    }
    pub fn apply(&self, plan: &ExportPlan) -> Result<ExportResult> {
        apply_at(&self.root, plan)
    }
    pub fn reconcile(&self, plan: &ExportPlan) -> Result<ExportResult> {
        self.apply(plan)
    }
}

pub fn preview(
    root: &Path,
    title: &str,
    markdown: &str,
    document_revision: u64,
    session_id: &str,
) -> Result<ExportPlan> {
    preview_in(
        root,
        "docs/architecture",
        title,
        markdown,
        document_revision,
        session_id,
    )
}
/// User configurable export base; paths are still root-relative and validated.
pub fn preview_in(
    root: &Path,
    base_directory: &str,
    title: &str,
    markdown: &str,
    document_revision: u64,
    session_id: &str,
) -> Result<ExportPlan> {
    ProjectFiles::open(root)?.preview_in(
        base_directory,
        title,
        markdown,
        document_revision,
        session_id,
    )
}
pub fn preview_documents_in(
    root: &Path,
    base_directory: &str,
    title: &str,
    markdown: &str,
    documents: &[DesignDocument],
    document_revision: u64,
    session_id: &str,
) -> Result<ExportPlan> {
    ProjectFiles::open(root)?.preview_documents_in(
        base_directory,
        title,
        markdown,
        documents,
        document_revision,
        session_id,
    )
}
fn preview_in_at(
    root_dir: &Directory,
    base_directory: &str,
    title: &str,
    markdown: &str,
    documents: &[DesignDocument],
    document_revision: u64,
    session_id: &str,
) -> Result<ExportPlan> {
    let topic = title
        .nfc()
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join("-");
    let topic = if topic.is_empty() {
        "새로운-설계".to_owned()
    } else {
        topic
    };
    validate_component(&topic)?;
    let parent = validate_relative_path(base_directory)?;
    let mut existing_parent = Some(root_dir.open_dir(".")?);
    for part in &parent {
        if let Some(current) = existing_parent.take() {
            if assert_spelling(&current, part, true)? {
                existing_parent = Some(current.open_dir(part).context("unsafe export parent")?);
            }
        }
    }
    let mut suffix = 1;
    let relative_directory = loop {
        let name = if suffix == 1 {
            topic.clone()
        } else {
            format!("{topic}-개정-{suffix}")
        };
        validate_component(&name)?;
        let occupied = match &existing_parent {
            Some(dir) => dir.names()?.iter().any(|n| folded(n) == folded(&name)),
            None => false,
        };
        if !occupied {
            break format!("{base_directory}/{name}");
        }
        suffix += 1;
    };
    validate_relative_path(&relative_directory)?;
    let mut files = vec![ExportFile {
        relative_path: MAIN_FILENAME.to_owned(),
        content: markdown.to_owned(),
        hash: hash_bytes(markdown.as_bytes()),
    }];
    let mut ordered: Vec<_> = documents.iter().collect();
    ordered.sort_by_key(|document| document.kind);
    files.extend(ordered.into_iter().map(|document| ExportFile {
        relative_path: document.kind.filename().to_owned(),
        content: document.markdown.clone(),
        hash: hash_bytes(document.markdown.as_bytes()),
    }));
    let plan = ExportPlan {
        id: uuid::Uuid::new_v4().to_string(),
        document_revision,
        document_hash: hash_bytes(markdown.as_bytes()),
        session_id: session_id.to_owned(),
        title: title.to_owned(),
        relative_directory,
        files,
        bundle_identity: None,
    };
    validate_plan(&plan)?;
    Ok(plan)
}
fn validate_plan(plan: &ExportPlan) -> Result<Vec<String>> {
    let parts = validate_relative_path(&plan.relative_directory)?;
    if parts.is_empty() || plan.files.is_empty() || plan.files.len() > 100 {
        bail!("invalid export plan");
    }
    let mut names = BTreeSet::new();
    for file in &plan.files {
        let file_parts = validate_relative_path(&file.relative_path)?;
        if file_parts.len() != 1
            || file.relative_path == MANIFEST
            || !names.insert(folded(&file.relative_path))
            || file.hash != hash_bytes(file.content.as_bytes())
        {
            bail!("invalid export file or hash");
        }
    }
    if plan.files[0].hash != plan.document_hash {
        bail!("invalid document hash");
    }
    Ok(parts)
}
/// Creates a new bundle exclusively. Persist its returned identity before apply.
pub fn reserve(root: &Path, plan: &ExportPlan) -> Result<ExportPlan> {
    ProjectFiles::open(root)?.reserve(plan)
}
fn reserve_at(root_dir: &Directory, plan: &ExportPlan) -> Result<ExportPlan> {
    let parts = validate_plan(plan)?;
    if plan.bundle_identity.is_some() {
        bail!("bundle already reserved");
    }
    let parent = descend(root_dir, &parts[..parts.len() - 1], true)?;
    let name = parts.last().unwrap();
    assert_spelling(&parent, name, false)?;
    parent
        .mkdir(name, 0o755)
        .context("export_conflict: directory already exists")?;
    let bundle = parent.open_dir(name)?;
    let mut reserved = plan.clone();
    reserved.bundle_identity = Some(bundle.identity()?);
    Ok(reserved)
}
fn manifest_bytes(plan: &ExportPlan) -> Result<Vec<u8>> {
    Ok(serde_json::to_vec_pretty(&Manifest {
        version: 1,
        export_id: plan.id.clone(),
        session_id: plan.session_id.clone(),
        document_revision: plan.document_revision,
        document_hash: plan.document_hash.clone(),
        files: plan
            .files
            .iter()
            .map(|f| ManifestFile {
                path: f.relative_path.clone(),
                hash: f.hash.clone(),
            })
            .collect(),
    })?)
}
fn existing_hash(dir: &Directory, name: &str, max: usize) -> Result<Option<String>> {
    match dir.open_file(name, false) {
        Ok(file) => {
            if file.metadata()?.len() > max as u64 {
                bail!("export_conflict: unexpected file size");
            }
            let mut bytes = Vec::new();
            file.take(max as u64 + 1).read_to_end(&mut bytes)?;
            if bytes.len() > max {
                bail!("export_conflict: file grew");
            }
            Ok(Some(hash_bytes(&bytes)))
        }
        Err(e)
            if e.downcast_ref::<std::io::Error>()
                .is_some_and(|v| v.kind() == std::io::ErrorKind::NotFound) =>
        {
            Ok(None)
        }
        Err(e) => Err(e.context("export_conflict: unsafe existing file")),
    }
}
fn apply_inner(root_dir: &Directory, plan: &ExportPlan) -> Result<Vec<String>> {
    let parts = validate_plan(plan)?;
    let identity = plan
        .bundle_identity
        .as_ref()
        .ok_or_else(|| anyhow!("export_conflict: bundle identity was not persisted"))?;
    let bundle =
        descend(root_dir, &parts, false).context("export_conflict: bundle missing or unsafe")?;
    if &bundle.identity()? != identity {
        bail!("export_conflict: bundle identity changed");
    }
    let expected: BTreeSet<_> = plan
        .files
        .iter()
        .map(|f| f.relative_path.as_str())
        .chain(std::iter::once(MANIFEST))
        .collect();
    for name in bundle.names()? {
        if !expected.contains(name.as_str()) {
            bail!("export_conflict: unexpected file in bundle");
        }
    }
    // Check every existing file before publishing any missing file.
    for file in &plan.files {
        assert_spelling(&bundle, &file.relative_path, true)?;
        if let Some(actual) = existing_hash(&bundle, &file.relative_path, file.content.len())? {
            if actual != file.hash {
                bail!("export_conflict: external content change");
            }
        }
    }
    let manifest = manifest_bytes(plan)?;
    if let Some(actual) = existing_hash(&bundle, MANIFEST, manifest.len())? {
        if actual != hash_bytes(&manifest) {
            bail!("export_conflict: manifest mismatch");
        }
    }
    for file in &plan.files {
        if existing_hash(&bundle, &file.relative_path, file.content.len())?.is_none() {
            bundle.publish(&file.relative_path, file.content.as_bytes())?;
        }
        if existing_hash(&bundle, &file.relative_path, file.content.len())?.as_deref()
            != Some(&file.hash)
        {
            bail!("export_conflict: file changed during publication");
        }
    }
    // Manifest publishes only after all desired bytes have been verified.
    if existing_hash(&bundle, MANIFEST, manifest.len())?.is_none() {
        bundle.publish(MANIFEST, &manifest)?;
    }
    let current = descend(root_dir, &parts, false)?;
    if &current.identity()? != identity {
        bail!("export_conflict: bundle changed during publication");
    }
    for name in current.names()? {
        if !expected.contains(name.as_str()) {
            bail!("export_conflict: unexpected file during verification");
        }
    }
    for file in &plan.files {
        if existing_hash(&current, &file.relative_path, file.content.len())?.as_deref()
            != Some(&file.hash)
        {
            bail!("export_conflict: file changed during verification");
        }
    }
    if existing_hash(&current, MANIFEST, manifest.len())?.as_deref()
        != Some(hash_bytes(&manifest).as_str())
    {
        bail!("export_conflict: manifest changed");
    }
    Ok(plan
        .files
        .iter()
        .map(|f| format!("{}/{}", plan.relative_directory, f.relative_path))
        .collect())
}
fn file_results(root_dir: &Directory, plan: &ExportPlan) -> Vec<ExportFileResult> {
    let bundle = validate_relative_path(&plan.relative_directory)
        .and_then(|parts| descend(root_dir, &parts, false));
    plan.files
        .iter()
        .map(|file| {
            let actual = bundle.as_ref().ok().and_then(|dir| {
                existing_hash(dir, &file.relative_path, file.content.len().max(FILE_LIMIT)).ok()
            });
            let (status, actual_hash) = match actual {
                Some(Some(hash)) if hash == file.hash => ("applied", Some(hash)),
                Some(Some(hash)) => ("conflict", Some(hash)),
                Some(None) => ("not_applied", None),
                None => ("unavailable", None),
            };
            ExportFileResult {
                path: format!("{}/{}", plan.relative_directory, file.relative_path),
                status: status.to_owned(),
                actual_hash,
            }
        })
        .collect()
}
pub fn apply(root: &Path, reserved_plan: &ExportPlan) -> Result<ExportResult> {
    ProjectFiles::open(root)?.apply(reserved_plan)
}
fn apply_at(root_dir: &Directory, reserved_plan: &ExportPlan) -> Result<ExportResult> {
    let result = apply_inner(root_dir, reserved_plan);
    let files = file_results(root_dir, reserved_plan);
    match result {
        Ok(paths) => Ok(ExportResult {
            status: ExportStatus::Completed,
            paths,
            error: None,
            files,
        }),
        Err(error) => {
            let message = format!("{error:#}");
            let collision = error
                .chain()
                .filter_map(|e| e.downcast_ref::<std::io::Error>())
                .any(|e| {
                    matches!(
                        e.raw_os_error(),
                        Some(libc::EEXIST | libc::ELOOP | libc::ENOTDIR)
                    )
                });
            let status =
                if collision || message.contains("conflict") || message.contains("collision") {
                    ExportStatus::Conflict
                } else {
                    ExportStatus::Failed
                };
            Ok(ExportResult {
                status,
                paths: Vec::new(),
                error: Some(message),
                files,
            })
        }
    }
}
pub fn reconcile(root: &Path, reserved_plan: &ExportPlan) -> Result<ExportResult> {
    apply(root, reserved_plan)
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProjectFileSummary {
    pub path: String,
    pub selectable: bool,
    pub size: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProjectScan {
    pub files: Vec<ProjectFileSummary>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EvidenceFile {
    pub path: String,
    pub hash: String,
    pub text: String,
}
fn excluded(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n.starts_with('.')
        || n.contains("credential")
        || n.contains("secret")
        || n.contains("private")
        || n.contains("password")
        || n.contains("token")
        || [".pem", ".key", ".p12", ".pfx", ".der", ".keystore", ".jks"]
            .iter()
            .any(|suffix| n.ends_with(suffix))
        || matches!(
            n.as_str(),
            "id_rsa"
                | "id_dsa"
                | "id_ecdsa"
                | "id_ed25519"
                | "authorized_keys"
                | "known_hosts"
                | "key.json"
                | "keys.json"
                | "node_modules"
                | "target"
                | "dist"
                | "build"
                | "vendor"
        )
}
fn selectable(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    !excluded(name)
        && (matches!(
            n.as_str(),
            "readme"
                | "readme.md"
                | "readme.txt"
                | "readme.rst"
                | "cargo.toml"
                | "package.json"
                | "go.mod"
                | "pyproject.toml"
                | "requirements.txt"
                | "gemfile"
                | "composer.json"
                | "build.gradle"
                | "pom.xml"
        ))
}
pub fn scan(root: &Path) -> Result<ProjectScan> {
    ProjectFiles::open(root)?.scan()
}
fn scan_at(dir: &Directory) -> Result<ProjectScan> {
    let mut files = Vec::new();
    for name in dir.names()? {
        if excluded(&name) || validate_component(&name).is_err() {
            continue;
        }
        // Opening through the pinned directory also excludes symlink/FIFO/device.
        let Ok(file) = dir.open_file(&name, false) else {
            continue;
        };
        let size = file.metadata()?.len();
        files.push(ProjectFileSummary {
            path: name.clone(),
            selectable: selectable(&name) && size <= FILE_LIMIT as u64,
            size,
        });
        if files.len() >= 2048 {
            break;
        }
    }
    Ok(ProjectScan { files })
}
pub fn read_selected(root: &Path, selected: &[String]) -> Result<Vec<EvidenceFile>> {
    ProjectFiles::open(root)?.read_selected(selected)
}
fn read_selected_at(dir: &Directory, selected: &[String]) -> Result<Vec<EvidenceFile>> {
    if selected.len() > 128 {
        bail!("too many context files");
    }
    let mut seen = BTreeSet::new();
    let mut total = 0usize;
    let mut evidence = Vec::new();
    for path in selected {
        let parts = validate_relative_path(path)?;
        if parts.len() != 1 || !selectable(path) || !seen.insert(folded(path)) {
            bail!("context_path_not_allowed");
        }
        assert_spelling(dir, path, true)?;
        let file = dir
            .open_file(path, false)
            .context("context file unavailable or symlink")?;
        if file.metadata()?.len() > FILE_LIMIT as u64 {
            bail!("context_file_too_large");
        }
        let mut bytes = Vec::new();
        file.take(FILE_LIMIT as u64 + 1).read_to_end(&mut bytes)?;
        if bytes.len() > FILE_LIMIT {
            bail!("context_file_too_large");
        }
        total += bytes.len();
        if total > CONTEXT_LIMIT {
            bail!("context_total_too_large");
        }
        if bytes.contains(&0) {
            bail!("context_binary_file");
        }
        let text = String::from_utf8(bytes).context("context is not UTF-8 text")?;
        if text
            .chars()
            .any(|c| c.is_control() && !matches!(c, '\n' | '\r' | '\t'))
        {
            bail!("context_binary_file");
        }
        evidence.push(EvidenceFile {
            path: path.clone(),
            hash: hash_bytes(text.as_bytes()),
            text,
        });
    }
    Ok(evidence)
}

// The viewer never follows symlinks or relaxes evidence selection. In particular,
// the filesystem's original Unicode spelling is retained (macOS may use NFD).
pub const VIEWER_FILE_LIMIT: usize = 2 * 1024 * 1024;
const VIEWER_MAX_FILES: usize = 2000;
const VIEWER_MAX_DEPTH: usize = 12;
const VIEWER_MAX_ENTRIES: usize = 20_000;
const VIEWER_SCAN_BYTES: usize = 32 * 1024 * 1024;
#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ViewerKind {
    Markdown,
    Text,
}
#[derive(Clone, Debug, Serialize)]
pub struct ViewerFile {
    pub path: String,
    pub name: String,
    pub kind: ViewerKind,
    pub size: u64,
}
#[derive(Clone, Debug, Serialize)]
pub struct ViewerEntry {
    pub path: String,
    pub name: String,
    #[serde(rename = "type")]
    pub entry_type: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<ViewerKind>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<ViewerEntry>,
}
#[derive(Debug, Serialize)]
pub struct ViewerTree {
    pub entries: Vec<ViewerEntry>,
    pub files: Vec<ViewerFile>,
    pub truncated: bool,
}
#[derive(Debug, Serialize)]
pub struct ViewerContent {
    pub path: String,
    pub name: String,
    pub kind: ViewerKind,
    pub content: String,
    pub size: u64,
    pub hash: String,
}
#[derive(Default)]
struct ViewerBudget {
    files: Vec<ViewerFile>,
    entries: usize,
    bytes: usize,
    truncated: bool,
}
fn viewer_name(name: &str) -> bool {
    !name.is_empty()
        && name != "."
        && name != ".."
        && name.len() <= 255
        && !name
            .chars()
            .any(|c| c.is_control() || matches!(c, '/' | '\\'))
        && !excluded(name)
        && !matches!(
            name.to_ascii_lowercase().as_str(),
            "coverage"
                | "__pycache__"
                | "venv"
                | "env"
                | "out"
                | "tmp"
                | "temp"
                | "cache"
                | "logs"
                | "auth.json"
                | "auth.toml"
                | "auth.yaml"
                | "auth.yml"
                | "credentials"
                | "passwd"
                | "shadow"
        )
        && !name.to_ascii_lowercase().contains("apikey")
        && !name.to_ascii_lowercase().contains("api_key")
}
fn viewer_parts(path: &str) -> Result<Vec<String>> {
    let parts: Vec<_> = path.split('/').map(str::to_owned).collect();
    if path.len() > 4096
        || parts.len() > VIEWER_MAX_DEPTH + 1
        || parts.iter().any(|p| !viewer_name(p))
    {
        bail!("file path is not viewable");
    }
    Ok(parts)
}
fn viewer_spelling(dir: &Directory, requested: &str) -> Result<String> {
    let names = dir.names()?;
    if names.iter().any(|name| name == requested) {
        return Ok(requested.to_owned());
    }
    let normalized: String = requested.nfc().collect();
    let mut equivalents = names
        .into_iter()
        .filter(|name| name.nfc().eq(normalized.chars()));
    let name = equivalents
        .next()
        .ok_or_else(|| anyhow!("file unavailable"))?;
    if equivalents.next().is_some() || !viewer_name(&name) {
        bail!("ambiguous file spelling");
    }
    Ok(name)
}
fn viewer_kind(name: &str) -> Option<ViewerKind> {
    let lower = name.to_ascii_lowercase();
    let extension = lower.rsplit_once('.').map(|(_, e)| e).unwrap_or("");
    if matches!(extension, "md" | "markdown" | "mdown") {
        return Some(ViewerKind::Markdown);
    }
    if matches!(
        lower.as_str(),
        "dockerfile" | "makefile" | "license" | "readme"
    ) || matches!(
        extension,
        "txt"
            | "rst"
            | "go"
            | "js"
            | "mjs"
            | "cjs"
            | "jsx"
            | "ts"
            | "tsx"
            | "py"
            | "java"
            | "kt"
            | "kts"
            | "c"
            | "h"
            | "cc"
            | "cpp"
            | "hpp"
            | "cs"
            | "rs"
            | "rb"
            | "php"
            | "swift"
            | "sh"
            | "bash"
            | "zsh"
            | "fish"
            | "sql"
            | "lua"
            | "r"
            | "pl"
            | "scala"
            | "dart"
            | "vue"
            | "svelte"
            | "html"
            | "htm"
            | "css"
            | "scss"
            | "less"
            | "json"
            | "yaml"
            | "yml"
            | "toml"
            | "ini"
            | "conf"
            | "cfg"
            | "properties"
            | "xml"
            | "gradle"
            | "mod"
            | "sum"
            | "lock"
    ) {
        Some(ViewerKind::Text)
    } else {
        None
    }
}
fn viewer_text(file: File) -> Result<String> {
    let metadata = file.metadata()?;
    if metadata.len() > VIEWER_FILE_LIMIT as u64 || metadata.nlink() != 1 {
        bail!("viewable file exceeds 2 MiB or has external hard links");
    }
    let mut bytes = Vec::new();
    file.take(VIEWER_FILE_LIMIT as u64 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > VIEWER_FILE_LIMIT || bytes.contains(&0) {
        bail!("file is too large or binary");
    }
    String::from_utf8(bytes).context("file is not UTF-8 text")
}
fn viewer_walk(
    dir: &Directory,
    prefix: &str,
    depth: usize,
    budget: &mut ViewerBudget,
) -> Result<Vec<ViewerEntry>> {
    let mut entries = Vec::new();
    for name in dir.names()? {
        if budget.entries >= VIEWER_MAX_ENTRIES
            || budget.files.len() >= VIEWER_MAX_FILES
            || budget.bytes >= VIEWER_SCAN_BYTES
        {
            budget.truncated = true;
            break;
        }
        budget.entries += 1;
        if !viewer_name(&name) {
            continue;
        }
        let path = if prefix.is_empty() {
            name.clone()
        } else {
            format!("{prefix}/{name}")
        };
        if let Ok(child) = dir.open_dir(&name) {
            if depth >= VIEWER_MAX_DEPTH {
                budget.truncated = true;
                continue;
            }
            let children = viewer_walk(&child, &path, depth + 1, budget)?;
            if !children.is_empty() {
                entries.push(ViewerEntry {
                    path,
                    name,
                    entry_type: "directory",
                    kind: None,
                    size: None,
                    children,
                });
            }
        } else if let Some(kind) = viewer_kind(&name) {
            let Ok(file) = dir.open_file(&name, false) else {
                continue;
            };
            let size = file.metadata()?.len();
            if size > VIEWER_FILE_LIMIT as u64 {
                continue;
            }
            if budget.bytes.saturating_add(size as usize) > VIEWER_SCAN_BYTES {
                budget.truncated = true;
                break;
            }
            let Ok(content) = viewer_text(file) else {
                continue;
            };
            if budget.bytes.saturating_add(content.len()) > VIEWER_SCAN_BYTES {
                budget.truncated = true;
                break;
            }
            budget.bytes += content.len();
            let size = content.len() as u64;
            budget.files.push(ViewerFile {
                path: path.clone(),
                name: name.clone(),
                kind,
                size,
            });
            entries.push(ViewerEntry {
                path,
                name,
                entry_type: "file",
                kind: Some(kind),
                size: Some(size),
                children: Vec::new(),
            });
        }
    }
    Ok(entries)
}
