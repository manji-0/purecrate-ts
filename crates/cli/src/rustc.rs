//! The input must also compile. The subset checks see syntax and types but
//! erase borrows and do not track moves or lifetimes, so rustc has the last
//! word: `check` succeeding means the crate compiles as a library.
//!
//! The external crates the input may name are `serde`, for the derives a
//! server needs on the same types (design/04 §3), and `uuid`, for its `Uuid`
//! (design/01 §6). rustc gets stand-ins. serde's `Serialize`/`Deserialize`
//! derives expand to nothing: they add impls, never change the code that is
//! translated, so borrows and moves are checked the same. The real derive,
//! and what it requires (`TryFrom`, `Display`), is checked by the server's
//! own build. The `uuid` stand-in has the part of its API the subset
//! accepts, with the same signatures and traits.

use std::env;
use std::ffi::OsStr;
use std::fs::{self, OpenOptions};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime};

/// One rustc error, located in the input.
#[derive(Debug, PartialEq, Eq)]
pub struct RustcError {
    pub at: String,
    pub code: Option<String>,
    pub message: String,
}

impl RustcError {
    pub fn line(&self) -> String {
        let code = self.code.as_deref().unwrap_or("error");
        format!("{}: [rustc/{code}] {}", self.at, self.message)
    }
}

pub enum Failure {
    /// rustc ran and rejected the crate.
    Rejected(Vec<RustcError>),
    /// rustc could not be run, or failed without a located error.
    Other(String),
}

/// Compiles `src` to metadata only, as `edition`. `RUSTC` overrides the
/// binary, as for cargo.
pub fn compile(src: &Path, edition: &str) -> Result<(), Failure> {
    let rustc = env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
    let stubs = stubs(&rustc)?;
    let scratch = Scratch::create()?;
    let output = Command::new(&rustc)
        .args(["--edition", edition, "--crate-type", "lib", "--emit=metadata"])
        .arg("--extern")
        .arg(format!("serde={}", stubs.serde.display()))
        .arg("--extern")
        .arg(format!("uuid={}", stubs.uuid.display()))
        .arg("-L")
        .arg(&stubs.dir)
        .args(["--cap-lints", "allow", "--error-format=short", "--color=never", "--out-dir"])
        .arg(&scratch.path)
        .arg(src)
        .output();
    let output = output.map_err(|e| {
        Failure::Other(format!(
            "run {}: {e}; check needs rustc to confirm the input compiles (set RUSTC to its path)",
            Path::new(&rustc).display()
        ))
    })?;
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    let errors = parse_short(&stderr);
    if errors.is_empty() {
        return Err(Failure::Other(format!("rustc failed on {}:\n{}", src.display(), stderr.trim_end())));
    }
    Err(Failure::Rejected(errors))
}

const SERDE_DERIVE_STUB: &str = "extern crate proc_macro;
use proc_macro::TokenStream;
#[proc_macro_derive(Serialize, attributes(serde))]
pub fn serialize(_: TokenStream) -> TokenStream { TokenStream::new() }
#[proc_macro_derive(Deserialize, attributes(serde))]
pub fn deserialize(_: TokenStream) -> TokenStream { TokenStream::new() }
";

const SERDE_STUB: &str = "pub use serde_derive::{Deserialize, Serialize};
pub trait Serialize {}
pub trait Deserialize<'de>: Sized {}
pub mod ser { pub use super::Serialize; }
pub mod de { pub use super::Deserialize; pub trait DeserializeOwned {} }
";

const UUID_STUB: &str = "#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Uuid([u8; 16]);
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Error(());
impl Uuid {
    pub fn parse_str(_: &str) -> Result<Uuid, Error> { Err(Error(())) }
    pub const fn try_parse(_: &str) -> Result<Uuid, Error> { Err(Error(())) }
    pub const fn nil() -> Uuid { Uuid([0; 16]) }
}
impl core::fmt::Display for Uuid {
    fn fmt(&self, _: &mut core::fmt::Formatter<'_>) -> core::fmt::Result { Ok(()) }
}
impl core::fmt::Display for Error {
    fn fmt(&self, _: &mut core::fmt::Formatter<'_>) -> core::fmt::Result { Ok(()) }
}
impl std::error::Error for Error {}
";

/// Builds the stand-in `uuid` into `dir` and returns the rlib.
fn uuid_stub(rustc: &OsStr, dir: &Path) -> Result<PathBuf, Failure> {
    build_stub(rustc, dir, "uuid", UUID_STUB, &["--crate-type", "rlib"], "uuid")?;
    Ok(dir.join("libuuid.rlib"))
}

/// Builds the stand-in `serde` (and its derive crate) into `dir` and returns
/// the rlib.
fn serde_stub(rustc: &OsStr, dir: &Path) -> Result<PathBuf, Failure> {
    build_stub(rustc, dir, "serde_derive", SERDE_DERIVE_STUB, &["--crate-type", "proc-macro"], "serde")?;
    let derive =
        derive_artifact(dir).ok_or_else(|| Failure::Other("the serde derive stand-in was not built".into()))?;
    let extern_derive = format!("serde_derive={}", derive.display());
    build_stub(rustc, dir, "serde", SERDE_STUB, &["--crate-type", "rlib", "--extern", &extern_derive], "serde")?;
    Ok(dir.join("libserde.rlib"))
}

/// Compiles the stand-in crate `name` from `source` into `dir`; `what` names
/// the stand-in in an error.
fn build_stub(rustc: &OsStr, dir: &Path, name: &str, source: &str, args: &[&str], what: &str) -> Result<(), Failure> {
    let src = dir.join(format!("{name}.rs"));
    fs::write(&src, source).map_err(|e| Failure::Other(format!("write {}: {e}", src.display())))?;
    let output = Command::new(rustc)
        .args(["--edition", "2021", "--crate-name", name, "--cap-lints", "allow", "--out-dir"])
        .arg(dir)
        .args(args)
        .arg(&src)
        .output()
        .map_err(|e| {
            Failure::Other(format!(
                "run {}: {e}; check needs rustc to confirm the input compiles (set RUSTC to its path)",
                Path::new(rustc).display()
            ))
        })?;
    if output.status.success() {
        Ok(())
    } else {
        Err(Failure::Other(format!(
            "rustc could not build the {what} stand-in:\n{}",
            String::from_utf8_lossy(&output.stderr).trim_end()
        )))
    }
}

fn derive_artifact(dir: &Path) -> Option<PathBuf> {
    fs::read_dir(dir).ok()?.filter_map(|e| e.ok().map(|e| e.path())).find(|p| {
        let stem = p.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        (stem == "serde_derive" || stem == "libserde_derive") && p.extension().is_some_and(|e| e != "rs")
    })
}

struct Stubs {
    dir: PathBuf,
    serde: PathBuf,
    uuid: PathBuf,
}

/// Stand-ins keyed by `rustc -vV` and the purecrate-ts version, so `check`
/// does not rebuild the serde proc-macro on every run. Override the root
/// with `XDG_CACHE_HOME`.
fn stubs(rustc: &OsStr) -> Result<Stubs, Failure> {
    load_or_build_stubs(rustc, &stub_cache_dir(rustc)?)
}

fn stub_cache_dir(rustc: &OsStr) -> Result<PathBuf, Failure> {
    Ok(cache_root().join(rustc_id(rustc)?))
}

fn cache_root() -> PathBuf {
    if let Some(p) = env::var_os("XDG_CACHE_HOME").filter(|s| !s.is_empty()) {
        return PathBuf::from(p).join("purecrate-ts").join("rustc-stubs");
    }
    if let Some(h) = env::var_os("HOME").filter(|s| !s.is_empty()) {
        return PathBuf::from(h).join(".cache").join("purecrate-ts").join("rustc-stubs");
    }
    env::temp_dir().join("purecrate-ts-rustc-stubs")
}

fn rustc_id(rustc: &OsStr) -> Result<String, Failure> {
    let output = Command::new(rustc).arg("-vV").output().map_err(|e| {
        Failure::Other(format!(
            "run {}: {e}; check needs rustc to confirm the input compiles (set RUSTC to its path)",
            Path::new(rustc).display()
        ))
    })?;
    if !output.status.success() {
        return Err(Failure::Other(format!(
            "rustc -vV failed:\n{}",
            String::from_utf8_lossy(&output.stderr).trim_end()
        )));
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let field = |key: &str| {
        text.lines()
            .find_map(|l| l.strip_prefix(key)?.strip_prefix(": ").map(str::trim))
            .unwrap_or("unknown")
            .replace('/', "_")
    };
    Ok(format!("{}-{}-{}", env!("CARGO_PKG_VERSION"), field("release"), field("host")))
}

fn stubs_ready(dir: &Path) -> bool {
    dir.join("ready").is_file()
        && dir.join("libserde.rlib").is_file()
        && dir.join("libuuid.rlib").is_file()
        && derive_artifact(dir).is_some()
}

fn cached_stubs(dir: &Path) -> Option<Stubs> {
    stubs_ready(dir).then(|| Stubs {
        dir: dir.to_path_buf(),
        serde: dir.join("libserde.rlib"),
        uuid: dir.join("libuuid.rlib"),
    })
}

/// Exclusive create of `dir/building`. Parallel `check` processes (and the
/// test binary's parallel tests) would otherwise rustc the proc-macro into
/// the same `--out-dir` and lose each other's `.rcgu.o` files.
struct BuildLock {
    path: PathBuf,
}

impl Drop for BuildLock {
    fn drop(&mut self) {
        if !self.path.as_os_str().is_empty() {
            let _ = fs::remove_file(&self.path);
        }
    }
}

fn exclusive_build_lock(dir: &Path) -> Result<BuildLock, Failure> {
    let path = dir.join("building");
    for i in 0.. {
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(_) => return Ok(BuildLock { path }),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                if lock_is_stale(&path) {
                    let _ = fs::remove_file(&path);
                    continue;
                }
                if cached_stubs(dir).is_some() {
                    return Ok(BuildLock { path: PathBuf::new() });
                }
                std::thread::sleep(Duration::from_millis(20 + 10 * i.min(20) as u64));
                if i >= 500 {
                    return Err(Failure::Other(format!(
                        "timed out waiting to build rustc stand-ins in {}",
                        dir.display()
                    )));
                }
            }
            Err(e) => return Err(Failure::Other(format!("lock {}: {e}", path.display()))),
        }
    }
    Err(Failure::Other("could not lock the rustc stand-in cache".into()))
}

fn lock_is_stale(path: &Path) -> bool {
    let Ok(meta) = fs::metadata(path) else {
        return true;
    };
    let Ok(modified) = meta.modified() else {
        return false;
    };
    SystemTime::now().duration_since(modified).is_ok_and(|d| d > Duration::from_secs(30))
}

fn load_or_build_stubs(rustc: &OsStr, dir: &Path) -> Result<Stubs, Failure> {
    if let Some(s) = cached_stubs(dir) {
        return Ok(s);
    }
    fs::create_dir_all(dir).map_err(|e| Failure::Other(format!("mkdir {}: {e}", dir.display())))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(dir, fs::Permissions::from_mode(0o700));
    }
    let _lock = exclusive_build_lock(dir)?;
    if let Some(s) = cached_stubs(dir) {
        return Ok(s);
    }
    uuid_stub(rustc, dir)?;
    serde_stub(rustc, dir)?;
    fs::write(dir.join("ready"), b"").map_err(|e| Failure::Other(format!("write {}/ready: {e}", dir.display())))?;
    Ok(Stubs { dir: dir.to_path_buf(), serde: dir.join("libserde.rlib"), uuid: dir.join("libuuid.rlib") })
}

/// A unique 0700 directory in the process temp dir. Created exclusively so
/// another user cannot have pre-created the path (or a symlink to it) for a
/// guessable name, and removed on drop so early returns cannot leave it.
struct Scratch {
    path: PathBuf,
}

impl Scratch {
    fn create() -> Result<Self, Failure> {
        let tmp = env::temp_dir();
        for _ in 0..16 {
            let path = tmp.join(format!("purecrate-rustc-{}", random_suffix()));
            match mkdir_exclusive(&path) {
                Ok(()) => return Ok(Self { path }),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(Failure::Other(format!("mkdir {}: {e}", path.display()))),
            }
        }
        Err(Failure::Other("could not create a unique rustc scratch directory".into()))
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn mkdir_exclusive(path: &Path) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        std::fs::DirBuilder::new().mode(0o700).create(path)
    }
    #[cfg(not(unix))]
    {
        fs::create_dir(path)
    }
}

fn random_suffix() -> String {
    let mut buf = [0u8; 16];
    if let Ok(mut f) = fs::File::open("/dev/urandom") {
        if f.read_exact(&mut buf).is_ok() {
            return hex(&buf);
        }
    }
    let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    format!("{t:x}-{}", std::process::id())
}

fn hex(bytes: &[u8]) -> String {
    const H: &[u8] = b"0123456789abcdef";
    let mut s = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        s.push(H[(b >> 4) as usize] as char);
        s.push(H[(b & 0xf) as usize] as char);
    }
    s
}

/// `--error-format=short` lines: `path:line:col: error[E0308]: message`.
fn parse_short(stderr: &str) -> Vec<RustcError> {
    stderr
        .lines()
        .filter_map(|line| {
            let (at, rest) = line.split_once(": error")?;
            let (code, message) = match rest.strip_prefix('[') {
                Some(r) => {
                    let (code, message) = r.split_once("]: ")?;
                    (Some(code.to_string()), message)
                }
                None => (None, rest.strip_prefix(": ")?),
            };
            Some(RustcError { at: at.to_string(), code, message: message.to_string() })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_errors_keep_location_code_and_message() {
        let stderr = "\
src/lib.rs:3:5: error[E0308]: mismatched types
src/lib.rs:9:1: error: expected item, found `}`
error: aborting due to 2 previous errors
";
        assert_eq!(
            parse_short(stderr),
            [
                RustcError {
                    at: "src/lib.rs:3:5".into(),
                    code: Some("E0308".into()),
                    message: "mismatched types".into(),
                },
                RustcError { at: "src/lib.rs:9:1".into(), code: None, message: "expected item, found `}`".into() },
            ]
        );
        assert_eq!(parse_short(stderr)[0].line(), "src/lib.rs:3:5: [rustc/E0308] mismatched types");
    }

    #[test]
    fn scratch_is_exclusive_0700_and_removed_on_drop() {
        let a = Scratch::create().unwrap_or_else(|_| panic!("first scratch"));
        let b = Scratch::create().unwrap_or_else(|_| panic!("second scratch"));
        assert_ne!(a.path, b.path);
        assert!(a.path.starts_with(env::temp_dir()));
        let predictable = env::temp_dir().join(format!("purecrate-rustc-{}", std::process::id()));
        assert_ne!(a.path, predictable);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(&a.path).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o700);
        }
        let path = a.path.clone();
        drop(a);
        assert!(!path.exists());
        drop(b);
    }

    #[test]
    fn stubs_are_reused_from_the_cache_without_rustc() {
        let n = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
        let dir = env::temp_dir().join(format!("purecrate-stub-cache-{}-{n}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let rustc = env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
        load_or_build_stubs(&rustc, &dir).unwrap_or_else(|_| panic!("build stubs"));
        assert!(stubs_ready(&dir));
        load_or_build_stubs(OsStr::new("/nonexistent/rustc"), &dir).unwrap_or_else(|_| panic!("reuse stubs"));
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn concurrent_stub_builds_share_one_cache() {
        let n = SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
        let dir = env::temp_dir().join(format!("purecrate-stub-race-{}-{n}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let rustc = env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
        std::thread::scope(|s| {
            for _ in 0..8 {
                s.spawn(|| {
                    load_or_build_stubs(&rustc, &dir).unwrap_or_else(|_| panic!("concurrent stubs"));
                });
            }
        });
        assert!(stubs_ready(&dir));
        fs::remove_dir_all(&dir).ok();
    }
}
