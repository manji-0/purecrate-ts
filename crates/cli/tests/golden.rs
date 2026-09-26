//! `examples/counter-ts` is the committed output for `examples/counter`.
//! Regenerate with:
//! `cargo run -p purecrate-ts -- build examples/counter --out examples/counter-ts`

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use purecrate_check::{check, prune_unreachable};
use purecrate_pack::{assemble, disk_path};
use purecrate_syntax::parse_source;

fn examples() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples")
}

fn files_under(root: &Path) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir).expect("read_dir") {
            let path = entry.expect("entry").path();
            if path.is_dir() {
                stack.push(path);
            } else {
                let rel = path.strip_prefix(root).expect("prefix");
                out.insert(rel.to_string_lossy().replace('\\', "/"));
            }
        }
    }
    out
}

#[test]
fn counter_ts_matches_generated_output() {
    let src = fs::read_to_string(examples().join("counter/src/lib.rs")).expect("read counter");
    let krate = parse_source("counter", &src).expect("parse counter");
    assert_eq!(check(&krate), vec![]);
    let pkg = assemble(&prune_unreachable(&krate));
    let golden = examples().join("counter-ts");

    let generated: BTreeSet<String> = pkg.files.iter().map(|f| disk_path(&f.stem)).collect();
    assert_eq!(files_under(&golden), generated, "file set differs");

    for file in &pkg.files {
        let path = disk_path(&file.stem);
        let on_disk = fs::read_to_string(golden.join(&path)).expect("read golden");
        assert_eq!(on_disk, file.source, "{path} differs; regenerate examples/counter-ts");
    }
}
