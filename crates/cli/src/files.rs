//! The files of a crate: the root and every module file `mod x;` reaches,
//! at `x.rs` or `x/mod.rs` as rustc looks for them. `#[path]` is not followed.

use std::fs;
use std::path::{Path, PathBuf};

use purecrate_syntax::module_decls_vis;

pub struct CrateFile {
    pub path: PathBuf,
    pub text: String,
    /// Every `mod` from the root to this file is `pub`.
    pub public: bool,
}

/// The root first, then module files in declaration order, depth first.
pub fn crate_files(root: &Path) -> Result<Vec<CrateFile>, String> {
    let mut out = Vec::new();
    visit(root, module_dir(root, true), true, &mut out)?;
    Ok(out)
}

fn visit(path: &Path, dir: PathBuf, public: bool, out: &mut Vec<CrateFile>) -> Result<(), String> {
    let text = fs::read_to_string(path).map_err(|e| format!("read {}: {e}", path.display()))?;
    let decls = module_decls_vis(&text).map_err(|e| format!("{}:{e}", path.display()))?;
    out.push(CrateFile { path: path.to_path_buf(), text, public });
    for (decl, decl_public) in decls {
        let base = decl.iter().fold(dir.clone(), |d, seg| d.join(seg));
        let flat = base.with_extension("rs");
        let nested = base.join("mod.rs");
        let file = if flat.exists() {
            flat
        } else if nested.exists() {
            nested
        } else {
            return Err(format!(
                "{}: module `{}` has no file at {} or {}",
                path.display(),
                decl.join("::"),
                flat.display(),
                nested.display()
            ));
        };
        visit(&file, base, public && decl_public, out)?;
    }
    Ok(())
}

/// Where `mod x;` in `file` looks for `x.rs`.
fn module_dir(file: &Path, is_root: bool) -> PathBuf {
    let parent = file.parent().unwrap_or(Path::new(".")).to_path_buf();
    let stem = file.file_stem().and_then(|s| s.to_str()).unwrap_or("");
    if is_root || stem == "mod" {
        parent
    } else {
        parent.join(stem)
    }
}
