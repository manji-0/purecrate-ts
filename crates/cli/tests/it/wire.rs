//! `--schema zod|valibot|arktype` writes a wire schema that type-checks and
//! reads serde's default JSON into the domain value. `fixtures/wire_shapes.rs`
//! has every type form a schema prints: each integer width, floats, `char`, `Uuid`, `()`,
//! `Option` (missing or `null`), `Vec`, tuples, newtypes, `Box`, unit, tuple
//! and struct variants, and recursive enums and structs. `Id`, `Label` and
//! `Sealed` are closed (design/04 §5): read by shape, as serde's derive does,
//! and built through the package-internal constructor.
//!
//! 64-bit integers come as serde_json writes them, JSON numbers, read from
//! the text with `parseJson`; a plain `JSON.parse` rounds those past 2^53 and
//! the schema must then reject them. Decimal strings are accepted too.

use crate::support;

#[allow(dead_code, unused_macros)]

use std::fs;
use std::process::Command;

use purecrate_check::accept;
use purecrate_emit_ts::WireSchema;
use purecrate_pack::{assemble_with, disk_path};
use purecrate_syntax::parse_source;

const SOURCE: &str = include_str!("../fixtures/wire_shapes.rs");

/// The library's throwing reader (`parse`) and its non-throwing check
/// (`valid`): a rejected input must come back as a failure, not a throw.
fn parse_fns(schema: WireSchema) -> (&'static str, &'static str, &'static str) {
    match schema {
        WireSchema::Zod => ("", "(s, x) => s.parse(x)", "(s, x) => s.safeParse(x).success"),
        WireSchema::Valibot => (
            "import * as v from \"valibot\";\n",
            "(s, x) => v.parse(s, x)",
            "(s, x) => v.safeParse(s, x).success",
        ),
        WireSchema::Arktype => (
            "import { type } from \"arktype\";\n",
            "(s, x) => s.assert(x)",
            "(s, x) => !(s(x) instanceof type.errors)",
        ),
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

// serde_json's output for `ints`: 64-bit integers are JSON numbers.
const intsText =
  '{"a":-128,"b":32767,"c":-2147483648,"d":-9223372036854775808,"e":255,"f":65535,"g":4294967295,"h":18446744073709551615,"i":9007199254740991}';

const accepts = [
  ["Holder", holder, holderValue],
  ["Ints", ints, intsValue],
  ["Ints", parseJson(intsText), intsValue],
  ["Ints", { ...ints, d: -5, h: 9007199254740991 }, { ...intsValue, d: -5n, h: 9007199254740991n }],
  ["Shape", parseJson('{"Tagged":9007199254740993}'), { kind: "Tagged", content: [9007199254740993n] }],
  ["Shape", JSON.parse('{"Tagged":7}'), { kind: "Tagged", content: [7n] }],
  ["Shape", { Rect: [-1, 0] }, { kind: "Rect", content: [-1, 0] }],
  ["Chain", { value: 3, next: null }, { value: 3, next: null }],
  // serde ignores unknown struct fields by default, inside a variant too.
  ["Chain", { value: 3, extra: true }, { value: 3, next: null }],
  ["Shape", { Named: { label: "x", extra: 1 } }, { kind: "Named", label: "x", tag: null }],
  // serde_json also reads a unit variant written as a map with a `null` value.
  ["Shape", { Dot: null }, { kind: "Dot" }],
  ["Tree", { Node: [{ Leaf: null }, 1, "Leaf"] }, { kind: "Node", content: [{ kind: "Leaf" }, 1, { kind: "Leaf" }] }],
  ["Sealed", { code: -1 }, { code: -1, hint: null }],
  ["Node", { Group: { label: "g", children: [{ Leaf: 1 }, { Group: { label: "h", children: [] } }] } },
    { kind: "Group", content: [{ label: "g", children: [{ kind: "Leaf", content: [1] }, { kind: "Group", content: [{ label: "h", children: [] }] }] }] }],
  ["Early", { late: { n: 2 } }, { late: { n: 2 } }],
  ["Letters", { one: "a", maybe: "😀", many: ["é", "\u{10ffff}", "\u{ffff}"] }, { one: "a", maybe: "😀", many: ["é", "\u{10ffff}", "\u{ffff}"] }],
  ["Letters", { one: "\n", many: [] }, { one: "\n", maybe: null, many: [] }],
  // serde reads a `Uuid` from any form `Uuid::parse_str` takes; the value is canonical.
  ["Ids", { one: "67E55044-10B1-426F-9247-BB680E5FE0C8", maybe: "{67e55044-10b1-426f-9247-bb680e5fe0c8}", many: ["urn:uuid:67e55044-10b1-426f-9247-bb680e5fe0c8", "67e5504410b1426f9247bb680e5fe0c8"] },
    { one: "67e55044-10b1-426f-9247-bb680e5fe0c8", maybe: "67e55044-10b1-426f-9247-bb680e5fe0c8", many: ["67e55044-10b1-426f-9247-bb680e5fe0c8", "67e55044-10b1-426f-9247-bb680e5fe0c8"] }],
];
const rejects = [
  ["Ints", { ...ints, a: 128 }],
  ["Ints", { ...ints, d: 1.5 }],
  ["Ints", JSON.parse(intsText)],
  ["Shape", JSON.parse('{"Tagged":9007199254740993}')],
  ["Ints", { ...ints, h: -1 }],
  ["Ints", { ...ints, h: "18446744073709551616" }],
  ["Ints", parseJson('{"a":1,"b":1,"c":1,"d":9223372036854775808,"e":1,"f":1,"g":1,"h":1,"i":1}')],
  ["Shape", { Unknown: 1 }],
  // serde's externally tagged enum: the wrapper has exactly one key.
  ["Shape", { Circle: 1.5, Rect: [1, 2] }],
  ["Shape", { Tagged: 7, extra: 1 }],
  ["Shape", { Named: { label: "x" }, Dot: null }],
  ["Tree", { Node: ["Leaf", 1, "Leaf"], Leaf: null }],
  ["Shape", {}],
  ["Shape", { Dot: 1 }],
  ["Shape", { Dot: null, x: 1 }],
  ["Shape", "Circle"],
  ["Tree", { Node: ["Leaf", 1] }],
  ["Chain", { value: 1.5 }],
  ["Node", { Group: { label: "g", children: [{ Leaf: 1.5 }] } }],
  ["Early", { late: {} }],
  ["Sealed", { code: 2147483648, hint: "h" }],
  ["Holder", { ...holder, tree: undefined }],
  // serde reads a `char` from a string of exactly one scalar value.
  ["Letters", { one: "", many: [] }],
  ["Letters", { one: "ab", many: [] }],
  ["Letters", { one: "e\u{301}", many: [] }],
  ["Letters", { one: "\ud800", many: [] }],
  ["Letters", { one: "\udfff", many: [] }],
  ["Letters", { one: 97, many: [] }],
  ["Letters", { one: "a", many: ["😀😀"] }],
  ["Ids", { one: "67e55044-10b1-426f-9247-bb680e5fe0c", many: [] }],
  ["Ids", { one: "URN:UUID:67e55044-10b1-426f-9247-bb680e5fe0c8", many: [] }],
  ["Ids", { one: "{67e5504410b1426f9247bb680e5fe0c8}", many: [] }],
  ["Ids", { one: 7, many: [] }],
  ["Ids", { one: "67e55044-10b1-426f-9247-bb680e5fe0c8", many: ["x"] }],
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
const text = (x) => JSON.stringify(x, (_k, v) => (typeof v === "bigint" ? `${v}n` : v));
for (const [name, input] of rejects) {
  try {
    if (valid(w[name], input)) out.push(`${name}: accepted ${text(input)}`);
  } catch (e) {
    out.push(`${name}: threw instead of rejecting ${text(input)}: ${e instanceof Error ? e.message : e}`);
  }
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
        let dir = support::scratch(&format!("wire-{lib}"));
        if dir.exists() {
            fs::remove_dir_all(&dir).ok();
        }
        for file in assemble_with(&typed, Some(schema)).files {
            let path = dir.join(disk_path(&file.stem));
            fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
            fs::write(path, file.source).expect("write");
        }
        let package = format!("boundary-{lib}");
        link(&dir, lib, &format!("{package}/node_modules/{lib}"));
        if schema == WireSchema::Arktype {
            link(&dir, "@ark", &format!("{package}/node_modules/@ark"));
        }
        support::typecheck(&dir);

        let (import, parse, valid) = parse_fns(schema);
        fs::write(
            dir.join("driver.ts"),
            format!(
                "{import}import {{ parseJson }} from \"./src/index.ts\";\nimport * as w from \"./src/purecrate-wire.ts\";\n\
                 const parse = {parse};\nconst valid = {valid};\n{CASES}"
            ),
        )
        .expect("write driver");
        let output = Command::new("node")
            .arg(format!("--conditions={}", support::SOURCE_CONDITION))
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
