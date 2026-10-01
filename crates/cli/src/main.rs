mod args;
mod drift;
mod files;
mod rustc;
mod survey;

use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use purecrate_check::{accept, prune_unreachable};
use purecrate_emit_ts::Package;
use purecrate_emit_ts::{has_wire, WireSchema};
use purecrate_pack::{assemble_with_access, disk_path, Access};
use purecrate_syntax::{parse_files_spanned, LineCol, Source};

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
    let result = execute(command);
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::from(1)
        }
    }
}

fn execute(command: Command) -> Result<(), String> {
    match command {
        Command::Build { input, out, schema, publishable } => {
            let schema = parse_schema(schema)?;
            load(&input, "nothing written", schema, access(publishable)).and_then(|pkg| write_replacing(&out, &pkg.files))
        }
        Command::Check { input, out: None, schema, publishable } => {
            let schema = parse_schema(schema)?;
            load(&input, "", schema, access(publishable)).map(|_| ())
        }
        Command::Check {
            input,
            out: Some(out),
            schema,
            publishable,
        } => {
            let schema = parse_schema(schema)?;
            load(&input, "", schema, access(publishable)).and_then(|pkg| check_drift(&input, &out, &pkg))
        }
        Command::Survey { inputs, json, all_causes } => run_survey(&inputs, json, all_causes),
        Command::Version => {
            println!("purecrate-ts {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        Command::Help => {
            println!("{}", args::USAGE);
            Ok(())
        }
    }
}

/// A crate that cannot be surveyed is reported and skipped; the others run.
fn run_survey(inputs: &[Input], json: bool, all_causes: bool) -> Result<(), String> {
    let mut failed = Vec::new();
    for input in inputs {
        let (files, missing) = survey::collect_files(&input.src);
        let report = if files.is_empty() {
            Err(format!("read {}: not found", input.src.display()))
        } else {
            survey::survey(&input.name, files, missing, all_causes)
        };
        match report {
            Ok(r) if json => println!("{}", r.json()),
            Ok(r) => print!("{}", r.human()),
            Err(e) => failed.push(e),
        }
    }
    match failed.as_slice() {
        [] => Ok(()),
        errors => Err(format!("{}\n{} crate(s) not surveyed", errors.join("\n"), errors.len())),
    }
}

fn parse_schema(schema: Option<String>) -> Result<Option<WireSchema>, String> {
    match schema {
        None => Ok(None),
        Some(name) => WireSchema::parse(&name)
            .map(Some)
            .ok_or_else(|| format!("--schema {name} is not zod, valibot, or arktype")),
    }
}

fn access(publishable: bool) -> Access {
    if publishable {
        Access::Publishable
    } else {
        Access::Private
    }
}

/// Parse, check, compile with rustc, prune, emit. `consequence` ends the
/// error summary line.
fn load(input: &Input, consequence: &str, schema: Option<WireSchema>, access: Access) -> Result<Package, String> {
    let src = &input.src;
    let files = files::crate_files(src)?;
    let sources: Vec<Source<'_>> = files.iter().map(|f| Source { text: &f.text, public: f.public }).collect();
    let (krate, spans) = parse_files_spanned(&input.name, &sources).map_err(|(i, e)| match e.at {
        Some(_) => format!("{}:{e}", files[i].path.display()),
        None => format!("{}: {e}", files[i].path.display()),
    })?;
    let diagnostics = match accept(&krate) {
        Ok(typed) => {
            return match rustc::compile(src, &input.edition) {
                Ok(()) => {
                    let pruned = prune_unreachable(&typed);
                    match schema {
                        Some(lib) if !has_wire(&pruned) => Err(format!(
                            "{}: --schema {}: no public struct or enum derives `Serialize` or `Deserialize`, so there is no wire form to write\n{}",
                            src.display(),
                            lib.runtime_dep(),
                            summary(1, consequence)
                        )),
                        _ => Ok(assemble_with_access(&pruned, schema, &input.version, access)),
                    }
                }
                Err(rustc::Failure::Other(e)) => Err(e),
                Err(rustc::Failure::Rejected(errors)) => {
                    let mut report: Vec<String> = errors.iter().map(|e| e.line()).collect();
                    report.push(summary(errors.len(), consequence));
                    Err(report.join("\n"))
                }
            }
        }
        Err(d) => d,
    };
    let at = |i: usize| {
        let (file, LineCol { line, col }) = spans[i];
        format!("{}:{line}:{col}", files[file].path.display())
    };
    let mut report: Vec<String> = Vec::new();
    for d in &diagnostics {
        // The statement or arm inside the item when known, else the item's name.
        let place = match d.at {
            Some(p) => format!("{}:{}:{}", files[spans[d.item].0].path.display(), p.line, p.col),
            None => at(d.item),
        };
        report.push(format!("{place}: [{}] {}", d.reason, d.message));
        for &other in &d.also {
            report.push(format!("  note: see {}", at(other)));
        }
    }
    report.push(summary(diagnostics.len(), consequence));
    Err(report.join("\n"))
}

fn summary(errors: usize, consequence: &str) -> String {
    match consequence {
        "" => format!("{errors} error(s)"),
        c => format!("{errors} error(s); {c}"),
    }
}

fn check_drift(input: &Input, out: &Path, pkg: &Package) -> Result<(), String> {
    let expected: BTreeMap<String, String> = pkg
        .files
        .iter()
        .map(|f| (disk_path(&f.stem), f.source.clone()))
        .collect();
    let actual = if out.is_dir() {
        read_tree(out)?
    } else {
        BTreeMap::new()
    };
    let drifts = drift::compare(&expected, &actual);
    if drifts.is_empty() {
        return Ok(());
    }
    let mut report = vec![format!("{}: {} file(s) out of date", out.display(), drifts.len())];
    report.extend(drifts.iter().map(|d| format!("  {:<8} {}", d.kind.to_string(), d.path)));
    report.push(format!(
        "run: purecrate-ts build {} --name {} --out {}",
        input.src.display(),
        input.name,
        out.display()
    ));
    Err(report.join("\n"))
}

fn read_tree(root: &Path) -> Result<BTreeMap<String, Vec<u8>>, String> {
    let mut out = BTreeMap::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = fs::read_dir(&dir).map_err(|e| format!("read {}: {e}", dir.display()))?;
        for entry in entries {
            let path = entry.map_err(|e| format!("read {}: {e}", dir.display()))?.path();
            if path.is_dir() {
                // Installed dependencies, and `npm run build` output.
                let name = path.file_name().and_then(|n| n.to_str());
                if name == Some("node_modules") || (dir == root && name == Some("dist")) {
                    continue;
                }
                stack.push(path);
                continue;
            }
            let rel = path
                .strip_prefix(root)
                .map_err(|e| format!("{}: {e}", path.display()))?
                .components()
                .map(|c| c.as_os_str().to_string_lossy().into_owned())
                .collect::<Vec<_>>()
                .join("/");
            let bytes = fs::read(&path).map_err(|e| format!("read {}: {e}", path.display()))?;
            out.insert(rel, bytes);
        }
    }
    Ok(out)
}

/// Write into a sibling directory, then swap it in, so a failed write never
/// leaves `out` half-replaced. Stale files of an earlier build go with it
/// except `node_modules/`, which `check --out` already skips and which a
/// user who ran `npm install` in the package would otherwise lose. `dist/`
/// is not kept: it is the previous `npm run build` and would be stale.
/// A directory that holds anything else is refused, not emptied.
fn write_replacing(out: &Path, files: &[purecrate_emit_ts::File]) -> Result<(), String> {
    if !replaceable(out)? {
        return Err(format!(
            "{} is not empty and holds no earlier purecrate-ts output (src/index.ts with its header); \
             nothing written. Choose an empty or new directory for --out",
            out.display()
        ));
    }
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
        let modules = old.join("node_modules");
        if modules.exists() {
            if let Err(e) = fs::rename(&modules, tmp.join("node_modules")) {
                fs::rename(&old, out).ok();
                fs::remove_dir_all(&tmp).ok();
                return Err(format!("keep {}/node_modules: {e}", out.display()));
            }
        }
        if let Err(e) = fs::rename(&tmp, out) {
            let restored = tmp.join("node_modules");
            if restored.exists() {
                fs::rename(&restored, old.join("node_modules")).ok();
            }
            fs::rename(&old, out).ok();
            fs::remove_dir_all(&tmp).ok();
            return Err(format!("replace {}: {e}", out.display()));
        }
        fs::remove_dir_all(&old).map_err(|e| format!("remove {}: {e}", old.display()))?;
    } else {
        fs::rename(&tmp, out).map_err(|e| format!("create {}: {e}", out.display()))?;
    }
    Ok(())
}

/// `out` is absent, an empty directory, or an earlier build's output.
fn replaceable(out: &Path) -> Result<bool, String> {
    if !out.exists() {
        return Ok(true);
    }
    if !out.is_dir() {
        return Ok(false);
    }
    let mut entries = fs::read_dir(out).map_err(|e| format!("read {}: {e}", out.display()))?;
    if entries.next().is_none() {
        return Ok(true);
    }
    let index = fs::read_to_string(out.join("src/index.ts")).unwrap_or_default();
    Ok(index.starts_with(purecrate_emit_ts::HEADER))
}

fn sibling(out: &Path, tag: &str) -> PathBuf {
    let name = out
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "out".to_string());
    out.with_file_name(format!(".{name}.purecrate-{tag}-{}", std::process::id()))
}
