//! `test`: the crate's public functions and methods called in Rust and in
//! the generated TypeScript on the same drawn arguments, and every result
//! compared as canonical text (design/01 §8). What the differential tests
//! do for the examples, for a user's crate.
//!
//! The Rust side is the crate's own files, copied with a harness appended
//! (`harness`) and compiled by rustc as a binary with overflow checks and
//! debug assertions, as a debug build runs. It draws each function's
//! arguments, calls it, and prints the arguments as TS and the result. The
//! TS side is the package `build` would write, and a node driver that
//! makes the same calls and prints their results with the same printers.

mod derive;
mod harness;
pub mod printers;

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use purecrate_ir::Crate;
use purecrate_pack::{disk_path, SOURCE_CONDITION};

use crate::args::{Input, TestOptions};
use crate::Loaded;
use harness::{Call, Plan, Words};

/// One call made on both sides.
struct Case {
    key: String,
    /// The TS arguments.
    args: String,
    rust: String,
}

pub fn run(input: &Input, loaded: &Loaded, opts: &TestOptions) -> Result<(), String> {
    if let Some(file) = loaded.files.iter().find(|f| f.text.contains("uuid::")) {
        return Err(format!(
            "{}: test cannot run a crate that uses `uuid`: rustc gets a stand-in for it, which parses nothing",
            file.path.display()
        ));
    }
    let mut plan = harness::plan(&loaded.krate, &loaded.spans);
    let mut report: Vec<String> = plan.skipped.iter().map(|s| format!("  skipped   {}: {}", s.name, s.why)).collect();
    for name in &opts.skip {
        let before = plan.calls.len();
        plan.calls.retain(|c| !skips(name, &c.key));
        if plan.calls.len() == before {
            return Err(format!("--skip {name}: no public function or method of that name is tested"));
        }
        report.push(format!("  skipped   {name}: --skip"));
    }
    let calls = &plan.calls;
    if calls.is_empty() {
        report.insert(0, format!("{}: no public function to test", input.src.display()));
        return Err(report.join("\n"));
    }
    let (dir, keep) = match &opts.keep {
        Some(dir) => (dir.clone(), true),
        None => (env::temp_dir().join(format!("purecrate-test-{}", std::process::id())), false),
    };
    if dir.exists() {
        fs::remove_dir_all(&dir).map_err(|e| format!("clear {}: {e}", dir.display()))?;
    }
    let result = run_in(input, loaded, opts, &plan, &dir);
    if !keep {
        fs::remove_dir_all(&dir).ok();
    }
    let cases = result?;

    let mut failed = 0;
    let mut lines = Vec::new();
    for call in calls {
        let mine: Vec<&(Case, String)> = cases.iter().filter(|(c, _)| c.key == call.key).collect();
        let wrong: Vec<&&(Case, String)> = mine.iter().filter(|(c, ts)| c.rust != *ts).collect();
        let name = call.key.trim_start_matches("pkg.");
        if mine.is_empty() {
            lines.push(format!(
                "  no cases  {name}: no arguments could be drawn (a closed type none of whose constructors succeeded on drawn input)"
            ));
        } else if wrong.is_empty() {
            lines.push(format!("  ok        {name} ({} cases)", mine.len()));
        } else {
            failed += 1;
            lines.push(format!("  DIFFERS   {name}: {} of {} cases", wrong.len(), mine.len()));
            let mut shown: Vec<&str> = Vec::new();
            for (case, ts) in wrong.iter() {
                // Each distinct call once, up to five.
                if shown.contains(&case.args.as_str()) {
                    continue;
                }
                if shown.len() == 5 {
                    break;
                }
                shown.push(&case.args);
                lines.push(format!("    {name}({})", clip(&case.args)));
                lines.push(format!("      rust: {}", clip(&case.rust)));
                lines.push(format!("      ts:   {}", clip(ts)));
            }
        }
    }
    lines.extend(report);
    let head =
        format!("{}: {} function(s), {} case(s), seed {}", input.src.display(), calls.len(), cases.len(), opts.seed);
    if failed == 0 {
        println!("{head}\n{}", lines.join("\n"));
        Ok(())
    } else {
        Err(format!(
            "{head}\n{}\n{failed} function(s) differ between Rust and the generated TypeScript",
            lines.join("\n")
        ))
    }
}

fn clip(s: &str) -> String {
    const MAX: usize = 300;
    match s.char_indices().nth(MAX) {
        Some((i, _)) => format!("{}…", &s[..i]),
        None => s.to_string(),
    }
}

/// Builds and runs both sides in `dir`; each case with the TS result.
fn run_in(
    input: &Input,
    loaded: &Loaded,
    opts: &TestOptions,
    plan: &Plan<'_>,
    dir: &Path,
) -> Result<Vec<(Case, String)>, String> {
    let rust_dir = dir.join("rust");
    let ts_dir = dir.join("ts");
    let binary = harness_binary(input, loaded, plan, &rust_dir)?;
    let mut rust = Command::new(&binary);
    rust.arg(opts.seed.to_string()).arg(opts.cases.to_string());
    let output = bounded(rust, opts.timeout).map_err(|e| match e {
        Run::Start(e) => format!("run {}: {e}", binary.display()),
        Run::Timeout(stderr) => {
            let running = stderr.lines().rev().find_map(|l| l.strip_prefix("start\t")).unwrap_or("?");
            let name = running.trim_start_matches("pkg.");
            format!(
                "the Rust side did not finish within {} s, in `{name}`: an argument drawn near its type's edge may make it run that long; leave it out with --skip {name}, or raise --timeout",
                opts.timeout
            )
        }
    })?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let said: Vec<&str> = stderr.lines().filter(|l| !l.starts_with("start\t")).collect();
        return Err(format!("the Rust harness failed:\n{}", said.join("\n").trim_end()));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let cases: Vec<Case> = stdout
        .lines()
        .filter_map(|l| {
            let mut parts = l.splitn(4, '\t');
            match (parts.next(), parts.next(), parts.next(), parts.next()) {
                (Some("case"), Some(key), Some(args), Some(rust)) => {
                    Some(Case { key: key.into(), args: args.into(), rust: rust.into() })
                }
                _ => None,
            }
        })
        .collect();

    if cases.is_empty() {
        return Ok(Vec::new());
    }
    for file in &loaded.package.files {
        let path = ts_dir.join(disk_path(&file.stem));
        fs::create_dir_all(path.parent().unwrap_or(&ts_dir)).map_err(|e| format!("mkdir {}: {e}", ts_dir.display()))?;
        fs::write(&path, &file.source).map_err(|e| format!("write {}: {e}", path.display()))?;
    }
    fs::write(ts_dir.join("driver.ts"), driver(&loaded.krate, &plan.calls, &cases))
        .map_err(|e| format!("write {}: {e}", ts_dir.display()))?;
    let mut node = Command::new("node");
    node.arg(format!("--conditions={SOURCE_CONDITION}")).arg("driver.ts").current_dir(&ts_dir);
    let output = bounded(node, opts.timeout).map_err(|e| match e {
        Run::Start(e) => format!("run node: {e}; test runs the generated TypeScript with node (22.18 or later)"),
        Run::Timeout(_) => format!(
            "the TypeScript side did not finish within {} s, where the Rust side did; raise --timeout if it is only slower",
            opts.timeout
        ),
    })?;
    if !output.status.success() {
        return Err(format!("node failed on the driver:\n{}", String::from_utf8_lossy(&output.stderr).trim_end()));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let ts: Vec<String> = stdout.lines().map(str::to_string).collect();
    if ts.len() != cases.len() {
        return Err(format!("the driver printed {} result(s) for {} case(s)", ts.len(), cases.len()));
    }
    Ok(cases.into_iter().zip(ts).collect())
}

/// Whether `--skip name` names the call `key` (`pkg.makeMove`,
/// `pkg.Ulid.parse`, `pkg.total<Item>`): by the name the report gives or
/// the Rust one (`make_move`, `Ulid::parse`), and a generic function at
/// every type it is called at.
fn skips(name: &str, key: &str) -> bool {
    let shown = key.trim_start_matches("pkg.");
    let shown = shown.split('<').next().unwrap_or(shown);
    let camel = |seg: &str| {
        let mut out = String::new();
        for (i, part) in seg.split('_').enumerate() {
            let mut cs = part.chars();
            match (i, cs.next()) {
                (0, Some(c)) => out.push(c),
                (_, Some(c)) => out.extend(c.to_uppercase()),
                (_, None) => {}
            }
            out.extend(cs);
        }
        out
    };
    let name: Vec<String> = name.replace("::", ".").split('.').map(camel).collect();
    shown == name.join(".")
}

enum Run {
    Start(std::io::Error),
    /// What it wrote to stderr before it was stopped.
    Timeout(String),
}

/// `command`'s output, or `Run::Timeout` once it has run `secs` seconds,
/// when it is killed.
fn bounded(mut command: Command, secs: u64) -> Result<std::process::Output, Run> {
    use std::io::Read;
    use std::process::Stdio;
    let mut child = command.stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().map_err(Run::Start)?;
    let drain = |mut r: Box<dyn Read + Send>| {
        std::thread::spawn(move || {
            let mut buf = Vec::new();
            r.read_to_end(&mut buf).ok();
            buf
        })
    };
    let out = drain(Box::new(child.stdout.take().expect("piped")));
    let err = drain(Box::new(child.stderr.take().expect("piped")));
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(secs);
    let status = loop {
        if let Some(status) = child.try_wait().map_err(Run::Start)? {
            break status;
        }
        if std::time::Instant::now() >= deadline {
            child.kill().ok();
            child.wait().ok();
            let stderr = err.join().unwrap_or_default();
            return Err(Run::Timeout(String::from_utf8_lossy(&stderr).into_owned()));
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    };
    let (stdout, stderr) = (out.join().unwrap_or_default(), err.join().unwrap_or_default());
    Ok(std::process::Output { status, stdout, stderr })
}

/// The crate's files with the harness appended, compiled.
fn harness_binary(input: &Input, loaded: &Loaded, plan: &Plan<'_>, dir: &Path) -> Result<PathBuf, String> {
    let root = loaded.files[0].path.parent().unwrap_or(Path::new("."));
    let mut words = Words::default();
    for file in &loaded.files {
        words.add(&file.text);
    }
    let modules: Vec<Vec<String>> = loaded.files.iter().map(|f| f.module.clone()).collect();
    let extra = harness::appended(plan, &modules, &words);
    for (file, extra) in loaded.files.iter().zip(&extra) {
        let rel = file.path.strip_prefix(root).unwrap_or(&file.path);
        let path = dir.join(rel);
        fs::create_dir_all(path.parent().unwrap_or(dir)).map_err(|e| format!("mkdir {}: {e}", dir.display()))?;
        fs::write(&path, format!("{}\n{extra}", file.text)).map_err(|e| format!("write {}: {e}", path.display()))?;
    }
    let main = dir.join(loaded.files[0].path.file_name().unwrap_or_default());
    let binary = dir.join("harness");
    crate::rustc::compile_harness(&main, &input.edition, &input.name.replace('-', "_"), &binary).map_err(|e| {
        format!("{e}\nthe crate compiles as a library, but not with the test harness appended (in {})", dir.display())
    })?;
    Ok(binary)
}

/// The node script: each case's call, printed as the Rust side printed it,
/// one line per case and escaped as the harness escapes.
fn driver(krate: &Crate, calls: &[Call], cases: &[Case]) -> String {
    let mut defs = String::from(
        "const esc = (s) => s.replaceAll(\"\\\\\", \"\\\\\\\\\").replaceAll(\"\\n\", \"\\\\n\").replaceAll(\"\\r\", \"\\\\r\").replaceAll(\"\\t\", \"\\\\t\");\n\
         const __ok = (r) => {\n  if (r.kind !== \"Ok\") throw new Error(\"purecrate-ts test: a constructor gave Err in TS and Ok in Rust\");\n  return r.value;\n};\n\
         const __some = (o) => {\n  if (o === null) throw new Error(\"purecrate-ts test: a constructor gave None in TS and Some in Rust\");\n  return o;\n};\n",
    );
    for (i, call) in calls.iter().enumerate() {
        let print = printers::ts_printer(krate, &call.ret).expect("a printable result (plan)");
        defs.push_str(&format!("const print{i} = {print};\n"));
    }
    let runs = cases.iter().map(|case| {
        let i = calls.iter().position(|c| c.key == case.key).expect("a case of a planned call");
        let call = &calls[i];
        let args = std::iter::once(case.args.as_str())
            .filter(|a| !a.is_empty())
            .chain(call.dicts.iter().map(String::as_str))
            .collect::<Vec<_>>()
            .join(", ");
        format!("run(() => {}({args}), print{i})", call.ts)
    });
    printers::driver(krate, &defs, runs, "out.map(esc).join(\"\\n\")")
}

#[cfg(test)]
mod tests {
    use super::skips;

    #[test]
    fn skip_names_a_call_by_its_ts_or_rust_name() {
        assert!(skips("perft", "pkg.perft"));
        assert!(skips("make_move", "pkg.makeMove"));
        assert!(skips("makeMove", "pkg.makeMove"));
        assert!(skips("Position::piece_at", "pkg.Position.pieceAt"));
        assert!(skips("Ulid.parse", "pkg.Ulid.parse"));
        assert!(skips("total", "pkg.total<Item>"));
        assert!(!skips("move", "pkg.makeMove"));
        assert!(!skips("parse", "pkg.Ulid.parse"));
    }
}
