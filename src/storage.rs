//! SQLite state, receipts and durable events share a single short transaction.
use crate::files::{Directory, ProjectFiles};
use anyhow::{Context, Result, anyhow, bail};
use fs2::FileExt;
use rusqlite::{Connection, OpenFlags, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    ffi::{CStr, c_char, c_int},
    fs::File,
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
    sync::OnceLock,
};

const SCHEMA_VERSION: i64 = 1;
#[derive(Clone, Debug)]
pub struct ReceiptInput {
    pub command_id: String,
    pub payload_hash: String,
    pub response: Value,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Receipt {
    pub payload_hash: String,
    pub response: Value,
}
#[derive(Clone, Debug)]
pub struct EventInput {
    pub session_id: Option<String>,
    pub kind: String,
    pub payload: Value,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EventRecord {
    pub cursor: u64,
    pub session_id: Option<String>,
    pub kind: String,
    pub payload: Value,
}

/// Not Clone: one connection and one OS lock belong to the controller.
/// Connection closes before its pinned state directory and lock handles.
pub struct Store {
    connection: Connection,
    root: PathBuf,
    _state: Directory,
    _lock: File,
    _root: Directory,
}

// SQLite's default xFullPathname resolves aliases into mutable absolute paths.
// Preserve an internally generated synthetic path and route Unix VFS pathname
// syscalls through its pinned dirfd. This works on macOS (where /dev/fd directory
// traversal is unavailable) and Linux, retaining SQLite's own I/O and locking.
// The wrapper is immutable after one-time registration and outlives all DBs.
fn register_dirfd_vfs() -> Result<()> {
    static REGISTERED: OnceLock<std::result::Result<(), String>> = OnceLock::new();
    REGISTERED
        .get_or_init(|| unsafe {
            let base = rusqlite::ffi::sqlite3_vfs_find(std::ptr::null());
            if base.is_null() {
                return Err("SQLite default VFS unavailable".into());
            }
            let set = (*base)
                .xSetSystemCall
                .ok_or_else(|| "SQLite Unix syscall routing unavailable".to_owned())?;
            // sqlite3_syscall_ptr is an erased C function pointer; SQLite casts
            // these back to the exact signatures listed in its Unix syscall table.
            macro_rules! route {
                ($name:literal, $function:expr, $signature:ty) => {{
                    let function: $signature = $function;
                    let erased: unsafe extern "C" fn() = std::mem::transmute(function);
                    if set(base, $name.as_ptr(), Some(erased)) != rusqlite::ffi::SQLITE_OK {
                        return Err(format!("SQLite syscall unavailable: {:?}", $name));
                    }
                }};
            }
            route!(
                c"open",
                routed_open,
                unsafe extern "C" fn(*const c_char, c_int, c_int) -> c_int
            );
            route!(
                c"unlink",
                routed_unlink,
                unsafe extern "C" fn(*const c_char) -> c_int
            );
            route!(
                c"access",
                routed_access,
                unsafe extern "C" fn(*const c_char, c_int) -> c_int
            );
            route!(
                c"stat",
                routed_stat,
                unsafe extern "C" fn(*const c_char, *mut libc::stat) -> c_int
            );
            route!(
                c"lstat",
                routed_lstat,
                unsafe extern "C" fn(*const c_char, *mut libc::stat) -> c_int
            );
            let mut wrapper = Box::new(std::ptr::read(base));
            wrapper.zName = c"architalk-dirfd".as_ptr();
            wrapper.pNext = std::ptr::null_mut();
            wrapper.xFullPathname = Some(dirfd_full_pathname);
            let ptr = Box::into_raw(wrapper);
            let code = rusqlite::ffi::sqlite3_vfs_register(ptr, 0);
            if code != rusqlite::ffi::SQLITE_OK {
                return Err(format!("VFS registration failed: {code}"));
            }
            Ok(())
        })
        .as_ref()
        .map(|_| ())
        .map_err(|s| anyhow!(s.clone()))
}
unsafe extern "C" fn dirfd_full_pathname(
    _vfs: *mut rusqlite::ffi::sqlite3_vfs,
    input: *const c_char,
    size: c_int,
    output: *mut c_char,
) -> c_int {
    if input.is_null() || output.is_null() {
        return rusqlite::ffi::SQLITE_CANTOPEN;
    }
    let path = unsafe { CStr::from_ptr(input) }.to_bytes();
    let valid = path
        .strip_prefix(b"/__architalk_fd__/")
        .is_some_and(|rest| {
            let mut parts = rest.split(|v| *v == b'/');
            let descriptor = parts.next().unwrap_or_default();
            !descriptor.is_empty()
                && descriptor.iter().all(u8::is_ascii_digit)
                && parts.next() == Some(b"workspace.db".as_slice())
                && parts.next().is_none()
        });
    if !valid || size <= 0 || path.len() >= size as usize {
        return rusqlite::ffi::SQLITE_CANTOPEN;
    }
    unsafe {
        std::ptr::copy_nonoverlapping(input, output, path.len() + 1);
    }
    rusqlite::ffi::SQLITE_OK
}

// None means ordinary SQLite usage: preserve the process default VFS behavior.
// Our marker can address only a held directory and the SQLite-owned filenames.
unsafe fn routed_path<'a>(path: *const c_char) -> Option<(c_int, &'a CStr)> {
    if path.is_null() {
        return None;
    }
    let bytes = unsafe { CStr::from_ptr(path) }.to_bytes();
    let rest = bytes.strip_prefix(b"/__architalk_fd__/")?;
    let slash = rest.iter().position(|b| *b == b'/');
    let descriptor = &rest[..slash.unwrap_or(rest.len())];
    if descriptor.is_empty() || !descriptor.iter().all(u8::is_ascii_digit) {
        return None;
    }
    let fd: c_int = std::str::from_utf8(descriptor).ok()?.parse().ok()?;
    let name = if let Some(slash) = slash {
        &rest[slash + 1..]
    } else {
        b""
    };
    if name.is_empty() {
        return Some((fd, c"."));
    }
    if !matches!(
        name,
        b"workspace.db" | b"workspace.db-journal" | b"workspace.db-wal" | b"workspace.db-shm"
    ) {
        return None;
    }
    // CStr points into the caller's nul-terminated pathname after its final '/'.
    let name_ptr = unsafe { path.add(bytes.len() - name.len()) };
    Some((fd, unsafe { CStr::from_ptr(name_ptr) }))
}
unsafe extern "C" fn routed_open(path: *const c_char, flags: c_int, mode: c_int) -> c_int {
    match unsafe { routed_path(path) } {
        Some((directory, name)) => {
            // Validate before any possible truncation. A hard link introduced
            // after Store::open must not turn a journal into an external write.
            let fd = unsafe {
                libc::openat(
                    directory,
                    name.as_ptr(),
                    (flags & !libc::O_TRUNC)
                        | libc::O_NOFOLLOW
                        | libc::O_CLOEXEC
                        | libc::O_NONBLOCK,
                    mode,
                )
            };
            if fd < 0 {
                return fd;
            }
            if name.to_bytes() != b"." {
                let mut metadata = std::mem::MaybeUninit::<libc::stat>::uninit();
                let valid = unsafe { libc::fstat(fd, metadata.as_mut_ptr()) } == 0 && {
                    let metadata = unsafe { metadata.assume_init() };
                    metadata.st_mode & libc::S_IFMT == libc::S_IFREG
                        && metadata.st_nlink == 1
                        && metadata.st_uid == unsafe { libc::geteuid() }
                };
                if !valid {
                    unsafe {
                        libc::close(fd);
                        set_errno(libc::EPERM);
                    }
                    return -1;
                }
                if flags & libc::O_TRUNC != 0 && unsafe { libc::ftruncate(fd, 0) } != 0 {
                    unsafe {
                        libc::close(fd);
                    }
                    return -1;
                }
            }
            fd
        }
        None => unsafe { libc::open(path, flags, mode) },
    }
}
unsafe fn set_errno(error: c_int) {
    #[cfg(target_os = "macos")]
    unsafe {
        *libc::__error() = error;
    }
    #[cfg(target_os = "linux")]
    unsafe {
        *libc::__errno_location() = error;
    }
}
unsafe extern "C" fn routed_unlink(path: *const c_char) -> c_int {
    match unsafe { routed_path(path) } {
        Some((fd, name)) => unsafe { libc::unlinkat(fd, name.as_ptr(), 0) },
        None => unsafe { libc::unlink(path) },
    }
}
unsafe extern "C" fn routed_access(path: *const c_char, mode: c_int) -> c_int {
    match unsafe { routed_path(path) } {
        Some((fd, name)) => unsafe {
            libc::faccessat(fd, name.as_ptr(), mode, libc::AT_SYMLINK_NOFOLLOW)
        },
        None => unsafe { libc::access(path, mode) },
    }
}
unsafe extern "C" fn routed_stat(path: *const c_char, output: *mut libc::stat) -> c_int {
    match unsafe { routed_path(path) } {
        Some((fd, name)) => unsafe {
            libc::fstatat(fd, name.as_ptr(), output, libc::AT_SYMLINK_NOFOLLOW)
        },
        None => unsafe { libc::stat(path, output) },
    }
}

unsafe extern "C" fn routed_lstat(path: *const c_char, output: *mut libc::stat) -> c_int {
    match unsafe { routed_path(path) } {
        Some((fd, name)) => unsafe {
            libc::fstatat(fd, name.as_ptr(), output, libc::AT_SYMLINK_NOFOLLOW)
        },
        None => unsafe { libc::lstat(path, output) },
    }
}

impl Store {
    pub fn open(root: &Path) -> Result<Self> {
        let root = root.canonicalize().context("project root unavailable")?;
        let root_dir = Directory::root(&root)?;
        // The project inode lock survives replacement of the .architalk name.
        root_dir.lock_exclusive()?;
        match root_dir.open_dir(".architalk") {
            Ok(_) => {}
            Err(error)
                if error
                    .downcast_ref::<std::io::Error>()
                    .is_some_and(|e| e.kind() == std::io::ErrorKind::NotFound) =>
            {
                root_dir.mkdir(".architalk", 0o700)?;
            }
            Err(error) => return Err(error.context("unsafe .architalk directory")),
        }
        let state = root_dir
            .open_dir(".architalk")
            .context("unsafe .architalk directory")?;
        state.make_private()?;
        let lock = state
            .open_file("workspace.lock", true)
            .context("unsafe workspace lock")?;
        ensure_owned_single_file(&lock)?;
        lock.try_lock_exclusive()
            .context("project_locked: another ArchiTalk process owns this project")?;
        // Inspect every SQLite sidecar before opening, through the pinned dirfd.
        for name in [
            "workspace.db-journal",
            "workspace.db-wal",
            "workspace.db-shm",
        ] {
            match state.open_file(name, false) {
                Ok(file) => ensure_owned_single_file(&file)?,
                Err(error)
                    if error
                        .downcast_ref::<std::io::Error>()
                        .is_some_and(|e| e.kind() == std::io::ErrorKind::NotFound) => {}
                Err(error) => return Err(error.context("unsafe SQLite sidecar")),
            }
        }
        let db = state
            .open_file("workspace.db", true)
            .context("unsafe workspace database")?;
        ensure_owned_single_file(&db)?;
        register_dirfd_vfs()?;
        let db_path = state.fd_path().join("workspace.db");
        let mut connection = Connection::open_with_flags_and_vfs(
            &db_path,
            OpenFlags::SQLITE_OPEN_READ_WRITE
                | OpenFlags::SQLITE_OPEN_CREATE
                | OpenFlags::SQLITE_OPEN_NOFOLLOW
                | OpenFlags::SQLITE_OPEN_NO_MUTEX,
            "architalk-dirfd",
        )
        .context("workspace database open failed; original retained")?;
        connection.busy_timeout(std::time::Duration::from_secs(2))?;
        let version: i64 = connection
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .context("workspace database corrupt; original retained")?;
        if version > SCHEMA_VERSION {
            bail!(
                "unsupported_schema: database v{version} is newer than supported v{SCHEMA_VERSION}"
            );
        }
        if version < 0 {
            bail!("unsupported_schema: invalid version");
        }
        let check: String = connection
            .query_row("PRAGMA quick_check(1)", [], |r| r.get(0))
            .context("workspace database corrupt; original retained")?;
        if check != "ok" {
            bail!("workspace database corrupt; original retained");
        }
        if version == 0 {
            // Existing unversioned SQLite files are never changed without a backup.
            if db.metadata()?.len() > 0 {
                let name = format!("workspace.pre-v1-{}.db", uuid::Uuid::new_v4());
                state.backup_file(&name, &db)?;
            }
            let tx = connection.transaction()?;
            tx.execute_batch("CREATE TABLE workspace (id INTEGER PRIMARY KEY CHECK(id=1), snapshot TEXT NOT NULL);
                CREATE TABLE receipts (command_id TEXT PRIMARY KEY, payload_hash TEXT NOT NULL, response TEXT NOT NULL);
                CREATE TABLE events (cursor INTEGER PRIMARY KEY AUTOINCREMENT, session_id TEXT, kind TEXT NOT NULL, payload TEXT NOT NULL);
                CREATE INDEX events_by_session ON events(session_id,cursor);
                PRAGMA user_version=1;")?;
            tx.commit()?;
        }
        connection.pragma_update(None, "journal_mode", "DELETE")?;
        connection.pragma_update(None, "synchronous", "FULL")?;
        connection.pragma_update(None, "foreign_keys", "ON")?;
        state.sync()?;
        Ok(Self {
            connection,
            root,
            _state: state,
            _lock: lock,
            _root: root_dir,
        })
    }
    pub fn project_files(&self) -> Result<ProjectFiles> {
        Ok(ProjectFiles::from_directory(self._root.try_clone()?))
    }
    pub fn root(&self) -> &Path {
        &self.root
    }
    pub fn load_workspace(&self) -> Result<Option<Value>> {
        let value: Option<String> = self
            .connection
            .query_row("SELECT snapshot FROM workspace WHERE id=1", [], |r| {
                r.get(0)
            })
            .optional()?;
        value
            .map(|text| serde_json::from_str(&text).context("invalid workspace snapshot"))
            .transpose()
    }
    pub fn find_receipt(&self, command_id: &str) -> Result<Option<Receipt>> {
        let value: Option<(String, String)> = self
            .connection
            .query_row(
                "SELECT payload_hash,response FROM receipts WHERE command_id=?1",
                [command_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        value
            .map(|(payload_hash, response)| {
                Ok(Receipt {
                    payload_hash,
                    response: serde_json::from_str(&response)?,
                })
            })
            .transpose()
    }
    pub fn commit_workspace(
        &mut self,
        snapshot: &Value,
        command: Option<ReceiptInput>,
        event: Option<EventInput>,
    ) -> Result<Option<EventRecord>> {
        let snapshot = serde_json::to_string(snapshot)?;
        let tx = self.connection.transaction()?;
        // Duplicate receipt insert is a hard error: rollback includes snapshot/event.
        if let Some(command) = command {
            tx.execute(
                "INSERT INTO receipts(command_id,payload_hash,response) VALUES(?1,?2,?3)",
                params![
                    command.command_id,
                    command.payload_hash,
                    serde_json::to_string(&command.response)?
                ],
            )?;
        }
        tx.execute("INSERT INTO workspace(id,snapshot) VALUES(1,?1) ON CONFLICT(id) DO UPDATE SET snapshot=excluded.snapshot", [&snapshot])?;
        let event = if let Some(event) = event {
            tx.execute(
                "INSERT INTO events(session_id,kind,payload) VALUES(?1,?2,?3)",
                params![
                    event.session_id,
                    event.kind,
                    serde_json::to_string(&event.payload)?
                ],
            )?;
            Some(EventRecord {
                cursor: tx.last_insert_rowid() as u64,
                session_id: event.session_id,
                kind: event.kind,
                payload: event.payload,
            })
        } else {
            None
        };
        tx.commit()?;
        Ok(event)
    }
    /// Replay is bounded; clients can reconnect with the last returned cursor.
    pub fn events_after(&self, session_id: Option<&str>, cursor: u64) -> Result<Vec<EventRecord>> {
        if cursor > i64::MAX as u64 {
            return Ok(Vec::new());
        }
        let mut statement = self.connection.prepare("SELECT cursor,session_id,kind,payload FROM events WHERE cursor>?1 AND (?2 IS NULL OR session_id=?2) ORDER BY cursor LIMIT 1000")?;
        let rows = statement.query_map(params![cursor as i64, session_id], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, Option<String>>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
            ))
        })?;
        rows.map(|row| {
            let (cursor, session_id, kind, payload) = row?;
            Ok(EventRecord {
                cursor: cursor as u64,
                session_id,
                kind,
                payload: serde_json::from_str(&payload)?,
            })
        })
        .collect()
    }
    pub fn latest_cursor(&self) -> Result<u64> {
        Ok(self
            .connection
            .query_row("SELECT COALESCE(MAX(cursor),0) FROM events", [], |r| {
                r.get::<_, i64>(0)
            })? as u64)
    }
}
fn ensure_owned_single_file(file: &File) -> Result<()> {
    let metadata = file.metadata()?;
    if metadata.uid() != unsafe { libc::geteuid() } || metadata.nlink() != 1 {
        bail!("unsafe state file ownership or hard link");
    }
    file.set_permissions(std::os::unix::fs::PermissionsExt::from_mode(0o600))?;
    Ok(())
}
