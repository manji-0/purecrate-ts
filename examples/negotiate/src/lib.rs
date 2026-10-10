// Proactive content negotiation (RFC 9110 §12): parse `Accept`,
// `Accept-Encoding`, and `Accept-Language`, then rank the server's
// available representations against them and pick the best one.
//
// Each header has three entry points:
//
// - `parse_accept` / `parse_accept_encoding` / `parse_accept_language`
//   read a field value into its ranges, in field order;
// - `rank_media` / `rank_encodings` / `rank_languages` take the field
//   (`None` when the request has no such header) and the server's list, and
//   return the acceptable entries sorted by (q descending, specificity
//   descending, server order);
// - `choose_media` / `choose_encoding` / `choose_language` return the head of
//   that list, or `NegotiationError::NotAcceptable` (406) when it is empty.
//
// What the model checks and leaves out
//
// - Lists follow §5.6.1 for recipients: empty elements and optional
//   whitespace around commas are skipped, so `a, ,b,` is two elements and
//   `` (empty) is a present field with no elements.
// - Parameters follow §5.6.6: `*( OWS ";" OWS [ parameter ] )`, no whitespace
//   around `=`, a value is a token or a quoted-string (`\` escapes one octet;
//   obs-text bytes are kept as they are).
// - A malformed element rejects the whole field (`Malformed { offset }`, the
//   byte where scanning stopped). RFC 9110 does not say whether to drop one
//   element or the field; dropping an element could turn `*;q=0x` into "no
//   exclusion", so the field is refused and the caller decides. §12.4.1 lets
//   a server ignore the header, which is `rank_*(None, ..)`.
// - `q` is matched case-insensitively and must be the qvalue grammar of
//   §12.4.2 exactly: `0`, `0.`, `0.d`, `0.dd`, `0.ddd`, `1`, `1.`, `1.0`,
//   `1.00`, `1.000`. More than three decimals, a value above 1, a quoted q,
//   or a second q in one element is malformed. q is kept as an integer in
//   thousandths (`0.7` is 700). q = 0 means "not acceptable".
// - In `Accept`, any parameter named q is the weight wherever it stands;
//   every other parameter belongs to the media range, before or after q.
//   `Accept-Encoding` and `Accept-Language` elements take only q.
// - Media types: type, subtype, and parameter names are compared
//   case-insensitively (stored lowercase). Parameter values are compared
//   exactly after removing quoting (`"flowed"` equals `flowed`), except
//   `charset`, whose values are case-insensitive. `*/subtype` is malformed.
// - A media range matches a representation when type and subtype match (or
//   are `*`) and every parameter of the range is present on the
//   representation with an equal value. Specificity is `type/subtype` over
//   `type/*` over `*/*`, then the number of range parameters: all
//   parameters count, so `text/plain;format=flowed` beats `text/plain`.
//   When two ranges of equal specificity match, the first in the field wins.
//   Server media types must be concrete (`type/subtype`, no q).
// - Codings are compared case-insensitively. An explicitly listed coding
//   takes its own q; `*` gives its q to every coding not listed. `identity`
//   that is neither listed nor covered by `*` is acceptable by default with
//   the lowest non-zero weight (q = 0.001), so any coding the client lists
//   is preferred to it, and only `identity;q=0`, or `*;q=0` without an
//   `identity` entry, excludes it. An empty `Accept-Encoding` therefore
//   accepts identity only.
// - Languages use RFC 4647 basic filtering (§3.3.1): a range matches a tag
//   when it equals the tag or is a prefix of it followed by `-`, compared
//   case-insensitively; `*` matches every tag. Ranges must be basic
//   language ranges (`1*8ALPHA *("-" 1*8alphanum)` or `*`), so `en-*` is
//   malformed. The most specific matching range is the one with the most
//   subtags; `*` is the least specific. Lookup (§3.4) is not modelled.
// - An absent header makes every representation acceptable with q = 1 and
//   specificity 0, so the server order decides.
// - A present but empty `Accept` or `Accept-Language` matches nothing.
// - Ties: the sorted list orders by q, then by the specificity of the range
//   that matched, then by server order; `choose_*` is its head, so server
//   order is the last tie-break. The sort is a stable insertion sort.
// - Not modelled: `Accept-Charset`, reactive negotiation, `Vary`, and
//   charset/encoding aliases (`x-gzip` is not `gzip`).

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NegotiationError {
    Malformed { offset: usize },
    InvalidAvailable { index: usize },
    NotAcceptable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaParameter {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaRange {
    pub ty: String,
    pub subtype: String,
    pub params: Vec<MediaParameter>,
    pub q: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodingRange {
    pub coding: String,
    pub q: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LanguageRange {
    pub range: String,
    pub q: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ranked {
    pub index: usize,
    pub q: u32,
    pub specificity: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Element {
    head: String,
    tail: String,
    params: Vec<MediaParameter>,
    q: u32,
    weighted: bool,
    at: usize,
}

const FULL: u32 = 1000;

// ---- scanning ----

fn is_tchar(b: u8) -> bool {
    matches!(
        b,
        b'a'..=b'z'
            | b'A'..=b'Z'
            | b'0'..=b'9'
            | b'!'
            | b'#'
            | b'$'
            | b'%'
            | b'&'
            | b'\''
            | b'*'
            | b'+'
            | b'-'
            | b'.'
            | b'^'
            | b'_'
            | b'`'
            | b'|'
            | b'~'
    )
}

fn skip_ows(b: &[u8], start: usize) -> usize {
    let mut i: usize = start;
    while i < b.len() && (b[i] == b' ' || b[i] == b'\t') {
        i += 1;
    }
    i
}

fn token_end(b: &[u8], start: usize) -> usize {
    let mut i: usize = start;
    while i < b.len() && is_tchar(b[i]) {
        i += 1;
    }
    i
}

fn lower(s: &str) -> String {
    s.chars().map(|c| c.to_ascii_lowercase()).collect::<String>()
}

// Reads a quoted-string starting at the opening `"`; returns its unescaped
// contents and the position after the closing `"`.
fn read_quoted(s: &str, start: usize) -> Result<(String, usize), NegotiationError> {
    let b = s.as_bytes();
    let mut out = String::new();
    let mut i: usize = start + 1;
    let mut chunk: usize = i;
    while i < b.len() {
        let c = b[i];
        if c == b'"' {
            out.push_str(&s[chunk..i]);
            return Ok((out, i + 1));
        }
        if c == b'\\' {
            if i + 1 >= b.len() {
                return Err(NegotiationError::Malformed { offset: i });
            }
            let e = b[i + 1];
            if !(e == b'\t' || e >= 0x20) || e == 0x7f {
                return Err(NegotiationError::Malformed { offset: i + 1 });
            }
            out.push_str(&s[chunk..i]);
            chunk = i + 1;
            i += 2;
            continue;
        }
        if !(c == b'\t' || c >= 0x20) || c == 0x7f {
            return Err(NegotiationError::Malformed { offset: i });
        }
        i += 1;
    }
    Err(NegotiationError::Malformed { offset: i })
}

// qvalue = ( "0" [ "." 0*3DIGIT ] ) / ( "1" [ "." 0*3("0") ] ), in thousandths.
fn read_qvalue(v: &str) -> Option<u32> {
    let b = v.as_bytes();
    if b.is_empty() || b.len() > 5 {
        return None;
    }
    let whole: u32 = match b[0] {
        b'0' => 0,
        b'1' => 1,
        _ => return None,
    };
    if b.len() == 1 {
        return Some(whole * FULL);
    }
    if b[1] != b'.' {
        return None;
    }
    let mut frac: u32 = 0;
    let mut scale: u32 = 100;
    let mut i: usize = 2;
    while i < b.len() {
        let d = b[i];
        if !matches!(d, b'0'..=b'9') {
            return None;
        }
        frac += u32::from(d - b'0') * scale;
        scale /= 10;
        i += 1;
    }
    if whole == 1 && frac != 0 {
        return None;
    }
    Some(whole * FULL + frac)
}

// Scans one comma list. With `slashed`, each element head is
// `token "/" token`; otherwise a single token.
fn scan_list(s: &str, slashed: bool) -> Result<Vec<Element>, NegotiationError> {
    let b = s.as_bytes();
    let n = b.len();
    let mut out: Vec<Element> = Vec::new();
    let mut i: usize = 0;
    while i < n {
        i = skip_ows(b, i);
        if i >= n {
            break;
        }
        if b[i] == b',' {
            i += 1;
            continue;
        }
        let at = i;
        let head_end = token_end(b, i);
        if head_end == i {
            return Err(NegotiationError::Malformed { offset: i });
        }
        let head = lower(&s[i..head_end]);
        i = head_end;
        let mut tail = String::new();
        if slashed {
            if i >= n || b[i] != b'/' {
                return Err(NegotiationError::Malformed { offset: i });
            }
            let tail_end = token_end(b, i + 1);
            if tail_end == i + 1 {
                return Err(NegotiationError::Malformed { offset: i + 1 });
            }
            tail = lower(&s[i + 1..tail_end]);
            i = tail_end;
        }
        let mut params: Vec<MediaParameter> = Vec::new();
        let mut q: u32 = FULL;
        let mut weighted = false;
        let mut more = true;
        while more {
            let j = skip_ows(b, i);
            if j < n && b[j] == b';' {
                i = skip_ows(b, j + 1);
                if i >= n || b[i] == b';' || b[i] == b',' {
                    continue;
                }
                let name_end = token_end(b, i);
                if name_end == i {
                    return Err(NegotiationError::Malformed { offset: i });
                }
                let name = lower(&s[i..name_end]);
                if name_end >= n || b[name_end] != b'=' {
                    return Err(NegotiationError::Malformed { offset: name_end });
                }
                let vstart = name_end + 1;
                let mut quoted = false;
                let mut value = String::new();
                if vstart < n && b[vstart] == b'"' {
                    let r = read_quoted(s, vstart)?;
                    let (v, after) = r;
                    value = v;
                    i = after;
                    quoted = true;
                } else {
                    let vend = token_end(b, vstart);
                    if vend == vstart {
                        return Err(NegotiationError::Malformed { offset: vstart });
                    }
                    value.push_str(&s[vstart..vend]);
                    i = vend;
                }
                if name == "q" {
                    if weighted || quoted {
                        return Err(NegotiationError::Malformed { offset: vstart });
                    }
                    match read_qvalue(&value) {
                        Some(w) => q = w,
                        None => return Err(NegotiationError::Malformed { offset: vstart }),
                    }
                    weighted = true;
                } else {
                    params.push(MediaParameter { name, value });
                }
            } else {
                more = false;
            }
        }
        i = skip_ows(b, i);
        if i < n && b[i] != b',' {
            return Err(NegotiationError::Malformed { offset: i });
        }
        out.push(Element {
            head,
            tail,
            params,
            q,
            weighted,
            at,
        });
    }
    Ok(out)
}

// ---- parsing the three fields ----

pub fn parse_accept(field: &str) -> Result<Vec<MediaRange>, NegotiationError> {
    let elements = scan_list(field, true)?;
    let mut out: Vec<MediaRange> = Vec::new();
    for e in &elements {
        if e.head == "*" && e.tail != "*" {
            return Err(NegotiationError::Malformed { offset: e.at });
        }
        out.push(MediaRange {
            ty: e.head.clone(),
            subtype: e.tail.clone(),
            params: e.params.clone(),
            q: e.q,
        });
    }
    Ok(out)
}

pub fn parse_accept_encoding(field: &str) -> Result<Vec<CodingRange>, NegotiationError> {
    let elements = scan_list(field, false)?;
    let mut out: Vec<CodingRange> = Vec::new();
    for e in &elements {
        if !e.params.is_empty() {
            return Err(NegotiationError::Malformed { offset: e.at });
        }
        out.push(CodingRange {
            coding: e.head.clone(),
            q: e.q,
        });
    }
    Ok(out)
}

fn is_alpha(b: u8) -> bool {
    matches!(b, b'a'..=b'z' | b'A'..=b'Z')
}

fn is_alnum(b: u8) -> bool {
    matches!(b, b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9')
}

// language-range = (1*8ALPHA *("-" 1*8alphanum)) / "*"   (RFC 4647 §2.1)
fn is_language_range(s: &str, wildcard: bool) -> bool {
    if s == "*" {
        return wildcard;
    }
    let b = s.as_bytes();
    let mut i: usize = 0;
    let mut first = true;
    while i <= b.len() {
        let start = i;
        while i < b.len() && b[i] != b'-' {
            let ok = if first { is_alpha(b[i]) } else { is_alnum(b[i]) };
            if !ok {
                return false;
            }
            i += 1;
        }
        let width = i - start;
        if width == 0 || width > 8 {
            return false;
        }
        first = false;
        i += 1;
    }
    true
}

pub fn parse_accept_language(field: &str) -> Result<Vec<LanguageRange>, NegotiationError> {
    let elements = scan_list(field, false)?;
    let mut out: Vec<LanguageRange> = Vec::new();
    for e in &elements {
        if !e.params.is_empty() || !is_language_range(&e.head, true) {
            return Err(NegotiationError::Malformed { offset: e.at });
        }
        out.push(LanguageRange {
            range: e.head.clone(),
            q: e.q,
        });
    }
    Ok(out)
}

// ---- stable sort ----

// Inserts `r` after every entry whose (q, specificity) is not lower, so
// entries that tie keep the order they were inserted in (server order).
fn insert_ranked(list: Vec<Ranked>, r: Ranked) -> Vec<Ranked> {
    let mut v = list;
    let mut i: usize = 0;
    while i < v.len() {
        let x = &v[i];
        if x.q < r.q || (x.q == r.q && x.specificity < r.specificity) {
            break;
        }
        i += 1;
    }
    v.insert(i, r);
    v
}

fn head_of(list: Vec<Ranked>) -> Result<Ranked, NegotiationError> {
    if list.is_empty() {
        return Err(NegotiationError::NotAcceptable);
    }
    Ok(list[0].clone())
}

// ---- media types ----

fn same_value(name: &str, a: &str, b: &str) -> bool {
    if name == "charset" {
        a.eq_ignore_ascii_case(b)
    } else {
        a == b
    }
}

fn has_parameter(params: &[MediaParameter], p: &MediaParameter) -> bool {
    params
        .iter()
        .any(|x| x.name == p.name && same_value(&p.name, &x.value, &p.value))
}

// Specificity of `range` on `rep`, or `None` when it does not match.
// 2000 for type/subtype, 1000 for type/*, 0 for */*, plus one per parameter.
fn media_specificity(range: &MediaRange, rep: &MediaRange) -> Option<u32> {
    let mut level: u32 = 0;
    if range.ty != "*" {
        if range.ty != rep.ty {
            return None;
        }
        level = 1000;
        if range.subtype != "*" {
            if range.subtype != rep.subtype {
                return None;
            }
            level = 2000;
        }
    }
    let mut count: u32 = 0;
    for p in &range.params {
        if !has_parameter(&rep.params, p) {
            return None;
        }
        count += 1;
    }
    Some(level + count)
}

fn parse_available_media(available: &[String]) -> Result<Vec<MediaRange>, NegotiationError> {
    let mut out: Vec<MediaRange> = Vec::new();
    for (index, a) in available.iter().enumerate() {
        let elements = match scan_list(a, true) {
            Ok(es) => es,
            Err(_) => return Err(NegotiationError::InvalidAvailable { index }),
        };
        if elements.len() != 1 {
            return Err(NegotiationError::InvalidAvailable { index });
        }
        let e = &elements[0];
        if e.weighted || e.head == "*" || e.tail == "*" {
            return Err(NegotiationError::InvalidAvailable { index });
        }
        out.push(MediaRange {
            ty: e.head.clone(),
            subtype: e.tail.clone(),
            params: e.params.clone(),
            q: FULL,
        });
    }
    Ok(out)
}

pub fn rank_media(
    accept: Option<&str>,
    available: &[String],
) -> Result<Vec<Ranked>, NegotiationError> {
    let reps = parse_available_media(available)?;
    let mut out: Vec<Ranked> = Vec::new();
    match accept {
        None => {
            for (index, _) in reps.iter().enumerate() {
                out = insert_ranked(
                    out,
                    Ranked {
                        index,
                        q: FULL,
                        specificity: 0,
                    },
                );
            }
        }
        Some(field) => {
            let ranges = parse_accept(field)?;
            for (index, rep) in reps.iter().enumerate() {
                let mut found = false;
                let mut best_q: u32 = 0;
                let mut best_s: u32 = 0;
                for range in &ranges {
                    match media_specificity(range, rep) {
                        Some(s) => {
                            if !found || s > best_s {
                                found = true;
                                best_q = range.q;
                                best_s = s;
                            }
                        }
                        None => {}
                    }
                }
                if found && best_q > 0 {
                    out = insert_ranked(
                        out,
                        Ranked {
                            index,
                            q: best_q,
                            specificity: best_s,
                        },
                    );
                }
            }
        }
    }
    Ok(out)
}

pub fn choose_media(accept: Option<&str>, available: &[String]) -> Result<Ranked, NegotiationError> {
    let ranked = rank_media(accept, available)?;
    head_of(ranked)
}

// ---- content codings ----

// The q of `coding` under `ranges` and its specificity (1 when listed,
// 0 through `*` or the identity default); `None` when it is not covered.
fn coding_quality(ranges: &[CodingRange], coding: &str) -> Option<(u32, u32)> {
    let mut star: Option<u32> = None;
    for r in ranges {
        if r.coding.eq_ignore_ascii_case(coding) {
            return Some((r.q, 1));
        }
        if r.coding == "*" && star.is_none() {
            star = Some(r.q);
        }
    }
    match star {
        Some(q) => Some((q, 0)),
        None => {
            if coding.eq_ignore_ascii_case("identity") {
                Some((1, 0))
            } else {
                None
            }
        }
    }
}

pub fn rank_encodings(
    accept_encoding: Option<&str>,
    available: &[String],
) -> Result<Vec<Ranked>, NegotiationError> {
    for (index, a) in available.iter().enumerate() {
        let end = token_end(a.as_bytes(), 0);
        if end == 0 || end != a.len() || a == "*" {
            return Err(NegotiationError::InvalidAvailable { index });
        }
    }
    let mut out: Vec<Ranked> = Vec::new();
    match accept_encoding {
        None => {
            for (index, _) in available.iter().enumerate() {
                out = insert_ranked(
                    out,
                    Ranked {
                        index,
                        q: FULL,
                        specificity: 0,
                    },
                );
            }
        }
        Some(field) => {
            let ranges = parse_accept_encoding(field)?;
            for (index, a) in available.iter().enumerate() {
                match coding_quality(&ranges, a) {
                    Some((q, specificity)) => {
                        if q > 0 {
                            out = insert_ranked(
                                out,
                                Ranked {
                                    index,
                                    q,
                                    specificity,
                                },
                            );
                        }
                    }
                    None => {}
                }
            }
        }
    }
    Ok(out)
}

pub fn choose_encoding(
    accept_encoding: Option<&str>,
    available: &[String],
) -> Result<Ranked, NegotiationError> {
    let ranked = rank_encodings(accept_encoding, available)?;
    head_of(ranked)
}

// ---- languages ----

fn subtag_count(s: &str) -> u32 {
    let mut n: u32 = 1;
    for b in s.bytes() {
        if b == b'-' {
            n += 1;
        }
    }
    n
}

// Basic filtering (RFC 4647 §3.3.1): the specificity of `range` on `tag`,
// or `None` when it does not match.
fn language_specificity(range: &str, tag: &str) -> Option<u32> {
    if range == "*" {
        return Some(0);
    }
    let rl = range.len();
    let tl = tag.len();
    if tl < rl {
        return None;
    }
    if !tag[..rl].eq_ignore_ascii_case(range) {
        return None;
    }
    if tl > rl && tag.as_bytes()[rl] != b'-' {
        return None;
    }
    Some(subtag_count(range))
}

pub fn rank_languages(
    accept_language: Option<&str>,
    available: &[String],
) -> Result<Vec<Ranked>, NegotiationError> {
    for (index, a) in available.iter().enumerate() {
        if !is_language_range(a, false) {
            return Err(NegotiationError::InvalidAvailable { index });
        }
    }
    let mut out: Vec<Ranked> = Vec::new();
    match accept_language {
        None => {
            for (index, _) in available.iter().enumerate() {
                out = insert_ranked(
                    out,
                    Ranked {
                        index,
                        q: FULL,
                        specificity: 0,
                    },
                );
            }
        }
        Some(field) => {
            let ranges = parse_accept_language(field)?;
            for (index, tag) in available.iter().enumerate() {
                let mut found = false;
                let mut best_q: u32 = 0;
                let mut best_s: u32 = 0;
                for r in &ranges {
                    match language_specificity(&r.range, tag) {
                        Some(s) => {
                            if !found || s > best_s {
                                found = true;
                                best_q = r.q;
                                best_s = s;
                            }
                        }
                        None => {}
                    }
                }
                if found && best_q > 0 {
                    out = insert_ranked(
                        out,
                        Ranked {
                            index,
                            q: best_q,
                            specificity: best_s,
                        },
                    );
                }
            }
        }
    }
    Ok(out)
}

pub fn choose_language(
    accept_language: Option<&str>,
    available: &[String],
) -> Result<Ranked, NegotiationError> {
    let ranked = rank_languages(accept_language, available)?;
    head_of(ranked)
}
