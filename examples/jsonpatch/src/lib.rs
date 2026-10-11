// JSON Patch (RFC 6902) over JSON Pointer (RFC 6901) on a small JSON
// model (RFC 8259).
//
// # What the model checks and leaves out
//
// - Numbers: integers in the `i64` range only. A JSON number with a
//   fraction or an exponent (even `1.0` or `1e2`) is a parse error
//   (`JsonError::NotAnInteger`), and an integer outside the `i64` range is
//   a parse error too (`JsonError::OutOfRange`). So `test` compares
//   integers, by value (`-0` equals `0`).
// - Objects: an ordered list of members. Parsing an object that repeats a
//   key is an error (`JsonError::DuplicateKey`), although RFC 8259 only
//   says names SHOULD be unique. Serializing keeps member order; `add` of
//   an existing key replaces its value in place, `add` of a new key appends
//   it at the end. Objects built by hand with repeated keys are not
//   rejected; lookups then see the first one.
// - Strings: every JSON escape, including `\uXXXX` and surrogate pairs. A
//   lone surrogate escape (a high one not followed by a low one, or a low
//   one alone) is an error (`JsonError::LoneSurrogate`), as is an
//   unescaped control character below U+0020. Serialization is compact (no
//   spaces), escapes `"`, `\` and control characters below U+0020 (as
//   `\b \f \n \r \t` where JSON has a short form, `\u00xx` in lowercase hex
//   otherwise), and writes everything else literally, `/` and non-ASCII
//   included.
// - Depth: values are parsed, compared, serialized and patched by
//   recursion, with no depth limit; a deep enough document exhausts the
//   stack of the caller.
// - Pointers: `""` is the whole document, `"/"` is the member named `""`.
//   A reference token is unescaped `~1` to `/` and then `~0` to `~`, which
//   is what a single left-to-right scan gives (`~01` is `~1`); a `~`
//   followed by anything else is an error. An array index is `0` or digits
//   without a leading zero; `-` names the position after the last element
//   and is accepted only as the last token of an `add` path. A pointer can
//   be read from its plain form (`Pointer::parse`) or from its JSON string
//   form (`Pointer::from_json`, e.g. `"/i\\j"`); the URI fragment form is
//   left out.
// - Patches: all six operations, on the root too. Unknown members of an
//   operation object are ignored, a missing required member is an error,
//   and the first failing operation aborts the whole patch: the input is
//   never changed (every function returns a new value) and nothing of the
//   partial result is returned. `move` refuses a `from` that is a proper
//   prefix of `path`; a `move` onto its own location is a no-op. Removing
//   the root is an error (`OpFailure::RemoveRoot`), since no document would
//   remain.

/// A JSON value. Object members keep their order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Json {
    Null,
    Bool(bool),
    Num(i64),
    Text(String),
    Array(Vec<Json>),
    Object(Vec<Member>),
}

/// One `"key": value` member of a JSON object.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Member {
    pub key: String,
    pub value: Json,
}

/// Why a JSON text was refused. Positions are byte offsets into the text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JsonError {
    UnexpectedEnd,
    UnexpectedByte(usize),
    NotAnInteger(usize),
    OutOfRange(usize),
    DuplicateKey(usize),
    InvalidEscape(usize),
    LoneSurrogate(usize),
    ControlCharacter(usize),
    TrailingInput(usize),
}

/// Why a JSON Pointer was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PointerError {
    MissingSlash,
    InvalidEscape,
    NotAJsonString,
}

/// Why one operation could not be applied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpFailure {
    NotFound,
    NotAContainer,
    InvalidIndex,
    IndexOutOfRange,
    DashNotAllowed,
    MoveIntoOwnChild,
    RemoveRoot,
    TestFailed,
}

/// A member of an operation object.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    Op,
    Path,
    From,
    Value,
}

/// Why a patch was refused. `index` is the operation's position in the
/// patch array.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PatchError {
    Document(JsonError),
    Patch(JsonError),
    NotAnArray,
    OpNotAnObject(usize),
    MissingMember { index: usize, field: Field },
    MemberNotAString { index: usize, field: Field },
    BadPointer { index: usize, field: Field, error: PointerError },
    UnknownOp(usize),
    Failed { index: usize, failure: OpFailure },
}

/// A parsed JSON Pointer: its reference tokens, unescaped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pointer {
    tokens: Vec<String>,
}

impl Pointer {
    /// Reads a pointer in its plain form (`""`, `"/a~1b/0"`).
    pub fn parse(text: &str) -> Result<Pointer, PointerError> {
        if text.is_empty() {
            return Ok(Pointer { tokens: Vec::new() });
        }
        let rest = text.strip_prefix("/").ok_or(PointerError::MissingSlash)?;
        let tokens = rest.split('/').map(unescape_token).collect::<Result<Vec<String>, PointerError>>()?;
        Ok(Pointer { tokens })
    }

    /// Reads a pointer from its JSON string form (`"\"/i\\\\j\""`).
    pub fn from_json(text: &str) -> Result<Pointer, PointerError> {
        match read_json(text) {
            Ok(Json::Text(s)) => Pointer::parse(&s),
            Ok(_) => Err(PointerError::NotAJsonString),
            Err(_) => Err(PointerError::NotAJsonString),
        }
    }

    /// The unescaped reference tokens.
    pub fn tokens(&self) -> Vec<String> {
        self.tokens.clone()
    }
}

/// One RFC 6902 operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Op {
    Add { path: Pointer, value: Json },
    Remove { path: Pointer },
    Replace { path: Pointer, value: Json },
    Move { from: Pointer, path: Pointer },
    Copy { from: Pointer, path: Pointer },
    Test { path: Pointer, value: Json },
}

// ---------------------------------------------------------------------------
// Text entry point

/// Applies a JSON Patch text to a JSON document text and returns the
/// patched document, serialized compactly.
pub fn apply(document: &str, patch: &str) -> Result<String, PatchError> {
    let doc = read_json(document).map_err(|e| PatchError::Document(e))?;
    let patch_value = read_json(patch).map_err(|e| PatchError::Patch(e))?;
    let ops = read_patch(&patch_value)?;
    let result = apply_ops(&doc, &ops)?;
    Ok(write_json(&result))
}

// ---------------------------------------------------------------------------
// JSON text

/// Parses one JSON text (RFC 8259) into a value.
pub fn read_json(text: &str) -> Result<Json, JsonError> {
    let (value, next) = parse_value(text, 0)?;
    let end = skip_ws(text.as_bytes(), next);
    if end < text.len() {
        return Err(JsonError::TrailingInput(end));
    }
    Ok(value)
}

fn skip_ws(b: &[u8], start: usize) -> usize {
    let mut i = start;
    while i < b.len() && matches!(b[i], b' ' | b'\t' | b'\n' | b'\r') {
        i += 1;
    }
    i
}

fn word_at(b: &[u8], i: usize, word: &[u8]) -> bool {
    i + word.len() <= b.len() && b[i..i + word.len()] == *word
}

fn parse_value(s: &str, start: usize) -> Result<(Json, usize), JsonError> {
    let b = s.as_bytes();
    let i = skip_ws(b, start);
    if i >= b.len() {
        return Err(JsonError::UnexpectedEnd);
    }
    let c = b[i];
    if c == b'{' {
        return parse_object(s, i + 1);
    }
    if c == b'[' {
        return parse_array(s, i + 1);
    }
    if c == b'"' {
        let (text, next) = parse_string(s, i + 1)?;
        return Ok((Json::Text(text), next));
    }
    if c == b'-' || matches!(c, b'0'..=b'9') {
        return parse_number(s, i);
    }
    if word_at(b, i, b"true") {
        return Ok((Json::Bool(true), i + 4));
    }
    if word_at(b, i, b"false") {
        return Ok((Json::Bool(false), i + 5));
    }
    if word_at(b, i, b"null") {
        return Ok((Json::Null, i + 4));
    }
    Err(JsonError::UnexpectedByte(i))
}

fn parse_number(s: &str, start: usize) -> Result<(Json, usize), JsonError> {
    let b = s.as_bytes();
    let mut i = start;
    if b[i] == b'-' {
        i += 1;
    }
    if i >= b.len() {
        return Err(JsonError::UnexpectedEnd);
    }
    if b[i] == b'0' {
        i += 1;
    } else if matches!(b[i], b'1'..=b'9') {
        while i < b.len() && matches!(b[i], b'0'..=b'9') {
            i += 1;
        }
    } else {
        return Err(JsonError::UnexpectedByte(i));
    }
    if i < b.len() && (b[i] == b'.' || b[i] == b'e' || b[i] == b'E') {
        return Err(JsonError::NotAnInteger(start));
    }
    let digits = &s[start..i];
    match digits.parse::<i64>() {
        Ok(n) => Ok((Json::Num(n), i)),
        Err(_) => Err(JsonError::OutOfRange(start)),
    }
}

fn short_escape(e: u8) -> Option<char> {
    match e {
        b'"' => Some('"'),
        b'\\' => Some('\\'),
        b'/' => Some('/'),
        b'b' => Some('\u{8}'),
        b'f' => Some('\u{c}'),
        b'n' => Some('\n'),
        b'r' => Some('\r'),
        b't' => Some('\t'),
        _ => None,
    }
}

fn hex4(b: &[u8], i: usize) -> Result<u32, JsonError> {
    if i + 4 > b.len() {
        return Err(JsonError::UnexpectedEnd);
    }
    let mut n: u32 = 0;
    for k in 0..4usize {
        match char::from(b[i + k]).to_digit(16) {
            Some(d) => {
                n = n * 16 + d;
            }
            None => {
                return Err(JsonError::InvalidEscape(i + k));
            }
        }
    }
    Ok(n)
}

/// `b[i]` is the `\` of a `\u` escape. Returns the character and the
/// position after the escape (after both halves of a surrogate pair).
fn unicode_escape(b: &[u8], i: usize) -> Result<(char, usize), JsonError> {
    let high = hex4(b, i + 2)?;
    if high >= 0xDC00 && high <= 0xDFFF {
        return Err(JsonError::LoneSurrogate(i));
    }
    if high >= 0xD800 && high <= 0xDBFF {
        let j = i + 6;
        if j + 1 < b.len() && b[j] == b'\\' && b[j + 1] == b'u' {
            let low = hex4(b, j + 2)?;
            if low >= 0xDC00 && low <= 0xDFFF {
                let n = 0x10000 + ((high - 0xD800) << 10) + (low - 0xDC00);
                if let Some(ch) = char::from_u32(n) {
                    return Ok((ch, j + 6));
                }
            }
        }
        return Err(JsonError::LoneSurrogate(i));
    }
    match char::from_u32(high) {
        Some(ch) => Ok((ch, i + 6)),
        None => Err(JsonError::LoneSurrogate(i)),
    }
}

/// `start` is just after the opening quote. Returns the string and the
/// position after the closing quote.
fn parse_string(s: &str, start: usize) -> Result<(String, usize), JsonError> {
    let b = s.as_bytes();
    let mut out = String::new();
    let mut i = start;
    let mut run = start;
    while i < b.len() {
        let c = b[i];
        if c == b'"' {
            out.push_str(&s[run..i]);
            return Ok((out, i + 1));
        }
        if c < 0x20 {
            return Err(JsonError::ControlCharacter(i));
        }
        if c == b'\\' {
            out.push_str(&s[run..i]);
            if i + 1 >= b.len() {
                return Err(JsonError::UnexpectedEnd);
            }
            let e = b[i + 1];
            if e == b'u' {
                let (ch, next) = unicode_escape(b, i)?;
                out.push(ch);
                i = next;
            } else {
                match short_escape(e) {
                    Some(ch) => {
                        out.push(ch);
                    }
                    None => {
                        return Err(JsonError::InvalidEscape(i));
                    }
                }
                i += 2;
            }
            run = i;
        } else {
            i += 1;
        }
    }
    Err(JsonError::UnexpectedEnd)
}

/// `start` is just after `[`.
fn parse_array(s: &str, start: usize) -> Result<(Json, usize), JsonError> {
    let b = s.as_bytes();
    let mut items: Vec<Json> = Vec::new();
    let mut i = skip_ws(b, start);
    if i < b.len() && b[i] == b']' {
        return Ok((Json::Array(items), i + 1));
    }
    while i < b.len() {
        let (value, next) = parse_value(s, i)?;
        items.push(value);
        let j = skip_ws(b, next);
        if j >= b.len() {
            return Err(JsonError::UnexpectedEnd);
        }
        if b[j] == b']' {
            return Ok((Json::Array(items), j + 1));
        }
        if b[j] != b',' {
            return Err(JsonError::UnexpectedByte(j));
        }
        i = j + 1;
    }
    Err(JsonError::UnexpectedEnd)
}

/// `start` is just after `{`.
fn parse_object(s: &str, start: usize) -> Result<(Json, usize), JsonError> {
    let b = s.as_bytes();
    let mut members: Vec<Member> = Vec::new();
    let mut i = skip_ws(b, start);
    if i < b.len() && b[i] == b'}' {
        return Ok((Json::Object(members), i + 1));
    }
    while i < b.len() {
        let key_at = skip_ws(b, i);
        if key_at >= b.len() {
            return Err(JsonError::UnexpectedEnd);
        }
        if b[key_at] != b'"' {
            return Err(JsonError::UnexpectedByte(key_at));
        }
        let (key, after_key) = parse_string(s, key_at + 1)?;
        if find_key(&members, &key).is_some() {
            return Err(JsonError::DuplicateKey(key_at));
        }
        let colon = skip_ws(b, after_key);
        if colon >= b.len() {
            return Err(JsonError::UnexpectedEnd);
        }
        if b[colon] != b':' {
            return Err(JsonError::UnexpectedByte(colon));
        }
        let (value, next) = parse_value(s, colon + 1)?;
        members.push(Member { key, value });
        let j = skip_ws(b, next);
        if j >= b.len() {
            return Err(JsonError::UnexpectedEnd);
        }
        if b[j] == b'}' {
            return Ok((Json::Object(members), j + 1));
        }
        if b[j] != b',' {
            return Err(JsonError::UnexpectedByte(j));
        }
        i = j + 1;
    }
    Err(JsonError::UnexpectedEnd)
}

/// Serializes a value compactly.
pub fn write_json(value: &Json) -> String {
    match value {
        Json::Null => String::from("null"),
        Json::Bool(true) => String::from("true"),
        Json::Bool(false) => String::from("false"),
        Json::Num(n) => int_text(*n),
        Json::Text(s) => quote(s),
        Json::Array(items) => write_array(items),
        Json::Object(members) => write_object(members),
    }
}

fn write_array(items: &Vec<Json>) -> String {
    let mut out = String::new();
    out.push('[');
    for (k, item) in items.iter().enumerate() {
        if k > 0 {
            out.push(',');
        }
        out.push_str(&write_json(item));
    }
    out.push(']');
    out
}

fn write_object(members: &Vec<Member>) -> String {
    let mut out = String::new();
    out.push('{');
    for (k, m) in members.iter().enumerate() {
        if k > 0 {
            out.push(',');
        }
        out.push_str(&quote(&m.key));
        out.push(':');
        out.push_str(&write_json(&m.value));
    }
    out.push('}');
    out
}

const HEX_DIGITS: &[u8] = b"0123456789abcdef";

/// The digit of `d`'s last decimal place (`d` may be negative).
fn last_digit(d: i64) -> char {
    char::from(HEX_DIGITS[(d % 10).abs() as usize])
}

fn int_text(n: i64) -> String {
    let mut digits: Vec<char> = vec![last_digit(n)];
    let mut rest = n / 10;
    while rest != 0 {
        digits.insert(0, last_digit(rest));
        rest /= 10;
    }
    if n < 0 {
        digits.insert(0, '-');
    }
    digits.iter().collect::<String>()
}

fn quote(s: &str) -> String {
    let mut out = String::new();
    out.push('"');
    for c in s.chars() {
        let n = u32::from(c);
        if c == '"' {
            out.push_str("\\\"");
        } else if c == '\\' {
            out.push_str("\\\\");
        } else if n == 8 {
            out.push_str("\\b");
        } else if n == 12 {
            out.push_str("\\f");
        } else if n == 10 {
            out.push_str("\\n");
        } else if n == 13 {
            out.push_str("\\r");
        } else if n == 9 {
            out.push_str("\\t");
        } else if n < 0x20 {
            out.push_str("\\u00");
            out.push(char::from(HEX_DIGITS[(n >> 4) as usize]));
            out.push(char::from(HEX_DIGITS[(n & 15) as usize]));
        } else {
            out.push(c);
        }
    }
    out.push('"');
    out
}

/// JSON equality (RFC 6902 §4.6): objects ignore member order, arrays are
/// ordered, numbers compare by value. `null`, booleans, numbers and strings
/// compare as the derived `==`; arrays and objects recurse through `json_eq`.
pub fn json_eq(a: &Json, b: &Json) -> bool {
    match (a, b) {
        (Json::Array(xs), Json::Array(ys)) => arrays_eq(xs, ys),
        (Json::Object(xs), Json::Object(ys)) => objects_eq(xs, ys),
        _ => a == b,
    }
}

fn arrays_eq(xs: &Vec<Json>, ys: &Vec<Json>) -> bool {
    if xs.len() != ys.len() {
        return false;
    }
    for i in 0..xs.len() {
        if !json_eq(&xs[i], &ys[i]) {
            return false;
        }
    }
    true
}

fn objects_eq(xs: &Vec<Member>, ys: &Vec<Member>) -> bool {
    xs.len() == ys.len()
        && xs.iter().all(|m| match find_key(ys, &m.key) {
            Some(j) => json_eq(&m.value, &ys[j].value),
            None => false,
        })
}

// ---------------------------------------------------------------------------
// JSON Pointer

fn unescape_token(piece: &str) -> Result<String, PointerError> {
    let b = piece.as_bytes();
    let mut out = String::new();
    let mut run: usize = 0;
    let mut i: usize = 0;
    while i < b.len() {
        if b[i] == b'~' {
            out.push_str(&piece[run..i]);
            if i + 1 < b.len() && b[i + 1] == b'1' {
                out.push('/');
            } else if i + 1 < b.len() && b[i + 1] == b'0' {
                out.push('~');
            } else {
                return Err(PointerError::InvalidEscape);
            }
            i += 2;
            run = i;
        } else {
            i += 1;
        }
    }
    out.push_str(&piece[run..b.len()]);
    Ok(out)
}

fn find_key(members: &Vec<Member>, key: &str) -> Option<usize> {
    members.iter().position(|m| m.key == key)
}

/// An array index token: `0`, or digits without a leading zero. `-` is
/// refused here; `add` handles it before asking.
fn index_of(token: &str) -> Result<usize, OpFailure> {
    if token == "-" {
        return Err(OpFailure::DashNotAllowed);
    }
    let b = token.as_bytes();
    if b.is_empty() {
        return Err(OpFailure::InvalidIndex);
    }
    if b.len() > 1 && b[0] == b'0' {
        return Err(OpFailure::InvalidIndex);
    }
    for c in token.bytes() {
        if !matches!(c, b'0'..=b'9') {
            return Err(OpFailure::InvalidIndex);
        }
    }
    token.parse::<usize>().map_err(|_| OpFailure::IndexOutOfRange)
}

fn child_of(doc: &Json, token: &str) -> Result<Json, OpFailure> {
    match doc {
        Json::Object(members) => member_at(members, token),
        Json::Array(items) => item_at(items, token),
        _ => Err(OpFailure::NotAContainer),
    }
}

fn member_at(members: &Vec<Member>, token: &str) -> Result<Json, OpFailure> {
    match find_key(members, token) {
        Some(i) => Ok(members[i].value.clone()),
        None => Err(OpFailure::NotFound),
    }
}

/// The index `token` names in a list of `len` items, which must hold it.
fn existing_index(len: usize, token: &str) -> Result<usize, OpFailure> {
    let i = index_of(token)?;
    if i >= len {
        return Err(OpFailure::IndexOutOfRange);
    }
    Ok(i)
}

fn item_at(items: &Vec<Json>, token: &str) -> Result<Json, OpFailure> {
    let i = existing_index(items.len(), token)?;
    Ok(items[i].clone())
}

/// Evaluates a pointer against a document (RFC 6901 §4) and returns a copy
/// of the value it names.
pub fn get_value(doc: &Json, pointer: &Pointer) -> Result<Json, OpFailure> {
    let mut current = doc.clone();
    for token in &pointer.tokens {
        current = child_of(&current, token)?;
    }
    Ok(current)
}

// ---------------------------------------------------------------------------
// Editing

enum Edit {
    Add(Json),
    Remove,
    Replace(Json),
}

/// No `&mut`: an edit rebuilds each container on the path, from the last
/// token back up to the root.
fn edit_at(doc: &Json, tokens: &Vec<String>, k: usize, edit: &Edit) -> Result<Json, OpFailure> {
    if k + 1 == tokens.len() {
        return edit_last(doc, &tokens[k], edit);
    }
    let child = child_of(doc, &tokens[k])?;
    let new_child = edit_at(&child, tokens, k + 1, edit)?;
    edit_last(doc, &tokens[k], &Edit::Replace(new_child))
}

fn edit_last(doc: &Json, token: &str, edit: &Edit) -> Result<Json, OpFailure> {
    match doc {
        Json::Object(members) => edit_member(members, token, edit),
        Json::Array(items) => edit_item(items, token, edit),
        _ => Err(OpFailure::NotAContainer),
    }
}

/// `add` of an existing key replaces its value in place; of a new key,
/// appends it.
fn edit_member(members: &Vec<Member>, token: &str, edit: &Edit) -> Result<Json, OpFailure> {
    let mut out = members.clone();
    match (edit, find_key(members, token)) {
        (Edit::Add(v), Some(at)) => {
            out[at] = Member { key: String::from(token), value: v.clone() };
        }
        (Edit::Replace(v), Some(at)) => {
            out[at] = Member { key: String::from(token), value: v.clone() };
        }
        (Edit::Remove, Some(at)) => {
            out.remove(at);
        }
        (Edit::Add(v), None) => {
            out.push(Member { key: String::from(token), value: v.clone() });
        }
        (_, None) => {
            return Err(OpFailure::NotFound);
        }
    }
    Ok(Json::Object(out))
}

fn edit_item(items: &Vec<Json>, token: &str, edit: &Edit) -> Result<Json, OpFailure> {
    let mut out = items.clone();
    match edit {
        Edit::Add(v) => {
            let mut at = items.len();
            if token != "-" {
                at = index_of(token)?;
                if at > items.len() {
                    return Err(OpFailure::IndexOutOfRange);
                }
            }
            out.insert(at, v.clone());
        }
        Edit::Replace(v) => {
            let at = existing_index(items.len(), token)?;
            out[at] = v.clone();
        }
        Edit::Remove => {
            let at = existing_index(items.len(), token)?;
            out.remove(at);
        }
    }
    Ok(Json::Array(out))
}

// ---------------------------------------------------------------------------
// Operations

fn add_value(doc: &Json, path: &Pointer, value: &Json) -> Result<Json, OpFailure> {
    if path.tokens.is_empty() {
        return Ok(value.clone());
    }
    edit_at(doc, &path.tokens, 0, &Edit::Add(value.clone()))
}

fn remove_value(doc: &Json, path: &Pointer) -> Result<Json, OpFailure> {
    if path.tokens.is_empty() {
        return Err(OpFailure::RemoveRoot);
    }
    edit_at(doc, &path.tokens, 0, &Edit::Remove)
}

fn replace_value(doc: &Json, path: &Pointer, value: &Json) -> Result<Json, OpFailure> {
    if path.tokens.is_empty() {
        return Ok(value.clone());
    }
    edit_at(doc, &path.tokens, 0, &Edit::Replace(value.clone()))
}

/// `a` is a proper prefix of `b`: shorter, and equal on every token it has.
fn proper_prefix(a: &Vec<String>, b: &Vec<String>) -> bool {
    a.len() < b.len() && a[..] == b[..a.len()]
}

fn move_value(doc: &Json, from: &Pointer, path: &Pointer) -> Result<Json, OpFailure> {
    if proper_prefix(&from.tokens, &path.tokens) {
        return Err(OpFailure::MoveIntoOwnChild);
    }
    let value = get_value(doc, from)?;
    if from.tokens == path.tokens {
        return Ok(doc.clone());
    }
    let removed = remove_value(doc, from)?;
    add_value(&removed, path, &value)
}

fn copy_value(doc: &Json, from: &Pointer, path: &Pointer) -> Result<Json, OpFailure> {
    let value = get_value(doc, from)?;
    add_value(doc, path, &value)
}

fn test_value(doc: &Json, path: &Pointer, value: &Json) -> Result<Json, OpFailure> {
    let actual = get_value(doc, path)?;
    if json_eq(&actual, value) {
        Ok(doc.clone())
    } else {
        Err(OpFailure::TestFailed)
    }
}

/// Applies one operation and returns the new document.
pub fn apply_op(doc: &Json, op: &Op) -> Result<Json, OpFailure> {
    match op {
        Op::Add { path, value } => add_value(doc, path, value),
        Op::Remove { path } => remove_value(doc, path),
        Op::Replace { path, value } => replace_value(doc, path, value),
        Op::Move { from, path } => move_value(doc, from, path),
        Op::Copy { from, path } => copy_value(doc, from, path),
        Op::Test { path, value } => test_value(doc, path, value),
    }
}

/// Applies operations in order; the first failure aborts the patch.
pub fn apply_ops(doc: &Json, ops: &Vec<Op>) -> Result<Json, PatchError> {
    let mut current = doc.clone();
    for (index, op) in ops.iter().enumerate() {
        match apply_op(&current, op) {
            Ok(next) => {
                current = next;
            }
            Err(failure) => {
                return Err(PatchError::Failed { index, failure });
            }
        }
    }
    Ok(current)
}

// ---------------------------------------------------------------------------
// Reading a patch document

/// Reads a patch: a JSON array of operation objects.
pub fn read_patch(value: &Json) -> Result<Vec<Op>, PatchError> {
    match value {
        Json::Array(items) => read_ops(items),
        _ => Err(PatchError::NotAnArray),
    }
}

fn read_ops(items: &Vec<Json>) -> Result<Vec<Op>, PatchError> {
    let mut ops: Vec<Op> = Vec::new();
    for (index, item) in items.iter().enumerate() {
        let op = read_op(index, item)?;
        ops.push(op);
    }
    Ok(ops)
}

fn read_op(index: usize, value: &Json) -> Result<Op, PatchError> {
    match value {
        Json::Object(members) => read_members(index, members),
        _ => Err(PatchError::OpNotAnObject(index)),
    }
}

fn member_value(index: usize, members: &Vec<Member>, key: &str, field: Field) -> Result<Json, PatchError> {
    match find_key(members, key) {
        Some(i) => Ok(members[i].value.clone()),
        None => Err(PatchError::MissingMember { index, field }),
    }
}

fn member_text(index: usize, members: &Vec<Member>, key: &str, field: Field) -> Result<String, PatchError> {
    let value = member_value(index, members, key, field)?;
    match value {
        Json::Text(s) => Ok(s),
        _ => Err(PatchError::MemberNotAString { index, field }),
    }
}

fn member_pointer(index: usize, members: &Vec<Member>, key: &str, field: Field) -> Result<Pointer, PatchError> {
    let text = member_text(index, members, key, field)?;
    match Pointer::parse(&text) {
        Ok(p) => Ok(p),
        Err(error) => Err(PatchError::BadPointer { index, field, error }),
    }
}

fn read_members(index: usize, members: &Vec<Member>) -> Result<Op, PatchError> {
    let name = member_text(index, members, "op", Field::Op)?;
    if name == "add" {
        let path = member_pointer(index, members, "path", Field::Path)?;
        let value = member_value(index, members, "value", Field::Value)?;
        return Ok(Op::Add { path, value });
    }
    if name == "remove" {
        let path = member_pointer(index, members, "path", Field::Path)?;
        return Ok(Op::Remove { path });
    }
    if name == "replace" {
        let path = member_pointer(index, members, "path", Field::Path)?;
        let value = member_value(index, members, "value", Field::Value)?;
        return Ok(Op::Replace { path, value });
    }
    if name == "move" {
        let from = member_pointer(index, members, "from", Field::From)?;
        let path = member_pointer(index, members, "path", Field::Path)?;
        return Ok(Op::Move { from, path });
    }
    if name == "copy" {
        let from = member_pointer(index, members, "from", Field::From)?;
        let path = member_pointer(index, members, "path", Field::Path)?;
        return Ok(Op::Copy { from, path });
    }
    if name == "test" {
        let path = member_pointer(index, members, "path", Field::Path)?;
        let value = member_value(index, members, "value", Field::Value)?;
        return Ok(Op::Test { path, value });
    }
    Err(PatchError::UnknownOp(index))
}
