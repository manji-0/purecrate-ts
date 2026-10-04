//! Every generated domain line is at most 100 columns (design/03 §1), a wide
//! East Asian character counting two, as oxfmt counts it. Lines oxfmt does
//! not break either are left as they are: an `import` of one name, and a
//! template literal, whose text a break would change.
//! Comment lines are left as they are. Runtime and adapter copies are
//! hand-written and checked on their own.

use crate::support;

const WIDTH: usize = 100;

fn is_comment(line: &str) -> bool {
    let t = line.trim_start();
    t.starts_with("//") || t.starts_with("/*") || t.starts_with('*')
}

/// What oxfmt keeps on one line however long.
fn unbreakable(line: &str) -> bool {
    let lone_import = line.starts_with("import ")
        && line.split_once('{').and_then(|(_, r)| r.split_once('}')).is_some_and(|(names, _)| !names.contains(','));
    lone_import || line.contains('`')
}

#[test]
fn generated_lines_fit_the_width() {
    let inputs = support::corpus::inputs();
    let mut over = Vec::new();
    let mut files = 0;
    for (name, src) in &inputs {
        for (file, text) in support::corpus::generated(name, src) {
            files += 1;
            for (i, line) in text.lines().enumerate() {
                let cols = purecrate_emit_ts::columns(line);
                if !is_comment(line) && !unbreakable(line) && cols > WIDTH {
                    over.push(format!("{file}:{}:{cols}: {line}", i + 1));
                }
            }
        }
    }
    assert!(files > 200, "read {files} files");
    assert!(over.is_empty(), "{} generated lines over {WIDTH} characters:\n{}", over.len(), over.join("\n"));
}
