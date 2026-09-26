use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use purecrate_check::{check, prune_unreachable};
use purecrate_pack::{assemble, disk_path};
use purecrate_syntax::{parse_source_spanned, LineCol};

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
    let (krate, spans) = parse_source_spanned(name, &text).map_err(|e| match e.at {
        Some(_) => format!("{}:{e}", src.display()),
        None => format!("{}: {e}", src.display()),
    })?;
    let diagnostics = check(&krate);
    if !diagnostics.is_empty() {
        let at = |i: usize| {
            let LineCol { line, col } = spans[i];
            format!("{}:{line}:{col}", src.display())
        };
        let mut report: Vec<String> = Vec::new();
        for d in &diagnostics {
            report.push(format!("{}: {}", at(d.item), d.message));
            for &other in &d.also {
                report.push(format!("  note: see {}", at(other)));
            }
        }
        report.push(format!("{} error(s); nothing written", diagnostics.len()));
        return Err(report.join("\n"));
    }
    let pkg = assemble(&prune_unreachable(&krate));
    write_replacing(out, &pkg.files)
}

/// Write into a sibling directory, then swap it in, so a failed write never
/// leaves `out` half-replaced.
fn write_replacing(out: &Path, files: &[purecrate_emit_ts::File]) -> Result<(), String> {
    let tmp = sibling(out, "tmp");
    if tmp.exists() {
        fs::remove_dir_all(&tmp).map_err(|e| format!("clear {}: {e}", tmp.display()))?;
    }
    let written = files.iter().try_for_each(|file| {
        let path = tmp.join(disk_path(&file.stem));
        let parent = path.parent().unwrap_or(&tmp);
        fs::create_dir_all(parent).map_err(|e| format!("mkdir {}: {e}", parent.display()))?;
        fs::write(&path, &file.source).map_err(|e| format!("write {}: {e}", path.display()))
    });
    if let Err(e) = written {
        fs::remove_dir_all(&tmp).ok();
        return Err(e);
    }
    if out.exists() {
        let old = sibling(out, "old");
        fs::rename(out, &old).map_err(|e| format!("move aside {}: {e}", out.display()))?;
        if let Err(e) = fs::rename(&tmp, out) {
            fs::rename(&old, out).ok();
            return Err(format!("replace {}: {e}", out.display()));
        }
        fs::remove_dir_all(&old).map_err(|e| format!("remove {}: {e}", old.display()))?;
    } else {
        fs::rename(&tmp, out).map_err(|e| format!("create {}: {e}", out.display()))?;
    }
    Ok(())
}

fn sibling(out: &Path, tag: &str) -> PathBuf {
    let name = out
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "out".to_string());
    out.with_file_name(format!(".{name}.purecrate-{tag}-{}", std::process::id()))
}
