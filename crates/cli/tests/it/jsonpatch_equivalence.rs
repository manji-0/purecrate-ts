//! `examples/jsonpatch`, RFC 8259 text, RFC 6901 JSON Pointer and RFC 6902
//! JSON Patch on integers and ordered members:
//!
//! - RFC 6902 Appendix A.1 to A.16 give the published result (compared by
//!   `json_eq`) or fail where the RFC says they fail; A.13, whose operation
//!   repeats `op`, is refused as a duplicate key, as the model's header
//!   says. RFC 6901 §5's twelve pointers, read from their JSON string form,
//!   name the published values in its example document;
//! - the same rules in idiomatic Rust (`idiomatic`, the line count design/07
//!   §2 compares against: `serde_json::Value`, `pointer_mut`, `Vec::insert`,
//!   `split`, `replace`) agree on every single operation built from a list
//!   of pointers and values over a set of documents, on every two-operation
//!   patch over a smaller set, on malformed patches, on every pointer over
//!   every document, and on reading and writing a list of JSON texts,
//!   malformed ones included;
//! - the generated package agrees with Rust on a sample of each.

use serde_json::Value;

use crate::support;

purecrate_canon::fixture!(mod jsonpatch = "../../../examples/jsonpatch/src/lib.rs");

use jsonpatch::{Json, JsonError, OpFailure, PatchError, Pointer};

/// RFC 6902 over RFC 6901 as one would write it with serde_json, under the
/// model's scope (integers in `i64`, unique keys, `-` only for `add`, the
/// root as a target). Not converted; the reference only.
mod idiomatic {
    use serde_core::de::{self, Deserialize, Deserializer, MapAccess, SeqAccess, Visitor};
    use serde_json::{Map, Value};
    use std::fmt;

    /// What is compared with the model: where the patch went wrong, not
    /// why in detail. A parse error is one class per text (no position or
    /// kind); a malformed patch is the index of the operation (no field);
    /// an operation that fails is its index and one of four kinds, every
    /// way of not finding a target (missing member, index out of range or
    /// malformed, `-`, not a container) being one.
    #[derive(Debug, PartialEq)]
    pub enum Error {
        Document,
        Patch,
        BadPatch(Option<usize>),
        Failed(usize, Failure),
    }

    #[derive(Debug, PartialEq)]
    pub enum Failure {
        Missing,
        IntoOwnChild,
        RemoveRoot,
        TestFailed,
    }

    /// A JSON value as the model reads it. serde_json keeps the last of
    /// repeated keys and takes any number, so this visitor refuses both.
    /// serde_json reads `-0` as the float `-0.0`, so `-0.0` and `-0e0`
    /// pass here as `0` too; the shared inputs leave those two out.
    struct Strict(Value);

    impl<'de> Deserialize<'de> for Strict {
        fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
            d.deserialize_any(StrictVisitor).map(Strict)
        }
    }

    struct StrictVisitor;

    impl<'de> Visitor<'de> for StrictVisitor {
        type Value = Value;

        fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
            f.write_str("JSON with i64 numbers and unique keys")
        }

        fn visit_unit<E>(self) -> Result<Value, E> {
            Ok(Value::Null)
        }

        fn visit_bool<E>(self, b: bool) -> Result<Value, E> {
            Ok(b.into())
        }

        fn visit_i64<E>(self, n: i64) -> Result<Value, E> {
            Ok(n.into())
        }

        fn visit_u64<E: de::Error>(self, n: u64) -> Result<Value, E> {
            i64::try_from(n).map(Value::from).map_err(|_| E::custom("out of range"))
        }

        fn visit_f64<E: de::Error>(self, x: f64) -> Result<Value, E> {
            if x == 0.0 && x.is_sign_negative() {
                Ok(0.into())
            } else {
                Err(E::custom("not an integer"))
            }
        }

        fn visit_str<E>(self, s: &str) -> Result<Value, E> {
            Ok(s.into())
        }

        fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Value, A::Error> {
            let mut items = Vec::new();
            while let Some(Strict(item)) = seq.next_element()? {
                items.push(item);
            }
            Ok(Value::Array(items))
        }

        fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Value, A::Error> {
            let mut members = Map::new();
            while let Some((key, Strict(value))) = map.next_entry::<String, Strict>()? {
                if members.insert(key, value).is_some() {
                    return Err(de::Error::custom("duplicate key"));
                }
            }
            Ok(Value::Object(members))
        }
    }

    pub fn parse(text: &str) -> Option<Value> {
        serde_json::from_str::<Strict>(text).ok().map(|s| s.0)
    }

    /// Every `~` starts `~0` or `~1` (RFC 6901 §3).
    fn valid_pointer(p: &str) -> bool {
        (p.is_empty() || p.starts_with('/')) && p.split('~').skip(1).all(|s| s.starts_with(['0', '1']))
    }

    pub fn get<'a>(doc: &'a Value, pointer: &str) -> Option<&'a Value> {
        if valid_pointer(pointer) {
            doc.pointer(pointer)
        } else {
            None
        }
    }

    /// Pointers stay text: serde_json's `pointer_mut` reads them.
    enum Op {
        Add(String, Value),
        Remove(String),
        Replace(String, Value),
        Move(String, String),
        Copy(String, String),
        Test(String, Value),
    }

    fn read_op(op: &Value) -> Option<Op> {
        let op = op.as_object()?;
        let pointer = |name: &str| op.get(name)?.as_str().filter(|p| valid_pointer(p)).map(String::from);
        let value = || op.get("value").cloned();
        Some(match op.get("op")?.as_str()? {
            "add" => Op::Add(pointer("path")?, value()?),
            "remove" => Op::Remove(pointer("path")?),
            "replace" => Op::Replace(pointer("path")?, value()?),
            "move" => Op::Move(pointer("from")?, pointer("path")?),
            "copy" => Op::Copy(pointer("from")?, pointer("path")?),
            "test" => Op::Test(pointer("path")?, value()?),
            _ => return None,
        })
    }

    /// The parent's pointer and the last reference token, unescaped.
    fn split(path: &str) -> (&str, String) {
        let (parent, last) = path.rsplit_once('/').expect("not the root");
        (parent, last.replace("~1", "/").replace("~0", "~"))
    }

    /// `0`, or digits without a leading zero.
    fn index(token: &str) -> Option<usize> {
        let digits = !token.is_empty() && token.bytes().all(|b| b.is_ascii_digit());
        if digits && (token == "0" || !token.starts_with('0')) {
            token.parse().ok()
        } else {
            None
        }
    }

    fn add(doc: &mut Value, path: &str, value: Value) -> Result<(), Failure> {
        if path.is_empty() {
            *doc = value;
            return Ok(());
        }
        let (parent, key) = split(path);
        match doc.pointer_mut(parent) {
            Some(Value::Object(members)) => {
                members.insert(key, value);
            }
            Some(Value::Array(items)) => {
                let at = if key == "-" { Some(items.len()) } else { index(&key) };
                let at = at.filter(|&i| i <= items.len()).ok_or(Failure::Missing)?;
                items.insert(at, value);
            }
            _ => return Err(Failure::Missing),
        }
        Ok(())
    }

    fn remove(doc: &mut Value, path: &str) -> Result<Value, Failure> {
        if path.is_empty() {
            return Err(Failure::RemoveRoot);
        }
        let (parent, key) = split(path);
        match doc.pointer_mut(parent) {
            Some(Value::Object(members)) => members.remove(&key).ok_or(Failure::Missing),
            Some(Value::Array(items)) => match index(&key) {
                Some(i) if i < items.len() => Ok(items.remove(i)),
                _ => Err(Failure::Missing),
            },
            _ => Err(Failure::Missing),
        }
    }

    fn apply_op(doc: &mut Value, op: Op) -> Result<(), Failure> {
        match op {
            Op::Add(path, value) => add(doc, &path, value)?,
            Op::Remove(path) => {
                remove(doc, &path)?;
            }
            Op::Replace(path, value) => *doc.pointer_mut(&path).ok_or(Failure::Missing)? = value,
            Op::Move(from, path) if path.starts_with(&format!("{from}/")) => return Err(Failure::IntoOwnChild),
            Op::Move(from, path) if from == path => {
                doc.pointer(&from).ok_or(Failure::Missing)?;
            }
            Op::Move(from, path) => {
                let value = remove(doc, &from)?;
                add(doc, &path, value)?;
            }
            Op::Copy(from, path) => {
                let value = doc.pointer(&from).cloned().ok_or(Failure::Missing)?;
                add(doc, &path, value)?;
            }
            Op::Test(path, value) => {
                if *doc.pointer(&path).ok_or(Failure::Missing)? != value {
                    return Err(Failure::TestFailed);
                }
            }
        }
        Ok(())
    }

    /// Reads every operation before applying any, as the model does.
    pub fn apply(doc: &str, patch: &str) -> Result<Value, Error> {
        let mut doc = parse(doc).ok_or(Error::Document)?;
        let patch = parse(patch).ok_or(Error::Patch)?;
        let ops = patch.as_array().ok_or(Error::BadPatch(None))?;
        let ops: Vec<Op> = ops
            .iter()
            .enumerate()
            .map(|(i, op)| read_op(op).ok_or(Error::BadPatch(Some(i))))
            .collect::<Result<_, _>>()?;
        for (i, op) in ops.into_iter().enumerate() {
            apply_op(&mut doc, op).map_err(|f| Error::Failed(i, f))?;
        }
        Ok(doc)
    }
}

fn json(text: &str) -> Json {
    jsonpatch::read_json(text).unwrap_or_else(|e| panic!("{text}: {e:?}"))
}

/// A model value as serde_json holds it (members sorted by key).
fn value(j: &Json) -> Value {
    match j {
        Json::Null => Value::Null,
        Json::Bool(b) => Value::from(*b),
        Json::Num(n) => Value::from(*n),
        Json::Text(s) => Value::from(s.as_str()),
        Json::Array(items) => Value::Array(items.iter().map(value).collect()),
        Json::Object(members) => Value::Object(members.iter().map(|m| (m.key.clone(), value(&m.value))).collect()),
    }
}

/// The value with every object's members sorted by key, as serde_json
/// writes them.
fn sorted(j: &Json) -> Json {
    match j {
        Json::Array(items) => Json::Array(items.iter().map(sorted).collect()),
        Json::Object(members) => {
            let mut members: Vec<jsonpatch::Member> =
                members.iter().map(|m| jsonpatch::Member { key: m.key.clone(), value: sorted(&m.value) }).collect();
            members.sort_by(|a, b| a.key.cmp(&b.key));
            Json::Object(members)
        }
        other => other.clone(),
    }
}

/// The model's result in the reference's classes (see `idiomatic::Error`).
fn class(result: Result<String, PatchError>) -> Result<Value, idiomatic::Error> {
    use idiomatic::{Error as E, Failure as F};
    match result {
        Ok(text) => Ok(idiomatic::parse(&text).unwrap_or_else(|| panic!("the model wrote {text}"))),
        Err(PatchError::Document(_)) => Err(E::Document),
        Err(PatchError::Patch(_)) => Err(E::Patch),
        Err(PatchError::NotAnArray) => Err(E::BadPatch(None)),
        Err(
            PatchError::OpNotAnObject(index)
            | PatchError::MissingMember { index, .. }
            | PatchError::MemberNotAString { index, .. }
            | PatchError::BadPointer { index, .. }
            | PatchError::UnknownOp(index),
        ) => Err(E::BadPatch(Some(index))),
        Err(PatchError::Failed { index, failure }) => Err(E::Failed(
            index,
            match failure {
                OpFailure::MoveIntoOwnChild => F::IntoOwnChild,
                OpFailure::RemoveRoot => F::RemoveRoot,
                OpFailure::TestFailed => F::TestFailed,
                OpFailure::NotFound
                | OpFailure::NotAContainer
                | OpFailure::InvalidIndex
                | OpFailure::IndexOutOfRange
                | OpFailure::DashNotAllowed => F::Missing,
            },
        )),
    }
}

/// RFC 6902 Appendix A that succeed: name, document, patch, result.
const APPENDIX_OK: [(&str, &str, &str, &str); 10] = [
    (
        "A.1",
        r#"{ "foo": "bar"}"#,
        r#"[{ "op": "add", "path": "/baz", "value": "qux" }]"#,
        r#"{"baz":"qux","foo":"bar"}"#,
    ),
    (
        "A.2",
        r#"{ "foo": [ "bar", "baz" ] }"#,
        r#"[{ "op": "add", "path": "/foo/1", "value": "qux" }]"#,
        r#"{ "foo": [ "bar", "qux", "baz" ] }"#,
    ),
    ("A.3", r#"{"baz":"qux","foo":"bar"}"#, r#"[{ "op": "remove", "path": "/baz" }]"#, r#"{ "foo": "bar" }"#),
    (
        "A.4",
        r#"{ "foo": [ "bar", "qux", "baz" ] }"#,
        r#"[{ "op": "remove", "path": "/foo/1" }]"#,
        r#"{ "foo": [ "bar", "baz" ] }"#,
    ),
    (
        "A.5",
        r#"{"baz":"qux","foo":"bar"}"#,
        r#"[{ "op": "replace", "path": "/baz", "value": "boo" }]"#,
        r#"{"baz":"boo","foo":"bar"}"#,
    ),
    (
        "A.6",
        r#"{"foo":{"bar":"baz","waldo":"fred"},"qux":{"corge":"grault"}}"#,
        r#"[{ "op": "move", "from": "/foo/waldo", "path": "/qux/thud" }]"#,
        r#"{"foo":{"bar":"baz"},"qux":{"corge":"grault","thud":"fred"}}"#,
    ),
    (
        "A.7",
        r#"{ "foo": [ "all", "grass", "cows", "eat" ] }"#,
        r#"[{ "op": "move", "from": "/foo/1", "path": "/foo/3" }]"#,
        r#"{ "foo": [ "all", "cows", "eat", "grass" ] }"#,
    ),
    (
        "A.8",
        r#"{"baz":"qux","foo":["a",2,"c"]}"#,
        r#"[{ "op": "test", "path": "/baz", "value": "qux" }, { "op": "test", "path": "/foo/1", "value": 2 }]"#,
        r#"{"baz":"qux","foo":["a",2,"c"]}"#,
    ),
    (
        "A.10",
        r#"{ "foo": "bar" }"#,
        r#"[{ "op": "add", "path": "/child", "value": { "grandchild": { } } }]"#,
        r#"{"foo":"bar","child":{"grandchild":{}}}"#,
    ),
    (
        "A.11",
        r#"{ "foo": "bar" }"#,
        r#"[{ "op": "add", "path": "/baz", "value": "qux", "xyz": 123 }]"#,
        r#"{"foo":"bar","baz":"qux"}"#,
    ),
];

const A14_DOC: &str = r#"{"/": 9, "~1": 10}"#;

/// The rest of Appendix A: name, document, patch.
const APPENDIX_REST: [(&str, &str, &str); 6] = [
    ("A.9", r#"{ "baz": "qux" }"#, r#"[{ "op": "test", "path": "/baz", "value": "bar" }]"#),
    ("A.12", r#"{ "foo": "bar" }"#, r#"[{ "op": "add", "path": "/baz/bat", "value": "qux" }]"#),
    ("A.13", r#"{ "foo": "bar" }"#, r#"[{ "op": "add", "path": "/baz", "value": "qux", "op": "remove" }]"#),
    ("A.14", A14_DOC, r#"[{"op": "test", "path": "/~01", "value": 10}]"#),
    ("A.15", A14_DOC, r#"[{"op": "test", "path": "/~01", "value": "10"}]"#),
    ("A.16", r#"{ "foo": ["bar"] }"#, r#"[{ "op": "add", "path": "/foo/-", "value": ["abc", "def"] }]"#),
];

/// RFC 6901 §5: the document, and each pointer's JSON string form with
/// the value it names.
const POINTER_DOC: &str = r#"{
    "foo": ["bar", "baz"],
    "": 0,
    "a/b": 1,
    "c%d": 2,
    "e^f": 3,
    "g|h": 4,
    "i\\j": 5,
    "k\"l": 6,
    " ": 7,
    "m~n": 8
}"#;

const POINTERS_6901: [(&str, &str); 12] = [
    (r#""""#, POINTER_DOC),
    (r#""/foo""#, r#"["bar", "baz"]"#),
    (r#""/foo/0""#, r#""bar""#),
    (r#""/""#, "0"),
    (r#""/a~1b""#, "1"),
    (r#""/c%d""#, "2"),
    (r#""/e^f""#, "3"),
    (r#""/g|h""#, "4"),
    (r#""/i\\j""#, "5"),
    (r#""/k\"l""#, "6"),
    (r#""/ ""#, "7"),
    (r#""/m~0n""#, "8"),
];

fn failed(index: usize, failure: OpFailure) -> Result<String, PatchError> {
    Err(PatchError::Failed { index, failure })
}

#[test]
fn the_rfc_examples_come_out_as_published() {
    for (name, doc, patch, expected) in APPENDIX_OK {
        let out = jsonpatch::apply(doc, patch).unwrap_or_else(|e| panic!("{name}: {e:?}"));
        assert!(jsonpatch::json_eq(&json(&out), &json(expected)), "{name}: {out}");
    }
    let [a9, a12, a13, a14, a15, a16] = APPENDIX_REST.map(|(_, doc, patch)| jsonpatch::apply(doc, patch));
    assert_eq!(a9, failed(0, OpFailure::TestFailed), "A.9");
    assert_eq!(a12, failed(0, OpFailure::NotFound), "A.12");
    assert!(matches!(a13, Err(PatchError::Patch(JsonError::DuplicateKey(_)))), "A.13: {a13:?}");
    assert!(jsonpatch::json_eq(&json(&a14.expect("A.14")), &json(A14_DOC)), "A.14");
    assert_eq!(a15, failed(0, OpFailure::TestFailed), "A.15");
    assert!(jsonpatch::json_eq(&json(&a16.expect("A.16")), &json(r#"{ "foo": ["bar", ["abc", "def"]] }"#)), "A.16");

    let doc = json(POINTER_DOC);
    for (pointer, expected) in POINTERS_6901 {
        let p = Pointer::from_json(pointer).unwrap_or_else(|e| panic!("{pointer}: {e:?}"));
        let got = jsonpatch::get_value(&doc, &p).unwrap_or_else(|e| panic!("{pointer}: {e:?}"));
        assert!(jsonpatch::json_eq(&got, &json(expected)), "{pointer}: {got:?}");
    }

    // The scope the header draws.
    assert_eq!(jsonpatch::read_json("-0"), Ok(Json::Num(0)));
    assert_eq!(jsonpatch::read_json("-0.0"), Err(JsonError::NotAnInteger(0)));
    assert_eq!(jsonpatch::read_json("-0e0"), Err(JsonError::NotAnInteger(0)));
    assert_eq!(jsonpatch::read_json("9223372036854775808"), Err(JsonError::OutOfRange(0)));
    assert_eq!(jsonpatch::read_json(r#"{"a":1,"a":1}"#), Err(JsonError::DuplicateKey(7)));
    assert_eq!(jsonpatch::read_json(r#""\ud800""#), Err(JsonError::LoneSurrogate(1)));
    assert_eq!(jsonpatch::apply("[1]", r#"[{"op":"remove","path":""}]"#), failed(0, OpFailure::RemoveRoot));
    assert_eq!(
        jsonpatch::apply("[1]", r#"[{"op":"replace","path":"/-","value":2}]"#),
        failed(0, OpFailure::DashNotAllowed)
    );
    assert_eq!(jsonpatch::apply("[1]", r#"[{"op":"add","path":"/01","value":2}]"#), failed(0, OpFailure::InvalidIndex));
    assert_eq!(
        jsonpatch::apply(r#"{"a":{}}"#, r#"[{"op":"move","from":"/a","path":"/a/b"}]"#),
        failed(0, OpFailure::MoveIntoOwnChild)
    );
    assert_eq!(jsonpatch::apply(r#"{"a":1}"#, r#"[{"op":"move","from":"","path":""}]"#), Ok(r#"{"a":1}"#.into()));
    assert_eq!(
        jsonpatch::apply(r#"{"a":1}"#, r#"[{"op":"copy","from":"","path":"/b"}]"#),
        Ok(r#"{"a":1,"b":{"a":1}}"#.into())
    );
    assert_eq!(jsonpatch::write_json(&json(r#""\u001F\/é\u007f""#)), "\"\\u001f/é\u{7f}\"");
}

/// Documents for the generated operations: nested containers, empty ones,
/// keys that need escaping or look like indices, strings with every kind
/// of escape, `i64` edges.
const DOCS: [&str; 8] = [
    r#"{"a":{"b":[1,{"c":null}],"":true,"x/y":"s","m~n":-1,"~1":[]},"arr":[0,[],{}],"e":{}}"#,
    r#"[1,"two",[3],{"four":4},[]]"#,
    r#""\"\\\/\b\f\n\r\t\u0000\u001f\u007f\ud83d\ude00é😀""#,
    r#"{"n":[9223372036854775807,-9223372036854775808,0,-0],"s":"\u00e9\ud834\udd1e"}"#,
    "{}",
    "[]",
    "null",
    r#"{"-":[1],"0":{"00":0},"":{"":[]}}"#,
];

const POINTERS: [&str; 46] = [
    "",
    "/",
    "//",
    "/a",
    "/a/b",
    "/a/b/0",
    "/a/b/1",
    "/a/b/1/c",
    "/a/b/1/c/d",
    "/a/b/2",
    "/a/b/-",
    "/a/b/-/0",
    "/a/b/01",
    "/a/b/00",
    "/a/b/+1",
    "/a/b/ 1",
    "/a/b/99999999999999999999999",
    "/a/",
    "/a/x~1y",
    "/a/m~0n",
    "/a/~01",
    "/a/~2",
    "/a/~",
    "/a/x/y",
    "a",
    "/arr/0",
    "/arr/1/0",
    "/arr/2/k",
    "/arr/-",
    "/arr/3",
    "/0",
    "/1",
    "/3/four",
    "/4",
    "/5",
    "/-",
    "/-/0",
    "/0/00",
    "/missing",
    "/missing/deeper",
    "/e/x",
    "/n/1",
    "/n/3",
    "/n/4",
    "/s",
    "/~01",
];

const VALUES: [&str; 7] = ["null", "1", r#""s""#, "[1]", r#"{"k":{}}"#, "-9223372036854775808", r#""😀\u0001""#];

/// A pointer as a JSON string.
fn quoted(p: &str) -> String {
    serde_json::to_string(p).expect("a string")
}

fn op_with(op: &str, path: &str, value: &str) -> String {
    format!(r#"{{"op":"{op}","path":{},"value":{value}}}"#, quoted(path))
}

fn op_from(op: &str, from: &str, path: &str) -> String {
    format!(r#"{{"op":"{op}","from":{},"path":{}}}"#, quoted(from), quoted(path))
}

/// Operations that are not well formed, one way each.
const MALFORMED: [&str; 14] = [
    r#"{"op":"add","path":"/x"}"#,
    r#"{"path":"/x","value":1}"#,
    r#"{"op":"add","path":1,"value":1}"#,
    r#"{"op":7,"path":"/x"}"#,
    r#"{"op":"jump","path":"/x"}"#,
    r#"{"op":"move","path":"/x"}"#,
    r#"{"op":"copy","from":"/~","path":"/x"}"#,
    r#"{"op":"remove","path":"a"}"#,
    r#"{"op":"remove","path":"/~2"}"#,
    r#"{"op":"ADD","path":"/x","value":1}"#,
    r#"{"op":"add","path":"/x","value":1,"op":"remove"}"#,
    "[]",
    r#""add""#,
    "null",
];

/// Every single operation over `POINTERS` and `VALUES`, then the malformed
/// ones and patches that are not arrays of operations.
fn single_ops() -> Vec<String> {
    let mut ops = Vec::new();
    for path in POINTERS {
        ops.push(format!(r#"{{"op":"remove","path":{}}}"#, quoted(path)));
        for v in VALUES {
            for op in ["add", "replace", "test"] {
                ops.push(op_with(op, path, v));
            }
        }
        for from in POINTERS {
            ops.push(op_from("move", from, path));
            ops.push(op_from("copy", from, path));
        }
    }
    let mut patches: Vec<String> =
        ops.iter().chain(MALFORMED.map(String::from).iter()).map(|op| format!("[{op}]")).collect();
    patches.extend(["[]", "{}", "null", "[", r#"[{"op":"test","path":"","value":null}"#].map(String::from));
    patches
}

/// Operations for the two-operation patches: fewer pointers, one value.
fn pair_ops() -> Vec<String> {
    let paths = ["", "/a", "/a/b", "/a/b/0", "/a/b/-", "/arr/0", "/x", "/missing/x", "/0", "/-"];
    let mut ops = Vec::new();
    for p in paths {
        ops.push(op_with("add", p, "[1]"));
        ops.push(format!(r#"{{"op":"remove","path":{}}}"#, quoted(p)));
        ops.push(op_with("replace", p, r#""s""#));
        ops.push(op_with("test", p, "[1]"));
    }
    for (from, path) in
        [("/a/b/0", "/arr/-"), ("/arr", "/a/b/-"), ("/a", "/a/b"), ("", "/x"), ("/arr/0", "/arr/1"), ("/e", "/e")]
    {
        ops.push(op_from("move", from, path));
        ops.push(op_from("copy", from, path));
    }
    ops.extend(MALFORMED[..3].iter().map(|s| s.to_string()));
    ops
}

fn pairs() -> Vec<String> {
    let ops = pair_ops();
    ops.iter().flat_map(|a| ops.iter().map(move |b| format!("[{a},{b}]"))).collect()
}

/// JSON texts to read and write back: well formed and not.
const TEXTS: [&str; 74] = [
    "0",
    "-1",
    "-0",
    "9223372036854775807",
    "-9223372036854775808",
    "true",
    "false",
    "null",
    " [ 1 , 2 ] ",
    " \t\r\n1\n",
    r#"{"a" : { } }"#,
    r#""\u0041\u00e9\u20ac\ud83d\ude00""#,
    r#""\uD834\uDD1E""#,
    r#""\/""#,
    r#""\u001F\u0000\u0008\u000c""#,
    "\"\u{7f}\u{80}é😀\"",
    "[[[[[]]]]]",
    r#"{"":""}"#,
    r#"{"b":1,"a":[{"d":4,"c":3}]}"#,
    r#"{"a":{"b":1},"b":{"b":1}}"#,
    "",
    " ",
    "-",
    "01",
    "-01",
    "1.",
    "1.0",
    "0.0",
    "1e5",
    "1E5",
    "+1",
    "9223372036854775808",
    "-9223372036854775809",
    "18446744073709551616",
    r#""\ud83d""#,
    r#""\ude00""#,
    r#""\ud83d\u0041""#,
    r#""\ud83dx""#,
    r#""\ud83d\""#,
    r#""\u12""#,
    r#""\u12g4""#,
    r#""\uDFFF""#,
    "\"a\tb\"",
    "\"a\u{1}b\"",
    r#""\x""#,
    r#""\'""#,
    r#""abc"#,
    r#""\"#,
    "trueX",
    "tru",
    "nul",
    "True",
    "[1,]",
    "[,1]",
    "[1 2]",
    "[1]]",
    "[",
    r#"{"a":1,}"#,
    r#"{"a":1,"a":2}"#,
    r#"{"a":{"b":1,"b":1}}"#,
    r#"[{"x":[],"x":[]}]"#,
    r#"{"a":1}x"#,
    "{a:1}",
    r#"{"a" 1}"#,
    r#"{"a":}"#,
    r#"{1:1}"#,
    "'a'",
    "NaN",
    "Infinity",
    "-Infinity",
    "\u{feff}1",
    "1 2",
    r#"["\u0000"]"#,
    "{}{}",
];

#[test]
fn constrained_rust_is_the_idiomatic_rules() {
    for text in DOCS.iter().chain(&TEXTS) {
        let model = jsonpatch::read_json(text);
        let reference = idiomatic::parse(text);
        assert_eq!(model.as_ref().ok().map(value), reference, "read {text:?}: {model:?}");
        if let (Ok(m), Some(r)) = (model, reference) {
            let written = jsonpatch::write_json(&m);
            assert_eq!(jsonpatch::read_json(&written), Ok(m.clone()), "round trip {text:?}");
            let canonical = jsonpatch::write_json(&sorted(&m));
            assert_eq!(canonical, serde_json::to_string(&r).expect("writes"), "write {text:?}");
        }
    }

    for doc in DOCS {
        let model_doc = json(doc);
        let reference_doc = idiomatic::parse(doc).expect(doc);
        for p in POINTERS {
            let model = Pointer::parse(p).ok().and_then(|p| jsonpatch::get_value(&model_doc, &p).ok());
            assert_eq!(model.as_ref().map(value).as_ref(), idiomatic::get(&reference_doc, p), "{doc} at {p:?}");
        }
    }

    let singles = single_ops();
    let pairs = pairs();
    let mut applied = 0;
    for (doc, patches) in DOCS.iter().map(|d| (d, &singles)).chain(DOCS[..4].iter().map(|d| (d, &pairs))) {
        for patch in patches {
            let model = class(jsonpatch::apply(doc, patch));
            assert_eq!(model, idiomatic::apply(doc, patch), "{doc} patched with {patch}");
            applied += 1;
        }
    }
    assert!(applied > 40_000, "{applied}");
    assert_eq!(class(jsonpatch::apply("[", "[]")), Err(idiomatic::Error::Document));
}

#[test]
fn jsonpatch_matches_rust() {
    let singles = single_ops();
    let pairs = pairs();
    let cases = support::cases(|cases| {
        for (_, doc, patch, _) in APPENDIX_OK {
            cases.push(case!(jsonpatch::apply(doc, patch)));
        }
        for (_, doc, patch) in APPENDIX_REST {
            cases.push(case!(jsonpatch::apply(doc, patch)));
        }
        let pointer_doc = json(POINTER_DOC);
        for (pointer, _) in POINTERS_6901 {
            let p = Pointer::from_json(pointer).expect("a pointer");
            cases.push(case!(jsonpatch::get_value(&pointer_doc, &p)));
        }
        for text in DOCS.iter().chain(&TEXTS) {
            cases.push(case!(jsonpatch::read_json(text)));
            if let Ok(v) = jsonpatch::read_json(text) {
                cases.push(case!(jsonpatch::write_json(&v)));
            }
        }
        for doc in DOCS {
            let d = json(doc);
            for p in POINTERS.iter().filter_map(|p| Pointer::parse(p).ok()) {
                cases.push(case!(jsonpatch::get_value(&d, &p)));
            }
        }
        // Most generated patches fail on a given document; the sample
        // takes more of those that apply.
        let docs = DOCS.iter().map(|d| (d, &singles, 5, 97)).chain(DOCS[..4].iter().map(|d| (d, &pairs, 5, 61)));
        for (k, (doc, patches, ok_step, failing_step)) in docs.enumerate() {
            let (ok, failing): (Vec<&String>, Vec<&String>) =
                patches.iter().partition(|patch| jsonpatch::apply(doc, patch).is_ok());
            let sample = ok.iter().skip(k % ok_step).step_by(ok_step);
            for patch in sample.chain(failing.iter().skip(k).step_by(failing_step)) {
                cases.push(case!(jsonpatch::apply(doc, patch.as_str())));
            }
        }
    });
    assert!((1000..2500).contains(&cases.len()), "{} cases", cases.len());
    support::assert_equivalent("jsonpatch", jsonpatch::SOURCE, &cases);
}
