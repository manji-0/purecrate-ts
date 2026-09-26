mod args;

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use purecrate_check::{check, prune_unreachable};
use purecrate_emit_ts::Package;
use purecrate_pack::{assemble, disk_path};
use purecrate_syntax::{parse_source_spanned, LineCol};

use args::{Command, Input};

fn main() -> ExitCode {
    let argv: Vec<String> = env::args().skip(1).collect();
    let command = match args::parse(&argv) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{e}\n\n{}", args::USAGE);
            return ExitCode::from(2);
        }
    };
    let result = match command {
        Command::Build { input, out } => {
            load(&input, "nothing written").and_then(|pkg| write_replacing(&out, &pkg.files))
        }
        Command::Check { input, .. } => load(&input, "").map(|_| ()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::from(1)
        }
    }
}

/// Parse, check, prune, emit. `consequence` ends the error summary line.
fn load(input: &Input, consequence: &str) -> Result<Package, String> {
    let src = &input.src;
    let text = fs::read_to_string(src).map_err(|e| format!("read {}: {e}", src.display()))?;
    let (krate, spans) = parse_source_spanned(&input.name, &text).map_err(|e| match e.at {
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
        report.push(match consequence {
            "" => format!("{} error(s)", diagnostics.len()),
            c => format!("{} error(s); {c}", diagnostics.len()),
        });
        return Err(report.join("\n"));
    }
    Ok(assemble(&prune_unreachable(&krate)))
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
