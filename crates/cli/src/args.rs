//! Command-line parsing. Resolving the crate name reads `Cargo.toml` if present.

use std::fs;
use std::path::{Path, PathBuf};

pub const USAGE: &str = "\
usage:
  purecrate-ts build <crate-path> --out <dir> [--name <crate>]
  purecrate-ts check <crate-path> [--out <dir>] [--name <crate>]

<crate-path> is a crate directory (reads src/lib.rs) or a single .rs file.
check without --out only runs the subset checks; with --out it also fails
when <dir> differs from what build would write.";

#[derive(Debug, PartialEq, Eq)]
pub struct Input {
    pub src: PathBuf,
    pub name: String,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Command {
    Build { input: Input, out: PathBuf },
    Check { input: Input, out: Option<PathBuf> },
}

pub fn parse(args: &[String]) -> Result<Command, String> {
    let (verb, rest) = args.split_first().ok_or("missing command")?;
    let mut path: Option<PathBuf> = None;
    let mut name: Option<String> = None;
    let mut out: Option<PathBuf> = None;
    let mut it = rest.iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--name" => name = Some(it.next().ok_or("--name needs a value")?.clone()),
            "--out" => out = Some(PathBuf::from(it.next().ok_or("--out needs a value")?)),
            flag if flag.starts_with('-') => return Err(format!("unknown flag {flag}")),
            other if path.is_some() => return Err(format!("unexpected argument {other}")),
            other => path = Some(PathBuf::from(other)),
        }
    }
    let path = path.ok_or("missing <crate-path>")?;
    let input = resolve_input(&path, name)?;
    match verb.as_str() {
        "build" => Ok(Command::Build {
            input,
            out: out.ok_or("build needs --out <dir>")?,
        }),
        "check" => Ok(Command::Check { input, out }),
        other => Err(format!("unknown command {other}")),
    }
}

fn resolve_input(path: &Path, name: Option<String>) -> Result<Input, String> {
    let (src, crate_dir) = if path.is_dir() {
        (path.join("src/lib.rs"), Some(path.to_path_buf()))
    } else {
        let crate_dir = path
            .parent()
            .filter(|p| p.file_name().is_some_and(|n| n == "src"))
            .and_then(Path::parent)
            .map(Path::to_path_buf);
        (path.to_path_buf(), crate_dir)
    };
    let name = match name {
        Some(n) if n.is_empty() => return Err("--name must not be empty".into()),
        Some(n) => n,
        None => infer_name(&src, crate_dir.as_deref())
            .ok_or_else(|| format!("cannot infer a crate name for {}; pass --name", src.display()))?,
    };
    Ok(Input { src, name })
}

/// `Cargo.toml` `[package] name`, else the crate directory, else the file stem.
fn infer_name(src: &Path, crate_dir: Option<&Path>) -> Option<String> {
    if let Some(dir) = crate_dir {
        if let Some(n) = fs::read_to_string(dir.join("Cargo.toml"))
            .ok()
            .and_then(|t| package_name(&t))
        {
            return Some(n);
        }
        if let Some(n) = dir.file_name() {
            return Some(n.to_string_lossy().into_owned());
        }
    }
    src.file_stem().map(|s| s.to_string_lossy().into_owned())
}

fn package_name(manifest: &str) -> Option<String> {
    let mut in_package = false;
    for line in manifest.lines().map(str::trim) {
        if line.starts_with('[') {
            in_package = line == "[package]";
            continue;
        }
        if !in_package {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else { continue };
        if key.trim() == "name" {
            return Some(value.trim().trim_matches('"').to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(xs: &[&str]) -> Vec<String> {
        xs.iter().map(|s| s.to_string()).collect()
    }

    fn examples() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples")
    }

    #[test]
    fn crate_directory_reads_lib_rs_and_infers_the_name() {
        let dir = examples().join("counter");
        let cmd = parse(&args(&["check", dir.to_str().unwrap()])).unwrap();
        assert_eq!(
            cmd,
            Command::Check {
                input: Input {
                    src: dir.join("src/lib.rs"),
                    name: "counter".into()
                },
                out: None
            }
        );
    }

    #[test]
    fn lib_rs_path_uses_the_crate_directory_name() {
        let src = examples().join("counter/src/lib.rs");
        let Command::Build { input, out } =
            parse(&args(&["build", src.to_str().unwrap(), "--out", "o"])).unwrap()
        else {
            panic!("expected build");
        };
        assert_eq!(input.name, "counter");
        assert_eq!(out, PathBuf::from("o"));
    }

    #[test]
    fn loose_file_uses_its_stem_and_name_overrides() {
        let Command::Check { input, .. } = parse(&args(&["check", "/tmp/shapes.rs"])).unwrap() else {
            panic!("expected check");
        };
        assert_eq!(input.name, "shapes");
        let Command::Check { input, .. } =
            parse(&args(&["check", "/tmp/shapes.rs", "--name", "geo"])).unwrap()
        else {
            panic!("expected check");
        };
        assert_eq!(input.name, "geo");
    }

    #[test]
    fn manifest_package_name_wins_over_directory() {
        let toml = "[workspace]\nname = \"no\"\n[package]\nname = \"real_name\"\nversion = \"0.1.0\"\n";
        assert_eq!(package_name(toml).as_deref(), Some("real_name"));
        assert_eq!(package_name("[dependencies]\nname = \"x\"\n"), None);
    }

    #[test]
    fn usage_errors() {
        assert!(parse(&args(&[])).is_err());
        assert!(parse(&args(&["build", "x.rs"])).unwrap_err().contains("--out"));
        assert!(parse(&args(&["emit", "x.rs"])).unwrap_err().contains("unknown command"));
        assert!(parse(&args(&["check", "x.rs", "--bogus"])).unwrap_err().contains("unknown flag"));
        assert!(parse(&args(&["check", "a.rs", "b.rs"])).unwrap_err().contains("unexpected"));
        assert!(parse(&args(&["check", "a.rs", "--name", ""])).unwrap_err().contains("empty"));
    }
}
