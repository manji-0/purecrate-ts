//! The input must also compile. The subset checks see syntax and types but
//! erase borrows and do not track moves or lifetimes, so rustc has the last
//! word: `check` succeeding means the crate compiles as a library.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

pub const EDITION: &str = "2021";

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

/// Compiles `src` to metadata only. `RUSTC` overrides the binary, as for cargo.
pub fn compile(src: &Path) -> Result<(), Failure> {
    let rustc = env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
    let out_dir = scratch();
    fs::create_dir_all(&out_dir).map_err(|e| Failure::Other(format!("mkdir {}: {e}", out_dir.display())))?;
    let output = Command::new(&rustc)
        .args(["--edition", EDITION, "--crate-type", "lib", "--emit=metadata"])
        .args(["--cap-lints", "allow", "--error-format=short", "--color=never", "--out-dir"])
        .arg(&out_dir)
        .arg(src)
        .output();
    fs::remove_dir_all(&out_dir).ok();
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

fn scratch() -> PathBuf {
    env::temp_dir().join(format!("purecrate-rustc-{}", std::process::id()))
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
            Some(RustcError {
                at: at.to_string(),
                code,
                message: message.to_string(),
            })
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
                RustcError {
                    at: "src/lib.rs:9:1".into(),
                    code: None,
                    message: "expected item, found `}`".into(),
                },
            ]
        );
        assert_eq!(parse_short(stderr)[0].line(), "src/lib.rs:3:5: [rustc/E0308] mismatched types");
    }
}
