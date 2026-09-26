use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use purecrate_pack::{assemble, disk_path};
use purecrate_syntax::parse_source;

fn main() -> ExitCode {
    let mut args = env::args().skip(1).collect::<Vec<_>>();
    if args.first().map(String::as_str) != Some("build") {
        eprintln!("usage: purecrate-ts build <src.rs> --name <crate> --out <dir>");
        return ExitCode::from(2);
    }
    args.remove(0);
    let mut src: Option<PathBuf> = None;
    let mut name = "crate".to_string();
    let mut out: Option<PathBuf> = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--name" => {
                name = args.get(i + 1).cloned().unwrap_or_default();
                i += 2;
            }
            "--out" => {
                out = args.get(i + 1).map(PathBuf::from);
                i += 2;
            }
            flag if flag.starts_with('-') => {
                eprintln!("unknown flag {flag}");
                return ExitCode::from(2);
            }
            other => {
                src = Some(PathBuf::from(other));
                i += 1;
            }
        }
    }
    let src = match src {
        Some(p) => p,
        None => {
            eprintln!("missing source file");
            return ExitCode::from(2);
        }
    };
    let out = match out {
        Some(p) => p,
        None => {
            eprintln!("missing --out");
            return ExitCode::from(2);
        }
    };
    match build(&src, &name, &out) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::from(1)
        }
    }
}

fn build(src: &Path, name: &str, out: &Path) -> Result<(), String> {
    let text = fs::read_to_string(src).map_err(|e| format!("read {}: {e}", src.display()))?;
    let krate = parse_source(name, &text).map_err(|e| match e.at {
        Some(_) => format!("{}:{e}", src.display()),
        None => format!("{}: {e}", src.display()),
    })?;
    let pkg = assemble(&krate);
    if out.exists() {
        fs::remove_dir_all(out).map_err(|e| format!("clear {}: {e}", out.display()))?;
    }
    fs::create_dir_all(out.join("src")).map_err(|e| format!("mkdir: {e}"))?;
    for file in pkg.files {
        let path = out.join(disk_path(&file.stem));
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).ok();
        }
        fs::write(&path, file.source).map_err(|e| format!("write {}: {e}", path.display()))?;
    }
    Ok(())
}
