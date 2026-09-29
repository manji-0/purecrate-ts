//! `toJson` writes domain values as serde_json writes the Rust value, byte
//! for byte (design/04 §6). The Rust side is the vendored serde_json over
//! the `Serialize` impls `fixture!` generates with derive's data-model calls.
//!
//! - Floats: `Json.f64` / `Json.f32` against serde_json (ryu) on boundary
//!   values and pseudo-random bit patterns.
//! - Shapes: every value of `fixtures/wire_shapes.rs` below is written by
//!   serde_json, read by the schema, and written back by `toJson`.
//! - Optimistic update: for every reachable `examples/payment` state and
//!   event, the client reads the server's JSON, runs `step`, and writes the
//!   same bytes the server writes for its own `step`.

#[allow(dead_code, unused_macros)]
mod support;

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use purecrate_check::accept;
use purecrate_emit_ts::WireSchema;
use purecrate_pack::{assemble_with, disk_path};
use purecrate_syntax::parse_source;

purecrate_canon::fixture!(mod shapes = "fixtures/wire_shapes.rs", "fixtures/wire_shapes_driver.rs");
purecrate_canon::fixture!(mod payment = "../../../examples/payment/src/lib.rs", "fixtures/payment_driver.rs");

/// JS string literal.
fn js(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if (c as u32) < 0x20 || (c as u32) > 0x7e => out.push_str(&format!("\\u{{{:x}}}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Writes the zod package for `source` (or an empty directory when `None`),
/// links the runtime, runs `script` on node, and returns its stdout.
fn run_node(name: &str, source: Option<&str>, script: &str) -> Option<String> {
    run_node_with(WireSchema::Zod, name, source, script)
}

fn run_node_with(schema: WireSchema, name: &str, source: Option<&str>, script: &str) -> Option<String> {
    if std::env::var_os("PURECRATE_SKIP_NODE").is_some() {
        return None;
    }
    let dir = std::env::temp_dir().join(format!(
        "purecrate-wire-write-{name}-{}-{}",
        schema.runtime_dep(),
        std::process::id()
    ));
    if dir.exists() {
        fs::remove_dir_all(&dir).ok();
    }
    fs::create_dir_all(&dir).expect("mkdir");
    link(&dir, "purecrate", "boundary");
    if let Some(source) = source {
        let krate = parse_source(name, source).expect("parse");
        let typed = accept(&krate).unwrap_or_else(|d| panic!("{name} rejected: {d:#?}"));
        for file in assemble_with(&typed, Some(schema)).files {
            let path = dir.join(disk_path(&file.stem));
            fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
            fs::write(path, file.source).expect("write");
        }
        let lib = schema.runtime_dep();
        let package = format!("boundary-{lib}");
        link(&dir, schema.package(), &package);
        link(&dir, lib, &format!("{package}/node_modules/{lib}"));
        if schema == WireSchema::Arktype {
            link(&dir, "@ark", &format!("{package}/node_modules/@ark"));
        }
        support::typecheck(&dir);
    }
    fs::write(dir.join("driver.ts"), script).expect("write driver");
    let output = Command::new("node")
        .arg(format!("--conditions={}", support::SOURCE_CONDITION))
        .arg("driver.ts")
        .current_dir(&dir)
        .output()
        .expect("node");
    assert!(
        output.status.success(),
        "node in {}:\n{}",
        dir.display(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&dir);
    Some(String::from_utf8(output.stdout).expect("utf8"))
}

fn link(dir: &Path, name: &str, rel: &str) {
    let target = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../packages").join(rel);
    let modules = dir.join("node_modules").join(name);
    fs::create_dir_all(modules.parent().expect("node_modules")).expect("mkdir");
    let _ = fs::remove_file(&modules);
    std::os::unix::fs::symlink(&target, &modules).unwrap_or_else(|e| panic!("link {name}: {e} ({target:?})"));
}

fn assert_ok(stdout: Option<String>) {
    if let Some(stdout) = stdout {
        assert_eq!(stdout.trim(), "ok", "mismatches:\n{stdout}");
    }
}

/// Deterministic xorshift64*.
struct Bits(u64);

impl Bits {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }
}

#[test]
fn floats_are_written_as_serde_json_writes_them() {
    let mut f64s: Vec<f64> = vec![
        0.0, -0.0, 1.0, -1.0, 0.1, 0.5, 1.5, 100.0, 123.456, 1e15, 9.999999999999998e15, 1e16, 1.5e16, 1e17,
        1e21, 1e22, 1e-4, 1.5e-4, 1e-5, 1.5e-5, 1e-6, 1e-7, 1.5e-7, 5e-324, f64::MIN_POSITIVE, f64::MAX,
        -f64::MAX, f64::EPSILON, 9007199254740993.0, 1234567890123456.7, 0.30000000000000004,
        f64::NAN, f64::INFINITY, f64::NEG_INFINITY,
    ];
    let mut f32s: Vec<f32> = vec![
        0.0, -0.0, 1.0, 0.1, 0.2, 0.3, 1.5, 16777216.0, 16777217.0, 1e12, 9.999999e12, 1e13, 1.5e13, 1e14,
        1e-5, 1e-6, 1.5e-6, 1e-7, f32::MIN_POSITIVE, 1e-45, f32::MAX, -f32::MAX, f32::EPSILON, 3.4028235e38,
        f32::NAN, f32::INFINITY,
    ];
    let mut bits = Bits(0x9e37_79b9_7f4a_7c15);
    for _ in 0..20_000 {
        f64s.push(f64::from_bits(bits.next()));
        f32s.push(f32::from_bits(bits.next() as u32));
        // Values of everyday size too, where plain notation is used.
        f64s.push((bits.next() % 2_000_000_000) as f64 / 1000.0 - 1e6);
        f32s.push((bits.next() % 2_000_000) as f32 / 100.0);
    }
    let rows64: Vec<String> = f64s
        .iter()
        .map(|x| format!("[{},{}]", js(&format!("{:016x}", x.to_bits())), js(&serde_json::to_string(x).unwrap())))
        .collect();
    let rows32: Vec<String> = f32s
        .iter()
        .map(|x| format!("[{},{}]", js(&format!("{:08x}", x.to_bits())), js(&serde_json::to_string(x).unwrap())))
        .collect();
    let script = format!(
        "import {{ Json }} from \"purecrate\";\n\
         const f64 = (h) => new Float64Array(new BigUint64Array([BigInt(\"0x\" + h)]).buffer)[0];\n\
         const f32 = (h) => new Float32Array(new Uint32Array([parseInt(h, 16)]).buffer)[0];\n\
         const out = [];\n\
         for (const [h, want] of [{}]) {{ const got = Json.f64(f64(h)); if (got !== want) out.push(`f64 ${{h}}: want ${{want}} got ${{got}}`); }}\n\
         for (const [h, want] of [{}]) {{ const got = Json.f32(f32(h)); if (got !== want) out.push(`f32 ${{h}}: want ${{want}} got ${{got}}`); }}\n\
         console.log(out.length === 0 ? \"ok\" : out.slice(0, 40).join(\"\\n\") + `\\n(${{out.length}} in all)`);\n",
        rows64.join(","),
        rows32.join(",")
    );
    assert_ok(run_node("floats", None, &script));
}

fn shape_values() -> Vec<(&'static str, String)> {
    use shapes::*;
    let ints = || Ints {
        a: -128,
        b: 32767,
        c: i32::MIN,
        d: i64::MIN,
        e: 255,
        f: 65535,
        g: u32::MAX,
        h: u64::MAX,
        i: 9007199254740991,
    };
    let misc = |text: &str, maybe: Option<i32>, labels: Option<Vec<Label>>| Misc {
        flag: true,
        text: text.to_string(),
        unit: (),
        maybe,
        list: vec![1, -2, 3],
        pair: (4, "p".to_string()),
        id: id_of(9007199254740993),
        labels,
        ints: Box::new(ints()),
    };
    let holder = Holder {
        tree: Tree::Node(Box::new(Tree::Leaf), 1, Box::new(Tree::Node(Box::new(Tree::Leaf), 2, Box::new(Tree::Leaf)))),
        shapes: vec![
            Shape::Dot,
            Shape::Circle(1.5),
            Shape::Circle(-0.0),
            Shape::Rect(1, 2),
            Shape::Named { label: label_of("x".into()), tag: Some(3) },
            Shape::Named { label: label_of("y".into()), tag: None },
            Shape::Tagged(id_of(u64::MAX)),
        ],
        first: Some(Shape::Circle(1e21)),
        chain: Chain { value: 1, next: Some(Box::new(Chain { value: 2, next: None })) },
        floats: Floats { x: 0.1, y: 2.25 },
        misc: misc("t", Some(-1), Some(vec![label_of("a".into())])),
    };
    vec![
        ("Holder", serde_json::to_string(&holder).unwrap()),
        ("Holder", serde_json::to_string(&Holder { first: None, shapes: vec![], ..holder }).unwrap()),
        ("Misc", serde_json::to_string(&misc("", None, None)).unwrap()),
        ("Misc", serde_json::to_string(&misc("\"\\\n\t\u{1}\u{7f}é😀\u{2028}", Some(0), Some(vec![]))).unwrap()),
        ("Ints", serde_json::to_string(&ints()).unwrap()),
        ("Floats", serde_json::to_string(&Floats { x: f32::NAN, y: f64::INFINITY }).unwrap()),
        ("Floats", serde_json::to_string(&Floats { x: 1e-7, y: 1e-7 }).unwrap()),
        ("Tree", serde_json::to_string(&Tree::Leaf).unwrap()),
        ("Sealed", serde_json::to_string(&sealed_of(-1, Some("h".into()))).unwrap()),
        ("Id", serde_json::to_string(&id_of(0)).unwrap()),
        ("Label", serde_json::to_string(&label_of("l".into())).unwrap()),
        (
            "Letters",
            serde_json::to_string(&Letters { one: '"', maybe: Some('😀'), many: vec!['\\', '\n', '\u{7f}', '\u{2028}', '\u{10ffff}'] })
                .unwrap(),
        ),
        ("Letters", serde_json::to_string(&Letters { one: '\0', maybe: None, many: vec![] }).unwrap()),
        (
            "Ids",
            serde_json::to_string(&Ids {
                one: uuid::Uuid::nil(),
                maybe: Some(uuid::Uuid::max()),
                many: vec![uuid::Uuid::from_u128(0x67e5_5044_10b1_426f_9247_bb68_0e5f_e0c8)],
            })
            .unwrap(),
        ),
        ("Ids", serde_json::to_string(&Ids { one: uuid::Uuid::from_u128(1), maybe: None, many: vec![] }).unwrap()),
    ]
}

#[test]
fn shapes_read_from_serde_json_are_written_back_byte_for_byte() {
    let values = shape_values();
    // Non-finite floats are written as `null`, which neither serde_json nor
    // the schema reads back: that row is written from the domain value.
    let rows: Vec<String> = values
        .iter()
        .map(|(ty, text)| format!("[{},{}]", js(ty), js(text)))
        .collect();
    let script = format!(
        "import {{ parseJson, Int }} from \"purecrate\";\n\
         import * as w from \"./src/purecrate-wire.ts\";\n\
         const out = [];\n\
         for (const [ty, text] of [{}]) {{\n\
           if (text.includes(\"null}}\") && ty === \"Floats\") {{\n\
             const got = w.toJson.Floats({{ x: Int.f32.of(NaN), y: Int.f64.of(Infinity) }});\n\
             if (got !== text) out.push(`Floats: want ${{text}} got ${{got}}`);\n\
             continue;\n\
           }}\n\
           let got;\n\
           try {{ got = w.toJson[ty](w[ty].parse(parseJson(text))); }} catch (e) {{ got = `threw ${{e}}`; }}\n\
           if (got !== text) out.push(`${{ty}}: want ${{text}}\\n  got ${{got}}`);\n\
         }}\n\
         console.log(out.length === 0 ? \"ok\" : out.join(\"\\n\"));\n",
        rows.join(",")
    );
    assert_ok(run_node("shapes", Some(shapes::SOURCE), &script));
}

/// A state rebuilt by running its events again (`step` consumes the
/// state, and the example derives nothing to copy it with).
fn replay(t: u8, codes: &[u8]) -> payment::PaymentIntent {
    codes.iter().fold(payment::create(payment::terms_of(t)), |s, &c| {
        payment::step(s, payment::decode(c)).unwrap_or_else(|_| panic!("replay {t} {codes:?}"))
    })
}

/// Every state reachable in three events under each capture and
/// confirmation method, keyed by the server's JSON for it, with a run that
/// reaches it.
fn payment_states() -> BTreeMap<String, (u8, Vec<u8>)> {
    let mut states = BTreeMap::new();
    for t in 0u8..4 {
        let mut frontier: Vec<Vec<u8>> = vec![Vec::new()];
        for depth in 0..=3 {
            let mut next = Vec::new();
            for codes in frontier {
                let text = serde_json::to_string(&replay(t, &codes)).unwrap();
                if states.contains_key(&text) {
                    continue;
                }
                if depth < 3 {
                    for code in 0u8..12 {
                        if payment::step(replay(t, &codes), payment::decode(code)).is_ok() {
                            next.push([codes.as_slice(), &[code]].concat());
                        }
                    }
                }
                states.insert(text, (t, codes));
            }
            frontier = next;
        }
    }
    states
}

#[test]
fn a_client_step_on_server_json_writes_what_the_server_writes() {
    let states = payment_states();
    assert!(states.len() > 20, "only {} states reached", states.len());
    let mut rows = Vec::new();
    for (state_text, (t, codes)) in &states {
        for code in 0u8..12 {
            let event_text = serde_json::to_string(&payment::decode(code)).unwrap();
            let server = match payment::step(replay(*t, codes), payment::decode(code)) {
                Ok(next) => serde_json::to_string(&next).unwrap(),
                Err(e) => format!("Err {}", serde_json::to_string(&e).unwrap()),
            };
            rows.push(format!("[{},{},{}]", js(state_text), js(&event_text), js(&server)));
        }
    }
    let script = format!(
        "import {{ parseJson }} from \"purecrate\";\n\
         import * as w from \"./src/purecrate-wire.ts\";\n\
         import {{ step }} from \"./src/index.ts\";\n\
         const out = [];\n\
         for (const [stateText, eventText, server] of [{}]) {{\n\
           const state = w.PaymentIntent.parse(parseJson(stateText));\n\
           const event = w.Event.parse(parseJson(eventText));\n\
           if (w.toJson.PaymentIntent(state) !== stateText) out.push(`state: ${{stateText}}`);\n\
           if (w.toJson.Event(event) !== eventText) out.push(`event: ${{eventText}}`);\n\
           const r = step(state, event);\n\
           const client = r.kind === \"Ok\" ? w.toJson.PaymentIntent(r.value) : `Err ${{w.toJson.PaymentError(r.error)}}`;\n\
           if (client !== server) out.push(`${{stateText}} + ${{eventText}}:\\n  server ${{server}}\\n  client ${{client}}`);\n\
         }}\n\
         console.log(out.length === 0 ? \"ok\" : out.slice(0, 20).join(\"\\n\"));\n",
        rows.join(",")
    );
    assert_ok(run_node("payment", Some(payment::SOURCE), &script));
}

/// `#[serde(try_from = "T")]` (design/04 §5): serde reads `T` and calls
/// `TryFrom`, failing on `Err`. The schema of every library reads a value
/// exactly when `X::try_from` accepts it, and gives the value it returns;
/// an event carrying a malformed ID is rejected as a whole.
#[test]
fn try_from_reads_what_the_checked_constructor_accepts() {
    use payment::{Amount, PaymentMethodId};
    let mut rows = Vec::new();
    let mut row = |ty: &str, text: String, want: String| rows.push(format!("[{},{},{}]", js(ty), js(&text), js(&want)));
    for s in ["", "pm_", "pm_x", "pa_x", "PM_x", "pm_é", "é", "pm_card"] {
        let want = match PaymentMethodId::try_from(s.to_string()) {
            Ok(id) => serde_json::to_string(&id).unwrap(),
            Err(_) => "Err".to_string(),
        };
        let text = serde_json::to_string(s).unwrap();
        row("PaymentMethodId", text.clone(), want.clone());
        let event = format!("{{\"AttachMethod\":{{\"id\":{text},\"kind\":\"Card\"}}}}");
        let want_event = if want == "Err" { want } else { event.clone() };
        row("Event", event, want_event);
    }
    for n in [i64::MIN, -1, 0, 49, 50, 2000, 99_999_999, 100_000_000, i64::MAX] {
        let want = match Amount::try_from(n) {
            Ok(a) => serde_json::to_string(&a).unwrap(),
            Err(_) => "Err".to_string(),
        };
        row("Amount", n.to_string(), want);
    }
    // The wrong shape fails before `try_from`.
    row("PaymentMethodId", "7".into(), "Err".into());
    // `i64` also reads digit strings (design/04 §3 rule 1), which serde_json
    // does not; `try_from` still runs on the value.
    row("Amount", "\"50\"".into(), "50".into());
    row("Amount", "\"49\"".into(), "Err".into());
    row("Amount", "50.5".into(), "Err".into());
    let rows = rows.join(",");
    for schema in [WireSchema::Zod, WireSchema::Valibot, WireSchema::Arktype] {
        let (import, valid, parse) = match schema {
            WireSchema::Zod => ("", "(s, x) => s.safeParse(x).success", "(s, x) => s.parse(x)"),
            WireSchema::Valibot => ("import * as v from \"valibot\";\n", "(s, x) => v.safeParse(s, x).success", "(s, x) => v.parse(s, x)"),
            WireSchema::Arktype => ("import { type } from \"arktype\";\n", "(s, x) => !(s(x) instanceof type.errors)", "(s, x) => s.assert(x)"),
        };
        let script = format!(
            "{import}import {{ parseJson }} from \"purecrate\";\n\
             import * as w from \"./src/purecrate-wire.ts\";\n\
             const valid = {valid};\nconst parse = {parse};\n\
             const out = [];\n\
             for (const [ty, text, want] of [{rows}]) {{\n\
               const x = parseJson(text);\n\
               const got = valid(w[ty], x) ? w.toJson[ty](parse(w[ty], x)) : \"Err\";\n\
               if (got !== want) out.push(`${{ty}} ${{text}}: want ${{want}} got ${{got}}`);\n\
             }}\n\
             console.log(out.length === 0 ? \"ok\" : out.join(\"\\n\"));\n"
        );
        assert_ok(run_node_with(schema, "try-from", Some(payment::SOURCE), &script));
    }
}

/// Every library's schema reads a `Uuid` from exactly the JSON strings
/// serde reads one from (`Uuid::parse_str`'s four forms, any case), and
/// `toJson` writes the canonical form serde writes.
#[test]
fn uuids_read_what_serde_reads() {
    use shapes::Ids;
    let good = uuid::Uuid::from_u128(0x67e5_5044_10b1_426f_9247_bb68_0e5f_e0c8);
    let mut texts: Vec<String> = Vec::new();
    for form in [
        good.hyphenated().to_string(),
        good.simple().to_string(),
        good.braced().to_string(),
        good.urn().to_string(),
    ] {
        texts.push(form.to_uppercase());
        texts.push(form[1..].to_string());
        texts.push(format!("{form} "));
        texts.push(form.replacen('6', "g", 1));
        texts.push(form.replacen('6', "é", 1));
        texts.push(form);
    }
    texts.extend(["", "URN:UUID:67e55044-10b1-426f-9247-bb680e5fe0c8", "{67e5504410b1426f9247bb680e5fe0c8}"].map(String::from));
    let mut rows = Vec::new();
    for s in &texts {
        let text = format!("{{\"one\":{},\"many\":[]}}", serde_json::to_string(s).unwrap());
        let want = match serde_json::from_str::<uuid::Uuid>(&serde_json::to_string(s).unwrap()) {
            Ok(u) => serde_json::to_string(&Ids { one: u, maybe: None, many: vec![] }).unwrap(),
            Err(_) => "Err".to_string(),
        };
        rows.push(format!("[{},{}]", js(&text), js(&want)));
    }
    // serde_json takes a UUID only as a string.
    rows.push(format!("[{},{}]", js("{\"one\":7,\"many\":[]}"), js("Err")));
    let accepted = rows.iter().filter(|r| !r.ends_with("\"Err\"]")).count();
    // Four forms, and three of them in uppercase: `URN:` is lowercase only.
    assert!(accepted == 7, "{accepted} accepted");
    let rows = rows.join(",");
    for schema in [WireSchema::Zod, WireSchema::Valibot, WireSchema::Arktype] {
        let (import, valid, parse) = match schema {
            WireSchema::Zod => ("", "(s, x) => s.safeParse(x).success", "(s, x) => s.parse(x)"),
            WireSchema::Valibot => ("import * as v from \"valibot\";\n", "(s, x) => v.safeParse(s, x).success", "(s, x) => v.parse(s, x)"),
            WireSchema::Arktype => ("import { type } from \"arktype\";\n", "(s, x) => !(s(x) instanceof type.errors)", "(s, x) => s.assert(x)"),
        };
        let script = format!(
            "{import}import {{ parseJson }} from \"purecrate\";\n\
             import * as w from \"./src/purecrate-wire.ts\";\n\
             const valid = {valid};\nconst parse = {parse};\n\
             const out = [];\n\
             for (const [text, want] of [{rows}]) {{\n\
               const x = parseJson(text);\n\
               const got = valid(w.Ids, x) ? w.toJson.Ids(parse(w.Ids, x)) : \"Err\";\n\
               if (got !== want) out.push(`${{text}}: want ${{want}} got ${{got}}`);\n\
             }}\n\
             console.log(out.length === 0 ? \"ok\" : out.join(\"\\n\"));\n"
        );
        assert_ok(run_node_with(schema, "uuids", Some(shapes::SOURCE), &script));
    }
}
