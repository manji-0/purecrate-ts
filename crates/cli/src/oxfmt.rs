//! The generated files' layout is oxfmt's: `build` and `check --out` run it
//! over them as a program (oxc's formatter is not published as a crate).
//! The layout is the CLI's, not the project's: oxfmt is given a width of
//! `purecrate_emit_ts::WIDTH`, not a `.oxfmtrc.json` beside the output. The
//! copied runtime and adapters are hand-written and left as they are.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use purecrate_emit_ts::Package;
use purecrate_pack::{disk_path, generated};

/// The oxfmt the committed examples were formatted with
/// (`examples/package.json`); another may lay a line out otherwise.
pub const VERSION: &str = "0.71.0";

/// `PURECRATE_OXFMT`, else `node_modules/.bin/oxfmt` in each of `near` or a
/// directory above it, else `oxfmt` on `PATH`.
pub fn locate(near: &[&Path]) -> Result<PathBuf, String> {
    if let Some(path) = env::var_os("PURECRATE_OXFMT").map(PathBuf::from) {
        return match path.is_file() {
            true => Ok(path),
            false => Err(format!("oxfmt not found: PURECRATE_OXFMT names {}, which is no file", path.display())),
        };
    }
    for dir in near {
        // An absolute path, so a relative one is searched above too, and
        // found from the scratch directory oxfmt runs in. `dir` need not
        // exist yet (`--out ts/plain` on a new example).
        let dir = fs::canonicalize(dir).or_else(|_| std::path::absolute(dir)).unwrap_or_else(|_| dir.to_path_buf());
        if let Some(found) = dir.ancestors().map(|d| d.join("node_modules/.bin/oxfmt")).find(|p| p.is_file()) {
            return Ok(found);
        }
    }
    let on_path = env::var_os("PATH")
        .into_iter()
        .flat_map(|paths| env::split_paths(&paths).collect::<Vec<_>>())
        .map(|d| d.join("oxfmt"))
        .find(|p| p.is_file());
    on_path.ok_or_else(|| {
        let places: Vec<String> = near.iter().map(|d| d.display().to_string()).collect();
        format!(
            "oxfmt not found: the generated files are laid out by oxfmt {VERSION}. \
             Searched PURECRATE_OXFMT, node_modules/.bin from {} upward, and PATH. \
             Install it with `npm install -D oxfmt@{VERSION}` beside the output, or set PURECRATE_OXFMT",
            places.join(" and ")
        )
    })
}

/// The version `oxfmt` reports, when it reports one.
pub fn version(oxfmt: &Path) -> Option<String> {
    let out = Command::new(oxfmt).arg("--version").output().ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    text.split_whitespace().last().map(str::to_string)
}

/// Lays out each generated file of `pkg` with `oxfmt`, in a scratch
/// directory of `scratch`'s name.
pub fn format(pkg: &mut Package, oxfmt: &Path, scratch: &Path) -> Result<(), String> {
    if scratch.exists() {
        fs::remove_dir_all(scratch).map_err(|e| format!("clear {}: {e}", scratch.display()))?;
    }
    let result = format_in(pkg, oxfmt, scratch);
    fs::remove_dir_all(scratch).ok();
    result
}

fn format_in(pkg: &mut Package, oxfmt: &Path, dir: &Path) -> Result<(), String> {
    let mut paths = Vec::new();
    for file in pkg.files.iter().filter(|f| generated(&f.stem)) {
        let rel = disk_path(&file.stem);
        let path = dir.join(&rel);
        fs::create_dir_all(path.parent().unwrap_or(dir)).map_err(|e| format!("mkdir {}: {e}", dir.display()))?;
        fs::write(&path, &file.source).map_err(|e| format!("write {}: {e}", path.display()))?;
        paths.push(rel);
    }
    if paths.is_empty() {
        return Ok(());
    }
    // What an `.editorconfig` above the directory could otherwise set.
    let config = format!(
        "{{\n  \"printWidth\": {},\n  \"useTabs\": false,\n  \"tabWidth\": 2,\n  \"endOfLine\": \"lf\"\n}}\n",
        purecrate_emit_ts::WIDTH
    );
    fs::write(dir.join("oxfmt.json"), config).map_err(|e| format!("write {}: {e}", dir.display()))?;
    let out = Command::new(oxfmt)
        .args(["-c", "oxfmt.json", "--write"])
        .args(&paths)
        .current_dir(dir)
        .output()
        .map_err(|e| format!("run {}: {e}", oxfmt.display()))?;
    if !out.status.success() {
        return Err(format!(
            "{} failed on the generated files:\n{}{}",
            oxfmt.display(),
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    for file in pkg.files.iter_mut().filter(|f| generated(&f.stem)) {
        let path = dir.join(disk_path(&file.stem));
        file.source = fs::read_to_string(&path).map_err(|e| format!("read {}: {e}", path.display()))?;
    }
    Ok(())
}
