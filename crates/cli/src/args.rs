//! Command-line parsing. Resolving the crate name and edition reads
//! `Cargo.toml` if present.

use std::fs;
use std::path::{Path, PathBuf};

pub const USAGE: &str = "\
usage:
  purecrate-ts build <crate-path> --out <dir> [--name <crate>] [--edition <year>] [--schema <lib>]
  purecrate-ts check <crate-path> [--out <dir>] [--name <crate>] [--edition <year>] [--schema <lib>]
  purecrate-ts survey <crate-path>... [--json]

--schema is zod, valibot, or arktype. It adds src/purecrate-wire.ts,
schemas for the public structs and enums. The numeric fields come from
the matching purecrate-* adapter.

<crate-path> is a crate directory (reads src/lib.rs, else src/main.rs) or a
single .rs file.
check and build run the subset checks, then compile the crate with rustc
(RUSTC overrides the binary); a crate rustc rejects is rejected. rustc gets
the edition from Cargo.toml ([package] edition, or [workspace.package] when
inherited; 2015 when a manifest names none, as cargo does), 2021 for a file
with no manifest, or --edition. check
with --out also fails when <dir> differs from what build would write.
survey follows `mod` declarations and reports, for each public function and
type, whether it is accepted together with what it refers to; --json prints
one JSON object per crate.";

#[derive(Debug, PartialEq, Eq)]
pub struct Input {
    pub src: PathBuf,
    pub name: String,
    /// The Rust edition rustc compiles the input with.
    pub edition: String,
    /// The generated package's version: the crate's, else `0.1.0`.
    pub version: String,
}

/// For a source file with no `Cargo.toml` to read.
pub const DEFAULT_EDITION: &str = "2021";

const EDITIONS: &[&str] = &["2015", "2018", "2021", "2024"];

#[derive(Debug, PartialEq, Eq)]
pub enum Command {
    Build {
        input: Input,
        out: PathBuf,
        schema: Option<String>,
    },
    Check {
        input: Input,
        out: Option<PathBuf>,
        schema: Option<String>,
    },
    Survey { inputs: Vec<Input>, json: bool },
}

pub fn parse(args: &[String]) -> Result<Command, String> {
    let (verb, rest) = args.split_first().ok_or("missing command")?;
    if verb == "survey" {
        return parse_survey(rest);
    }
    let mut path: Option<PathBuf> = None;
    let mut name: Option<String> = None;
    let mut edition: Option<String> = None;
    let mut out: Option<PathBuf> = None;
    let mut schema: Option<String> = None;
    let mut it = rest.iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--name" => name = Some(it.next().ok_or("--name needs a value")?.clone()),
            "--edition" => {
                let value = it.next().ok_or("--edition needs a year")?;
                if !EDITIONS.contains(&value.as_str()) {
                    return Err(format!("--edition {value} is not one of {}", EDITIONS.join(", ")));
                }
                edition = Some(value.clone());
            }
            "--out" => out = Some(PathBuf::from(it.next().ok_or("--out needs a value")?)),
            "--schema" => {
                let value = it.next().ok_or("--schema needs zod, valibot, or arktype")?;
                if !matches!(value.as_str(), "zod" | "valibot" | "arktype") {
                    return Err(format!("--schema {value} is not zod, valibot, or arktype"));
                }
                schema = Some(value.clone());
            }
            flag if flag.starts_with('-') => return Err(format!("unknown flag {flag}")),
            other if path.is_some() => return Err(format!("unexpected argument {other}")),
            other => path = Some(PathBuf::from(other)),
        }
    }
    let path = path.ok_or("missing <crate-path>")?;
    let mut input = resolve_input(&path, name)?;
    if let Some(e) = edition {
        input.edition = e;
    }
    match verb.as_str() {
        "build" => Ok(Command::Build {
            input,
            out: out.ok_or("build needs --out <dir>")?,
            schema,
        }),
        "check" => Ok(Command::Check { input, out, schema }),
        other => Err(format!("unknown command {other}")),
    }
}

fn parse_survey(rest: &[String]) -> Result<Command, String> {
    let mut json = false;
    let mut inputs = Vec::new();
    for arg in rest {
        match arg.as_str() {
            "--json" => json = true,
            flag if flag.starts_with('-') => return Err(format!("unknown flag {flag}")),
            other => inputs.push(resolve_input(Path::new(other), None)?),
        }
    }
    if inputs.is_empty() {
        return Err("missing <crate-path>".into());
    }
    Ok(Command::Survey { inputs, json })
}

fn resolve_input(path: &Path, name: Option<String>) -> Result<Input, String> {
    let (src, crate_dir) = if path.is_dir() {
        let lib = path.join("src/lib.rs");
        let main = path.join("src/main.rs");
        let src = if !lib.exists() && main.exists() { main } else { lib };
        (src, Some(path.to_path_buf()))
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
    let (edition, version) = match crate_dir.filter(|d| d.join("Cargo.toml").is_file()) {
        None => (DEFAULT_EDITION.to_string(), purecrate_pack::DEFAULT_VERSION.to_string()),
        Some(dir) => {
            // A manifest without an edition is 2015, as cargo reads it.
            let edition = package_field(&dir, "edition")?.unwrap_or_else(|| "2015".into());
            if !EDITIONS.contains(&edition.as_str()) {
                return Err(format!("{}: edition {edition} is not one of {}", dir.display(), EDITIONS.join(", ")));
            }
            let version = package_field(&dir, "version")?.unwrap_or_else(|| purecrate_pack::DEFAULT_VERSION.into());
            (edition, version)
        }
    };
    Ok(Input { src, name, edition, version })
}

/// `[package] key` in `dir/Cargo.toml`. `key.workspace = true` reads
/// `[workspace.package]` from the nearest enclosing manifest that has it.
fn package_field(dir: &Path, key: &str) -> Result<Option<String>, String> {
    let manifest = fs::read_to_string(dir.join("Cargo.toml")).map_err(|e| format!("read {}/Cargo.toml: {e}", dir.display()))?;
    match manifest_value(&manifest, "package", key) {
        None => Ok(None),
        Some(Value::Text(v)) => Ok(Some(v)),
        Some(Value::Inherited) => dir
            .ancestors()
            .skip(1)
            .find_map(|d| {
                let text = fs::read_to_string(d.join("Cargo.toml")).ok()?;
                match manifest_value(&text, "workspace.package", key)? {
                    Value::Text(v) => Some(v),
                    Value::Inherited => None,
                }
            })
            .map(Some)
            .ok_or_else(|| format!("{}: {key}.workspace = true, but no workspace sets {key}", dir.display())),
    }
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
    match manifest_value(manifest, "package", "name")? {
        Value::Text(n) => Some(n),
        Value::Inherited => None,
    }
}

#[derive(Debug, PartialEq, Eq)]
enum Value {
    Text(String),
    /// `key.workspace = true` or `key = { workspace = true }`.
    Inherited,
}

/// A string `key` in the `[section]` table. Enough of TOML for the flat
/// keys cargo manifests use here; not a TOML parser.
fn manifest_value(manifest: &str, section: &str, key: &str) -> Option<Value> {
    let header = format!("[{section}]");
    let mut inside = false;
    for line in manifest.lines().map(str::trim) {
        if line.starts_with('[') {
            inside = line == header;
            continue;
        }
        if !inside {
            continue;
        }
        let Some((k, value)) = line.split_once('=') else { continue };
        let (k, value) = (k.trim(), value.trim());
        if k == format!("{key}.workspace") && value == "true" {
            return Some(Value::Inherited);
        }
        if k == key {
            if value.starts_with('{') && value.contains("workspace") {
                return Some(Value::Inherited);
            }
            return Some(Value::Text(value.trim_matches('"').to_string()));
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
                    name: "counter".into(),
                    edition: "2021".into(),
                    version: "0.1.0".into(),
                },
                out: None,
                schema: None,
            }
        );
    }

    #[test]
    fn lib_rs_path_uses_the_crate_directory_name() {
        let src = examples().join("counter/src/lib.rs");
        let Command::Build { input, out, schema } =
            parse(&args(&["build", src.to_str().unwrap(), "--out", "o"])).unwrap()
        else {
            panic!("expected build");
        };
        assert_eq!(schema, None);
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
    fn edition_comes_from_the_manifest_or_the_flag() {
        let dir = std::env::temp_dir().join(format!("purecrate-args-edition-{}", std::process::id()));
        let krate = dir.join("members/k");
        fs::create_dir_all(krate.join("src")).unwrap();
        fs::write(krate.join("src/lib.rs"), "").unwrap();
        let edition = |manifest: &str, extra: &[&str]| {
            fs::write(krate.join("Cargo.toml"), manifest).unwrap();
            let mut a = vec!["check", krate.to_str().unwrap()];
            a.extend_from_slice(extra);
            parse(&args(&a)).map(|c| match c {
                Command::Check { input, .. } => input.edition,
                _ => unreachable!(),
            })
        };
        assert_eq!(edition("[package]\nname = \"k\"\nedition = \"2024\"\n", &[]).unwrap(), "2024");
        assert_eq!(edition("[package]\nname = \"k\"\n", &[]).unwrap(), "2015");
        assert_eq!(edition("[package]\nname = \"k\"\n", &["--edition", "2018"]).unwrap(), "2018");
        fs::write(dir.join("Cargo.toml"), "[workspace]\nmembers = [\"members/k\"]\n[workspace.package]\nedition = \"2024\"\n").unwrap();
        assert_eq!(edition("[package]\nname = \"k\"\nedition.workspace = true\n", &[]).unwrap(), "2024");
        assert_eq!(edition("[package]\nname = \"k\"\nedition = { workspace = true }\n", &[]).unwrap(), "2024");
        assert!(edition("[package]\nname = \"k\"\nedition = \"2030\"\n", &[]).unwrap_err().contains("2030"));
        assert!(edition("[package]\nname = \"k\"\n", &["--edition", "3000"]).unwrap_err().contains("3000"));
        fs::write(krate.join("Cargo.toml"), "[package]\nname = \"k\"\nversion.workspace = true\n").unwrap();
        fs::write(dir.join("Cargo.toml"), "[workspace]\n[workspace.package]\nversion = \"2.3.4\"\n").unwrap();
        let Command::Check { input, .. } = parse(&args(&["check", krate.to_str().unwrap()])).unwrap() else {
            unreachable!()
        };
        assert_eq!((input.version.as_str(), input.edition.as_str()), ("2.3.4", "2015"));
        fs::remove_dir_all(&dir).ok();
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
