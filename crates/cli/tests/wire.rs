//! `--schema zod|valibot|arktype` writes a wire schema that type-checks and
//! reads serde's default JSON into the domain value. `fixtures/wire_shapes.rs`
//! has every type form a schema prints: each integer width, floats, `()`,
//! `Option` (missing or `null`), `Vec`, tuples, newtypes, `Box`, unit, tuple
//! and struct variants, and recursive enums and structs.
//!
//! 64-bit integers are written as decimal strings: the schemas do not read
//! them from JSON numbers yet.

#[allow(dead_code, unused_macros)]
mod support;

use std::fs;
use std::process::Command;

use purecrate_check::accept;
use purecrate_emit_ts::WireSchema;
use purecrate_pack::{assemble_with, disk_path};
use purecrate_syntax::parse_source;

const SOURCE: &str = include_str!("fixtures/wire_shapes.rs");

/// Reads with the library's own entry point; a rejected input throws.
fn parse_fn(schema: WireSchema) -> (&'static str, &'static str) {
    match schema {
        WireSchema::Zod => ("", "(s, x) => s.parse(x)"),
        WireSchema::Valibot => ("import * as v from \"valibot\";\n", "(s, x) => v.parse(s, x)"),
        WireSchema::Arktype => ("", "(s, x) => s.assert(x)"),
    }
}

const CASES: &str = r#"
const show = (x) =>
  x === undefined ? "undefined"
  : x === null ? "null"
  : typeof x === "bigint" ? `${x}n`
  : Array.isArray(x) ? `[${x.map(show).join(",")}]`
  : typeof x === "object" ? `{${Object.keys(x).sort().map((k) => `${k}:${show(x[k])}`).join(",")}}`
  : JSON.stringify(x);

const ints = {
  a: -128, b: 32767, c: -2147483648, d: "-9223372036854775808",
  e: 255, f: 65535, g: 4294967295, h: "18446744073709551615", i: 9007199254740991,
};
const intsValue = {
  a: -128, b: 32767, c: -2147483648, d: -9223372036854775808n,
  e: 255, f: 65535, g: 4294967295, h: 18446744073709551615n, i: 9007199254740991,
};
const holder = {
  tree: { Node: ["Leaf", 1, { Node: ["Leaf", 2, "Leaf"] }] },
  shapes: ["Dot", { Circle: 1.5 }, { Rect: [1, 2] }, { Named: { label: "x", tag: 3 } }, { Named: { label: "y" } }, { Tagged: "7" }],
  first: null,
  chain: { value: 1, next: { value: 2 } },
  floats: { x: 0.1, y: 2.25 },
  misc: { flag: true, text: "t", unit: null, list: [1, 2], pair: [3, "p"], id: "9007199254740993", labels: ["a"], ints },
};
const holderValue = {
  tree: { kind: "Node", content: [{ kind: "Leaf" }, 1, { kind: "Node", content: [{ kind: "Leaf" }, 2, { kind: "Leaf" }] }] },
  shapes: [
    { kind: "Dot" }, { kind: "Circle", content: [1.5] }, { kind: "Rect", content: [1, 2] },
    { kind: "Named", label: "x", tag: 3 }, { kind: "Named", label: "y", tag: null }, { kind: "Tagged", content: [7n] },
  ],
  first: null,
  chain: { value: 1, next: { value: 2, next: null } },
  floats: { x: Math.fround(0.1), y: 2.25 },
  misc: {
    flag: true, text: "t", unit: undefined, maybe: null, list: [1, 2], pair: [3, "p"],
    id: 9007199254740993n, labels: ["a"], ints: intsValue,
  },
};

const accepts = [
  ["Holder", holder, holderValue],
  ["Ints", ints, intsValue],
  ["Shape", { Rect: [-1, 0] }, { kind: "Rect", content: [-1, 0] }],
  ["Chain", { value: 3, next: null }, { value: 3, next: null }],
];
const rejects = [
  ["Ints", { ...ints, a: 128 }],
  ["Ints", { ...ints, d: 1 }],
  ["Shape", { Unknown: 1 }],
  ["Shape", "Circle"],
  ["Tree", { Node: ["Leaf", 1] }],
  ["Chain", { value: 1.5 }],
  ["Holder", { ...holder, tree: undefined }],
];

const out = [];
for (const [name, input, want] of accepts) {
  let got;
  try {
    got = show(parse(w[name], input));
  } catch (e) {
    got = `threw ${e instanceof Error ? e.message.split("\n")[0] : e}`;
  }
  if (got !== show(want)) out.push(`${name}: want ${show(want)}\n  got ${got}`);
}
for (const [name, input] of rejects) {
  let threw = false;
  try {
    parse(w[name], input);
  } catch {
    threw = true;
  }
  if (!threw) out.push(`${name}: accepted ${JSON.stringify(input)}`);
}
console.log(out.length === 0 ? "ok" : out.join("\n"));
"#;

#[test]
fn wire_schemas_type_check_and_read_serde_json_into_the_domain_value() {
    if std::env::var_os("PURECRATE_SKIP_NODE").is_some() {
        return;
    }
    let krate = parse_source("wire_shapes", SOURCE).expect("parse");
    let typed = accept(&krate).expect("accept");
    for schema in [WireSchema::Zod, WireSchema::Valibot, WireSchema::Arktype] {
        let lib = schema.runtime_dep();
        let dir = std::env::temp_dir().join(format!("purecrate-wire-{lib}-{}", std::process::id()));
        if dir.exists() {
            fs::remove_dir_all(&dir).ok();
        }
        for file in assemble_with(&typed, Some(schema)).files {
            let path = dir.join(disk_path(&file.stem));
            fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
            fs::write(path, file.source).expect("write");
        }
        let package = format!("boundary-{lib}");
        link(&dir, "purecrate", "boundary");
        link(&dir, schema.package(), &package);
        link(&dir, lib, &format!("{package}/node_modules/{lib}"));
        if schema == WireSchema::Arktype {
            link(&dir, "@ark", &format!("{package}/node_modules/@ark"));
        }
        support::typecheck(&dir);

        let (import, parse) = parse_fn(schema);
        fs::write(
            dir.join("driver.ts"),
            format!("{import}import * as w from \"./src/purecrate-wire.ts\";\nconst parse = {parse};\n{CASES}"),
        )
        .expect("write driver");
        let output = Command::new("node")
            .arg("driver.ts")
            .current_dir(&dir)
            .output()
            .expect("node");
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            output.status.success() && stdout.trim() == "ok",
            "{lib} in {}:\nstdout:\n{stdout}\nstderr:\n{}",
            dir.display(),
            String::from_utf8_lossy(&output.stderr)
        );
        let _ = fs::remove_dir_all(&dir);
    }
}

fn link(dir: &std::path::Path, name: &str, rel: &str) {
    let target = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../packages").join(rel);
    let modules = dir.join("node_modules").join(name);
    fs::create_dir_all(modules.parent().expect("node_modules")).expect("mkdir");
    let _ = fs::remove_file(&modules);
    std::os::unix::fs::symlink(&target, &modules).unwrap_or_else(|e| panic!("link {name}: {e} ({target:?})"));
}
