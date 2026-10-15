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

use crate::support;

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::process::Command;

use purecrate_check::accept;
use purecrate_emit_ts::WireSchema;
use purecrate_pack::assemble_with;
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

/// Writes the zod package for `source`, links the schema library, runs
/// `script` on node, and returns its stdout.
fn run_node(name: &str, source: &str, script: &str) -> Option<String> {
    run_node_with(WireSchema::Zod, name, source, script)
}

fn run_node_with(schema: WireSchema, name: &str, source: &str, script: &str) -> Option<String> {
    if std::env::var_os("PURECRATE_SKIP_NODE").is_some() {
        return None;
    }
    let dir = support::scratch(&format!("wire-write-{name}-{}", schema.runtime_dep()));
    let krate = parse_source(name, source).expect("parse");
    let typed = accept(&krate).unwrap_or_else(|d| panic!("{name} rejected: {d:#?}"));
    support::write_package(&dir, &assemble_with(&typed, Some(schema)));
    // The runtime and adapter are in the package; only the library is not.
    let lib = schema.runtime_dep();
    let package = format!("boundary-{lib}");
    support::link(&dir, lib, &format!("{package}/node_modules/{lib}"));
    if schema == WireSchema::Arktype {
        support::link(&dir, "@ark", &format!("{package}/node_modules/@ark"));
    }
    support::typecheck(&dir);
    fs::write(dir.join("driver.ts"), script).expect("write driver");
    let output = Command::new("node")
        .arg(format!("--conditions={}", support::SOURCE_CONDITION))
        .arg("driver.ts")
        .current_dir(&dir)
        .output()
        .expect("node");
    assert!(output.status.success(), "node in {}:\n{}", dir.display(), String::from_utf8_lossy(&output.stderr));
    let _ = fs::remove_dir_all(&dir);
    Some(String::from_utf8(output.stdout).expect("utf8"))
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
        0.0,
        -0.0,
        1.0,
        -1.0,
        0.1,
        0.5,
        1.5,
        100.0,
        123.456,
        1e15,
        9.999999999999998e15,
        1e16,
        1.5e16,
        1e17,
        1e21,
        1e22,
        1e-4,
        1.5e-4,
        1e-5,
        1.5e-5,
        1e-6,
        1e-7,
        1.5e-7,
        5e-324,
        f64::MIN_POSITIVE,
        f64::MAX,
        -f64::MAX,
        f64::EPSILON,
        9007199254740993.0,
        1234567890123456.7,
        0.30000000000000004,
        f64::NAN,
        f64::INFINITY,
        f64::NEG_INFINITY,
    ];
    let mut f32s: Vec<f32> = vec![
        0.0,
        -0.0,
        1.0,
        0.1,
        0.2,
        0.3,
        1.5,
        16777216.0,
        16777217.0,
        1e12,
        9.999999e12,
        1e13,
        1.5e13,
        1e14,
        1e-5,
        1e-6,
        1.5e-6,
        1e-7,
        f32::MIN_POSITIVE,
        1e-45,
        f32::MAX,
        -f32::MAX,
        f32::EPSILON,
        3.4028235e38,
        f32::NAN,
        f32::INFINITY,
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
        "import {{ Json }} from \"./src/purecrate-runtime.ts\";\n\
         const f64 = (h) => new Float64Array(new BigUint64Array([BigInt(\"0x\" + h)]).buffer)[0];\n\
         const f32 = (h) => new Float32Array(new Uint32Array([parseInt(h, 16)]).buffer)[0];\n\
         const out = [];\n\
         for (const [h, want] of [{}]) {{ const got = Json.f64(f64(h)); if (got !== want) out.push(`f64 ${{h}}: want ${{want}} got ${{got}}`); }}\n\
         for (const [h, want] of [{}]) {{ const got = Json.f32(f32(h)); if (got !== want) out.push(`f32 ${{h}}: want ${{want}} got ${{got}}`); }}\n\
         console.log(out.length === 0 ? \"ok\" : out.slice(0, 40).join(\"\\n\") + `\\n(${{out.length}} in all)`);\n",
        rows64.join(","),
        rows32.join(",")
    );
    assert_ok(run_node("floats", shapes::SOURCE, &script));
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
        ("Wide", serde_json::to_string(&Wide { a: i128::MIN, b: u128::MAX }).unwrap()),
        ("Wide", serde_json::to_string(&Wide { a: -1, b: 0 }).unwrap()),
        ("Floats", serde_json::to_string(&Floats { x: f32::NAN, y: f64::INFINITY }).unwrap()),
        ("Floats", serde_json::to_string(&Floats { x: 1e-7, y: 1e-7 }).unwrap()),
        ("Tree", serde_json::to_string(&Tree::Leaf).unwrap()),
        ("Sealed", serde_json::to_string(&sealed_of(-1, Some("h".into()))).unwrap()),
        ("Id", serde_json::to_string(&id_of(0)).unwrap()),
        ("Label", serde_json::to_string(&label_of("l".into())).unwrap()),
        (
            "Letters",
            serde_json::to_string(&Letters {
                one: '"',
                maybe: Some('😀'),
                many: vec!['\\', '\n', '\u{7f}', '\u{2028}', '\u{10ffff}'],
            })
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
        (
            "Catalog",
            serde_json::to_string(&Catalog {
                counts: ["😀", "\u{fffd}", "__proto__", "constructor", "", "a\"\n"]
                    .iter()
                    .enumerate()
                    .map(|(i, k)| (k.to_string(), i as u32))
                    .collect(),
                tags: ["z", "😀", "\u{fffd}", "__proto__"].iter().map(|k| k.to_string()).collect(),
                nested: [("b", vec![label_of("x".into())]), ("a", vec![])]
                    .into_iter()
                    .map(|(k, v)| (k.to_string(), v))
                    .collect(),
            })
            .unwrap(),
        ),
        (
            "Catalog",
            serde_json::to_string(&Catalog { counts: BTreeMap::new(), tags: BTreeSet::new(), nested: BTreeMap::new() })
                .unwrap(),
        ),
        ("Blocks", serde_json::to_string(&blocks()).unwrap()),
        (
            "Blocks",
            serde_json::to_string(&Blocks {
                key: [0; 4],
                grid: [[0; 2]; 2],
                names: Some(["😀".into(), "\"\n".into()]),
                rows: vec![[u64::MAX], [0]],
                full: [255; 32],
                none: [],
            })
            .unwrap(),
        ),
        (
            "Holding",
            serde_json::to_string(&Holding {
                labels: Paged { items: vec![label_of("a".into()), label_of("é".into())], total: 2 },
                wrapped: Wrapped(u64::MAX),
                wrapped_labels: Wrapped(vec![label_of("w".into())]),
                nums: Paged { items: vec![0, u64::MAX], total: 9 },
                pick: Choice::One("x".into()),
                other: Choice::Two { b: id_of(7), n: 255 },
                none: Choice::Neither,
                links: Links::Link(-1, Box::new(Links::Link(127, Box::new(Links::End)))),
            })
            .unwrap(),
        ),
    ]
}

fn blocks() -> shapes::Blocks {
    shapes::Blocks {
        key: [1, 2, 3, 255],
        grid: [[i16::MIN, -1], [0, i16::MAX]],
        names: None,
        rows: vec![],
        full: std::array::from_fn(|i| i as u8),
        none: [],
    }
}

/// `shapes::Blocks` with serde's own derive, which the oracle reads with:
/// `fixture!` writes only `Serialize`.
#[derive(serde::Serialize, serde::Deserialize)]
struct BlocksOracle {
    key: [u8; 4],
    grid: [[i16; 2]; 2],
    names: Option<[String; 2]>,
    rows: Vec<[u64; 1]>,
    full: [u8; 32],
    none: [bool; 0],
}

/// Every library's schema reads an array from exactly the JSON serde reads
/// one from: an array of `N` elements, not one more or fewer, nested too.
#[test]
fn arrays_read_at_their_length() {
    let good = serde_json::to_string(&blocks()).unwrap();
    let mut texts = vec![
        good.clone(),
        good.replace("[1,2,3,255]", "[1,2,3]"),
        good.replace("[1,2,3,255]", "[1,2,3,255,0]"),
        good.replace("[1,2,3,255]", "[]"),
        good.replace("[1,2,3,255]", "{\"0\":1,\"1\":2,\"2\":3,\"3\":4}"),
        good.replace("[[-32768,-1],[0,32767]]", "[[-32768,-1]]"),
        good.replace("[[-32768,-1],[0,32767]]", "[[-32768,-1],[0]]"),
        good.replace("[[-32768,-1],[0,32767]]", "[[-32768,-1],[0,32767],[1,1]]"),
        good.replace("\"names\":null", "\"names\":[\"a\"]"),
        good.replace("\"names\":null", "\"names\":[\"a\",\"b\"]"),
        good.replace("\"rows\":[]", "\"rows\":[[1],[]]"),
        good.replace("\"rows\":[]", "\"rows\":[[1],[2]]"),
        good.replace("\"none\":[]", "\"none\":[true]"),
        good.replace(",31]", "]"),
        good.replace(",31]", ",31,32]"),
    ];
    texts.dedup();
    let mut rows = Vec::new();
    for text in &texts {
        let want = match serde_json::from_str::<BlocksOracle>(text) {
            Ok(b) => serde_json::to_string(&b).unwrap(),
            Err(_) => "Err".to_string(),
        };
        rows.push(format!("[{},{}]", js(text), js(&want)));
    }
    let accepted = rows.iter().filter(|r| !r.ends_with("\"Err\"]")).count();
    assert_eq!(accepted, 3, "{rows:#?}");
    let rows = rows.join(",");
    for schema in [WireSchema::Zod, WireSchema::Valibot, WireSchema::Arktype] {
        let script = format!(
            "import * as w from \"./src/purecrate-wire.ts\";\n\
             const out = [];\n\
             for (const [text, want] of [{rows}]) {{\n\
               let got;\n\
               try {{ got = w.toJson.Blocks(w.fromJson.Blocks(text)); }} catch {{ got = \"Err\"; }}\n\
               if (got !== want) out.push(`${{text}}: want ${{want}} got ${{got}}`);\n\
             }}\n\
             console.log(out.length === 0 ? \"ok\" : out.join(\"\\n\"));\n"
        );
        assert_ok(run_node_with(schema, "arrays", shapes::SOURCE, &script));
    }
}

#[test]
fn shapes_read_from_serde_json_are_written_back_byte_for_byte() {
    let values = shape_values();
    // Non-finite floats are written as `null`, which neither serde_json nor
    // the schema reads back: that row is written from the domain value.
    let rows: Vec<String> = values.iter().map(|(ty, text)| format!("[{},{}]", js(ty), js(text))).collect();
    let script = format!(
        "import {{ Int }} from \"./src/purecrate-runtime.ts\";\n\
         import * as w from \"./src/purecrate-wire.ts\";\n\
         const out = [];\n\
         for (const [ty, text] of [{}]) {{\n\
           if (text.includes(\"null}}\") && ty === \"Floats\") {{\n\
             const got = w.toJson.Floats({{ x: Int.f32.of(NaN), y: Int.f64.of(Infinity) }});\n\
             if (got !== text) out.push(`Floats: want ${{text}} got ${{got}}`);\n\
             continue;\n\
           }}\n\
           let got;\n\
           try {{ got = w.toJson[ty](w.fromJson[ty](text)); }} catch (e) {{ got = `threw ${{e}}`; }}\n\
           if (got !== text) out.push(`${{ty}}: want ${{text}}\\n  got ${{got}}`);\n\
         }}\n\
         console.log(out.length === 0 ? \"ok\" : out.join(\"\\n\"));\n",
        rows.join(",")
    );
    // Each library reads the text back (`fromJson`), as serde does.
    for schema in [WireSchema::Zod, WireSchema::Valibot, WireSchema::Arktype] {
        assert_ok(run_node_with(schema, "shapes", shapes::SOURCE, &script));
    }
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
        "import {{ parseJson }} from \"./src/purecrate-runtime.ts\";\n\
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
    assert_ok(run_node("payment", payment::SOURCE, &script));
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
    // `i64` reads a JSON integer only, as serde_json does: not a digit
    // string, not a float (design/04 §3 rule 1).
    row("Amount", "\"50\"".into(), "Err".into());
    row("Amount", "\"49\"".into(), "Err".into());
    row("Amount", "50.5".into(), "Err".into());
    let rows = rows.join(",");
    for schema in [WireSchema::Zod, WireSchema::Valibot, WireSchema::Arktype] {
        let (import, valid, parse) = match schema {
            WireSchema::Zod => ("", "(s, x) => s.safeParse(x).success", "(s, x) => s.parse(x)"),
            WireSchema::Valibot => {
                ("import * as v from \"valibot\";\n", "(s, x) => v.safeParse(s, x).success", "(s, x) => v.parse(s, x)")
            }
            WireSchema::Arktype => (
                "import { type } from \"arktype\";\n",
                "(s, x) => !(s(x) instanceof type.errors)",
                "(s, x) => s.assert(x)",
            ),
        };
        let script = format!(
            "{import}import {{ parseJson }} from \"./src/purecrate-runtime.ts\";\n\
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
        assert_ok(run_node_with(schema, "try-from", payment::SOURCE, &script));
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
    for form in
        [good.hyphenated().to_string(), good.simple().to_string(), good.braced().to_string(), good.urn().to_string()]
    {
        texts.push(form.to_uppercase());
        texts.push(form[1..].to_string());
        texts.push(format!("{form} "));
        texts.push(form.replacen('6', "g", 1));
        texts.push(form.replacen('6', "é", 1));
        texts.push(form);
    }
    texts.extend(
        ["", "URN:UUID:67e55044-10b1-426f-9247-bb680e5fe0c8", "{67e5504410b1426f9247bb680e5fe0c8}"].map(String::from),
    );
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
            WireSchema::Valibot => {
                ("import * as v from \"valibot\";\n", "(s, x) => v.safeParse(s, x).success", "(s, x) => v.parse(s, x)")
            }
            WireSchema::Arktype => (
                "import { type } from \"arktype\";\n",
                "(s, x) => !(s(x) instanceof type.errors)",
                "(s, x) => s.assert(x)",
            ),
        };
        let script = format!(
            "{import}import {{ parseJson }} from \"./src/purecrate-runtime.ts\";\n\
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
        assert_ok(run_node_with(schema, "uuids", shapes::SOURCE, &script));
    }
}

/// A value `try_from` refuses fails with an issue that names the type and,
/// the error being an enum, the variant it returned; zod also carries the
/// error value itself.
#[test]
fn a_refused_try_from_says_what_serde_says() {
    for schema in [WireSchema::Zod, WireSchema::Valibot, WireSchema::Arktype] {
        let (import, message) = match schema {
            WireSchema::Zod => (
                "",
                "(() => { const r = w.Amount.safeParse(49); return r.success ? \"accepted\" : r.error.issues[0].message + \" \" + r.error.issues[0].params.error.kind; })()",
            ),
            WireSchema::Valibot => (
                "import * as v from \"valibot\";\n",
                "(() => { const r = v.safeParse(w.Amount, 49); return r.success ? \"accepted\" : r.issues[0].message; })()",
            ),
            WireSchema::Arktype => (
                "import { type } from \"arktype\";\n",
                "(() => { const r = w.Amount(49); return r instanceof type.errors ? r.summary : \"accepted\"; })()",
            ),
        };
        let script = format!("{import}import * as w from \"./src/purecrate-wire.ts\";\nconsole.log({message});\n");
        // `None` when node is skipped (PURECRATE_SKIP_NODE).
        let Some(out) = run_node_with(schema, "try-from-message", payment::SOURCE, &script) else { return };
        // The text of the error's `impl Display`, which serde reports too.
        let want = match schema {
            WireSchema::Zod => "Amount: amount must be 50 to 99999999 AmountOutOfRange",
            _ => "Amount: amount must be 50 to 99999999",
        };
        assert!(out.contains(want), "{schema:?}: {out}");
    }
}

/// An error keeps its location: a bad field of a struct is reported at the
/// field in every library, and arktype, which tries variants by hand, also
/// reports a bad field inside the variant the object names.
#[test]
fn errors_keep_their_location() {
    let terms = r#"{ amount: 50, capture: "Nope", confirmation: "Manual" }"#;
    let event = r#"{ Confirm: { method: null, outcome: "Bogus" } }"#;
    for schema in [WireSchema::Zod, WireSchema::Valibot, WireSchema::Arktype] {
        let (import, path) = match schema {
            WireSchema::Zod => ("", "(s, x) => { const r = s.safeParse(x); return r.success ? \"accepted\" : r.error.issues[0].path.join(\".\"); }"),
            WireSchema::Valibot => (
                "import * as v from \"valibot\";\n",
                "(s, x) => { const r = v.safeParse(s, x); return r.success ? \"accepted\" : (r.issues[0].path ?? []).map((p) => p.key).join(\".\"); }",
            ),
            WireSchema::Arktype => (
                "import { type } from \"arktype\";\n",
                "(s, x) => { const r = s(x); return r instanceof type.errors ? r[0].path.join(\".\") : \"accepted\"; }",
            ),
        };
        let script = format!(
            "{import}import * as w from \"./src/purecrate-wire.ts\";\n\
             const path = {path};\n\
             console.log(path(w.Terms, {terms}));\n\
             console.log(path(w.Event, {event}));\n"
        );
        let Some(out) = run_node_with(schema, "error-paths", payment::SOURCE, &script) else { return };
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines.first(), Some(&"capture"), "{schema:?}: {out}");
        if schema == WireSchema::Arktype {
            assert_eq!(lines.get(1), Some(&"Confirm.outcome"), "{out}");
        }
    }
}

/// `fromJson.T(text)` reads serde_json's text through `parseJson`, so an
/// `i64` past 2^53 stays exact, and throws on malformed text and on a value
/// the schema refuses, with each library.
#[test]
fn from_json_reads_the_text_exactly_and_throws_on_bad_input() {
    let ints = r#"{"a":-128,"b":32767,"c":-2147483648,"d":-9223372036854775807,"e":255,"f":65535,"g":4294967295,"h":18446744073709551615,"i":9007199254740991}"#;
    let script = format!(
        "import * as w from \"./src/purecrate-wire.ts\";\n\
         const ok = w.fromJson.Ints({ints:?});\n\
         const threw = (text) => {{ try {{ w.fromJson.Ints(text); return \"read\"; }} catch {{ return \"threw\"; }} }};\n\
         console.log(`${{ok.d}} ${{ok.h}} ${{threw('{{\"a\":1')}} ${{threw({bad:?})}}`);\n",
        bad = ints.replace("-128", "-129"),
    );
    for schema in [WireSchema::Zod, WireSchema::Valibot, WireSchema::Arktype] {
        let Some(out) = run_node_with(schema, "from-json", shapes::SOURCE, &script) else { return };
        assert_eq!(out.trim(), "-9223372036854775807 18446744073709551615 threw threw", "{schema:?}");
    }
}

/// `fixtures/btree_keys.rs`'s `Keyed`, with the real derive: what serde_json
/// reads and writes for a map whose key is not a `String` (design/01 §7.26).
mod keyed {
    use std::collections::{BTreeMap, BTreeSet};

    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
    pub struct UserId(pub u64);

    #[derive(Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
    pub struct Code(pub String);

    #[derive(Serialize, Deserialize, Default)]
    pub struct Keyed {
        pub small: BTreeMap<i8, u8>,
        pub bytes: BTreeMap<u8, u8>,
        pub wide: BTreeMap<u64, u8>,
        pub signed: BTreeMap<i64, u8>,
        pub huge: BTreeMap<u128, u8>,
        pub least: BTreeMap<i128, u8>,
        pub count: BTreeMap<usize, u8>,
        pub flags: BTreeMap<bool, u8>,
        pub letters: BTreeMap<char, u8>,
        pub ids: BTreeMap<uuid::Uuid, u8>,
        pub users: BTreeMap<UserId, String>,
        pub codes: BTreeMap<Code, u8>,
        pub tags: BTreeSet<u32>,
        pub chars: BTreeSet<char>,
        pub members: BTreeSet<UserId>,
        pub nested: BTreeMap<i32, BTreeMap<char, Vec<u8>>>,
    }

    pub const FIELDS: [&str; 16] = [
        "small", "bytes", "wide", "signed", "huge", "least", "count", "flags", "letters", "ids", "users", "codes",
        "tags", "chars", "members", "nested",
    ];
}

/// `Keyed`'s JSON with every map and set empty but `field`, which is `json`.
fn keyed_text(field: &str, json: &str) -> String {
    let parts: Vec<String> = keyed::FIELDS
        .iter()
        .map(|f| {
            let empty = if matches!(*f, "tags" | "chars" | "members") { "[]" } else { "{}" };
            format!("\"{f}\":{}", if *f == field { json } else { empty })
        })
        .collect();
    format!("{{{}}}", parts.join(","))
}

/// Map keys other than `String` (design/04 §3.3): every text serde_json
/// 1.0.145 reads for a key of each type is read by each library and
/// written back as serde_json writes the value it read (a `Uuid` in its
/// canonical form, `i128`'s `-0` as `0`, a key given twice with its later
/// value), and every text it refuses is refused.
#[test]
fn keyed_maps_read_and_write_what_serde_json_does() {
    let source = include_str!("../fixtures/btree_keys.rs");
    let mut rich = keyed::Keyed::default();
    rich.small.extend([(10, 1), (9, 2), (-1, 3), (-128, 4), (127, 5)]);
    rich.bytes.extend([(255, 1), (0, 2)]);
    rich.wide.extend([(u64::MAX, 1), (9_007_199_254_740_993, 2), (10, 3), (9, 4)]);
    rich.signed.extend([(i64::MIN, 1), (i64::MAX, 2), (-1, 3)]);
    rich.huge.extend([(u128::MAX, 1), (0, 2), (1 << 64, 3)]);
    rich.least.extend([(i128::MIN, 1), (i128::MAX, 2), (0, 3), (-10, 4)]);
    rich.count.extend([(9_007_199_254_740_991, 1), (10, 2), (9, 3)]);
    rich.flags.extend([(true, 1), (false, 2)]);
    rich.letters.extend([('"', 1), ('\\', 2), ('😀', 3), ('\u{fffd}', 4), ('\u{2028}', 5), ('\0', 6)]);
    rich.ids.extend([(uuid::Uuid::max(), 1), (uuid::Uuid::nil(), 2), (uuid::Uuid::from_u128(9), 3)]);
    rich.users.extend([(keyed::UserId(10), "a".to_string()), (keyed::UserId(9), "é\n".to_string())]);
    rich.codes
        .extend([("b", 1), ("😀", 2), ("\u{fffd}", 3), ("", 4), ("\"q\"", 5)].map(|(k, v)| (keyed::Code(k.into()), v)));
    rich.tags.extend([10, 9, u32::MAX, 0]);
    rich.chars.extend(['😀', '\u{fffd}', 'a']);
    rich.members.extend([keyed::UserId(u64::MAX), keyed::UserId(9), keyed::UserId(10)]);
    rich.nested.insert(10, [('b', vec![1]), ('a', vec![])].into_iter().collect());
    rich.nested.insert(-1, BTreeMap::new());
    let mut texts =
        vec![serde_json::to_string(&rich).unwrap(), serde_json::to_string(&keyed::Keyed::default()).unwrap()];
    let map = |k: &str| format!("{{{}:1}}", serde_json::to_string(k).unwrap());
    let ints = [
        "0", "1", "-1", "01", "+1", " 1", "1 ", "", "-0", "1e2", "1.0", "00", "-01", "0x1", "1_0", "１", "-", "--1",
        "1-",
    ];
    for (field, extra) in [
        ("small", &["127", "-128", "128", "-129"][..]),
        ("bytes", &["255", "256"]),
        ("wide", &["9007199254740993", "18446744073709551615", "18446744073709551616"]),
        ("signed", &["-9223372036854775808", "9223372036854775807", "9223372036854775808"]),
        ("huge", &["340282366920938463463374607431768211455", "340282366920938463463374607431768211456"]),
        (
            "least",
            &[
                "-170141183460469231731687303715884105728",
                "-170141183460469231731687303715884105729",
                "170141183460469231731687303715884105727",
            ],
        ),
        ("count", &["9007199254740991"]),
        ("users", &["18446744073709551615"]),
    ] {
        for k in ints.iter().chain(extra) {
            texts.push(keyed_text(field, &map(k)));
        }
    }
    for k in ["true", "false", "True", "1", " true", "true ", "", "null", "0"] {
        texts.push(keyed_text("flags", &map(k)));
    }
    for k in ["a", "ab", "", "😀", "é", "e\u{301}", "\"", "\\", "\u{10ffff}"] {
        texts.push(keyed_text("letters", &map(k)));
    }
    for k in [
        "6BA7B810-9DAD-11D1-80B4-00C04FD430C8",
        "6ba7b8109dad11d180b400c04fd430c8",
        "{6ba7b810-9dad-11d1-80b4-00c04fd430c8}",
        "urn:uuid:6ba7b810-9dad-11d1-80b4-00c04fd430c8",
        "x",
        "",
    ] {
        texts.push(keyed_text("ids", &map(k)));
    }
    for k in ["a", "", "😀", "10"] {
        texts.push(keyed_text("codes", &map(k)));
    }
    // A key twice, or two texts of one key: the later value, as serde's map.
    texts.push(keyed_text("bytes", r#"{"1":1,"2":2,"1":3}"#));
    let (upper, lower) = ("6BA7B810-9DAD-11D1-80B4-00C04FD430C8", "6ba7b810-9dad-11d1-80b4-00c04fd430c8");
    texts.push(keyed_text("ids", &format!(r#"{{"{upper}":1,"{lower}":2}}"#)));
    texts.push(keyed_text("ids", &format!(r#"{{"{lower}":2,"{upper}":1}}"#)));
    texts.push(keyed_text("least", r#"{"0":1,"-0":2}"#));
    // A map's value refused under a good key; a set of each key's values.
    texts.push(keyed_text("bytes", r#"{"1":256}"#));
    for json in ["[3,1,3]", "[]", r#"["1"]"#, "[4294967296]", "[-1]", "[1.0]", "{}"] {
        texts.push(keyed_text("tags", json));
    }
    for json in [r#"["b","a","b"]"#, r#"["ab"]"#, "[1]"] {
        texts.push(keyed_text("chars", json));
    }
    for json in ["[2,1,2]", r#"["1"]"#, "[18446744073709551616]"] {
        texts.push(keyed_text("members", json));
    }
    for json in [r#"{"10":{"b":[1],"a":[]},"9":{},"-1":{"😀":[255]}}"#, r#"{"1":{"ab":[]}}"#, r#"{"1":[]}"#] {
        texts.push(keyed_text("nested", json));
    }
    let mut refused = 0;
    let rows: Vec<String> = texts
        .iter()
        .map(|text| {
            let want = match serde_json::from_str::<keyed::Keyed>(text) {
                Ok(v) => serde_json::to_string(&v).unwrap(),
                Err(_) => {
                    refused += 1;
                    "refused".to_string()
                }
            };
            format!("[{},{}]", js(text), js(&want))
        })
        .collect();
    assert!(refused > 40 && refused + 40 < texts.len(), "{refused} of {} refused", texts.len());
    let script = format!(
        "import * as w from \"./src/purecrate-wire.ts\";\n\
         const out = [];\n\
         for (const [text, want] of [{}]) {{\n\
           let got;\n\
           try {{ got = w.toJson.Keyed(w.fromJson.Keyed(text)); }} catch {{ got = \"refused\"; }}\n\
           if (got !== want) out.push(`${{text}}\\n  want ${{want}}\\n  got  ${{got}}`);\n\
         }}\n\
         console.log(out.length === 0 ? \"ok\" : out.join(\"\\n\"));\n",
        rows.join(",")
    );
    for schema in [WireSchema::Zod, WireSchema::Valibot, WireSchema::Arktype] {
        assert_ok(run_node_with(schema, "keyed", source, &script));
    }
}
