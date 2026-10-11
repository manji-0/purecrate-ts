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
//   order is the last tie-break (the sort is stable).
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

fn malformed(offset: usize) -> NegotiationError {
    NegotiationError::Malformed { offset }
}

fn is_tchar(c: char) -> bool {
    c.is_ascii_alphanumeric()
        || matches!(c, '!' | '#' | '$' | '%' | '&' | '\'' | '*' | '+' | '-' | '.' | '^' | '_' | '`' | '|' | '~')
}

fn is_ctl(b: u8) -> bool {
    (b < 0x20 && b != b'\t') || b == 0x7f
}

// The position after the optional whitespace (space and tab) at `start`.
fn skip_ows(s: &str, start: usize) -> usize {
    s.len() - s[start..].trim_start_matches(|c: char| c == ' ' || c == '\t').len()
}

// The end of the token at `start`, which must not be empty.
fn token_end(s: &str, start: usize) -> Result<usize, NegotiationError> {
    let end = s.len() - s[start..].trim_start_matches(|c: char| is_tchar(c)).len();
    if end == start {
        return Err(malformed(start));
    }
    Ok(end)
}

// Reads a parameter value at `start`, a token or a quoted-string; returns it
// (unescaped) and the position after it.
fn read_value(s: &str, start: usize) -> Result<(String, usize), NegotiationError> {
    if !s[start..].starts_with("\"") {
        let end = token_end(s, start)?;
        return Ok((String::from(&s[start..end]), end));
    }
    let b = s.as_bytes();
    let mut out = String::new();
    let mut chunk: usize = start + 1;
    let mut i: usize = chunk;
    while i < b.len() && b[i] != b'"' {
        if b[i] == b'\\' {
            if i + 1 == b.len() {
                return Err(malformed(i));
            }
            out.push_str(&s[chunk..i]);
            chunk = i + 1;
            i += 1;
        }
        if is_ctl(b[i]) {
            return Err(malformed(i));
        }
        i += 1;
    }
    if i == b.len() {
        return Err(malformed(i));
    }
    out.push_str(&s[chunk..i]);
    Ok((out, i + 1))
}

// qvalue = ( "0" [ "." 0*3DIGIT ] ) / ( "1" [ "." 0*3("0") ] ), in thousandths.
fn read_qvalue(v: &str) -> Option<u32> {
    let (whole, frac) = v.split_once('.').unwrap_or((v, ""));
    if frac.len() > 3 {
        return None;
    }
    let mut q: u32 = 0;
    let mut scale: u32 = 100;
    for c in frac.chars() {
        q += c.to_digit(10)? * scale;
        scale /= 10;
    }
    match whole {
        "0" => Some(q),
        "1" if q == 0 => Some(FULL),
        _ => None,
    }
}

// Scans one comma list. With `slashed`, each element head is
// `token "/" token`; otherwise a single token. Positions are byte offsets
// into `s`, so a quoted comma never ends an element.
fn scan_list(s: &str, slashed: bool) -> Result<Vec<Element>, NegotiationError> {
    let mut out: Vec<Element> = Vec::new();
    let mut i = skip_ows(s, 0);
    while i < s.len() {
        if s[i..].starts_with(",") {
            i = skip_ows(s, i + 1);
            continue;
        }
        let at = i;
        i = token_end(s, at)?;
        let head = s[at..i].to_ascii_lowercase();
        let mut tail = String::new();
        if slashed {
            if !s[i..].starts_with("/") {
                return Err(malformed(i));
            }
            let start = i + 1;
            i = token_end(s, start)?;
            tail = s[start..i].to_ascii_lowercase();
        }
        let mut params: Vec<MediaParameter> = Vec::new();
        let mut q: Option<u32> = None;
        while s[skip_ows(s, i)..].starts_with(";") {
            i = skip_ows(s, skip_ows(s, i) + 1);
            if i == s.len() || s[i..].starts_with(";") || s[i..].starts_with(",") {
                continue;
            }
            let name_end = token_end(s, i)?;
            let name = s[i..name_end].to_ascii_lowercase();
            if !s[name_end..].starts_with("=") {
                return Err(malformed(name_end));
            }
            let start = name_end + 1;
            let (value, after) = read_value(s, start)?;
            i = after;
            if name != "q" {
                params.push(MediaParameter { name, value });
                continue;
            }
            if q.is_some() || s[start..].starts_with("\"") {
                return Err(malformed(start));
            }
            q = Some(read_qvalue(&value).ok_or(malformed(start))?);
        }
        i = skip_ows(s, i);
        if i < s.len() && !s[i..].starts_with(",") {
            return Err(malformed(i));
        }
        out.push(Element { head, tail, params, q: q.unwrap_or(FULL), weighted: q.is_some(), at });
    }
    Ok(out)
}

// ---- parsing the three fields ----

fn media_of(e: &Element) -> MediaRange {
    MediaRange { ty: e.head.clone(), subtype: e.tail.clone(), params: e.params.clone(), q: e.q }
}

pub fn parse_accept(field: &str) -> Result<Vec<MediaRange>, NegotiationError> {
    let elements = scan_list(field, true)?;
    elements
        .iter()
        .map(|e| if e.head == "*" && e.tail != "*" { Err(malformed(e.at)) } else { Ok(media_of(e)) })
        .collect()
}

pub fn parse_accept_encoding(field: &str) -> Result<Vec<CodingRange>, NegotiationError> {
    let elements = scan_list(field, false)?;
    elements
        .iter()
        .map(|e| {
            if e.params.is_empty() {
                Ok(CodingRange { coding: e.head.clone(), q: e.q })
            } else {
                Err(malformed(e.at))
            }
        })
        .collect()
}

// language-range = (1*8ALPHA *("-" 1*8alphanum)) / "*"   (RFC 4647 §2.1)
fn is_language_range(s: &str, wildcard: bool) -> bool {
    if s == "*" {
        return wildcard;
    }
    let primary = s.split_once('-').map(|(p, _)| p).unwrap_or(s);
    primary.chars().all(|c| c.is_ascii_alphabetic())
        && s.split('-').all(|t| !t.is_empty() && t.len() <= 8 && t.chars().all(|c| c.is_ascii_alphanumeric()))
}

pub fn parse_accept_language(field: &str) -> Result<Vec<LanguageRange>, NegotiationError> {
    let elements = scan_list(field, false)?;
    elements
        .iter()
        .map(|e| {
            if e.params.is_empty() && is_language_range(&e.head, true) {
                Ok(LanguageRange { range: e.head.clone(), q: e.q })
            } else {
                Err(malformed(e.at))
            }
        })
        .collect()
}

// ---- ranking ----

// Of one representation's matches (in field order), the first of the most
// specific, when its q is not 0.
fn best_match(matches: &[Ranked]) -> Option<Ranked> {
    match matches.iter().min_by(|a, b| b.specificity.cmp(&a.specificity)) {
        Some(m) if m.q > 0 => Some(m.clone()),
        _ => None,
    }
}

// Sorts by q, then specificity, both descending. The sort is stable, so
// entries that tie keep server order.
fn by_rank(list: Vec<Ranked>) -> Vec<Ranked> {
    let mut v = list;
    v.sort_by(|a, b| b.q.cmp(&a.q).then(b.specificity.cmp(&a.specificity)));
    v
}

fn head_of(list: Vec<Ranked>) -> Result<Ranked, NegotiationError> {
    if list.is_empty() {
        return Err(NegotiationError::NotAcceptable);
    }
    Ok(list[0].clone())
}

// ---- media types ----

fn has_parameter(params: &[MediaParameter], p: &MediaParameter) -> bool {
    params.iter().any(|x| {
        x.name == p.name && (x.value == p.value || (p.name == "charset" && x.value.eq_ignore_ascii_case(&p.value)))
    })
}

// Specificity of `range` on `rep`, or `None` when it does not match.
// 2000 for type/subtype, 1000 for type/*, 0 for */*, plus one per parameter.
fn media_specificity(range: &MediaRange, rep: &MediaRange) -> Option<u32> {
    let level: u32 = if range.ty == "*" {
        0
    } else if range.subtype == "*" {
        1000
    } else {
        2000
    };
    let types = (range.ty == "*" || range.ty == rep.ty) && (range.subtype == "*" || range.subtype == rep.subtype);
    if !types || !range.params.iter().all(|p| has_parameter(&rep.params, p)) {
        return None;
    }
    Some(level + range.params.len() as u32)
}

fn parse_available_media(available: &[String]) -> Result<Vec<MediaRange>, NegotiationError> {
    let mut out: Vec<MediaRange> = Vec::new();
    for (index, a) in available.iter().enumerate() {
        let elements = scan_list(a, true).map_err(|_| NegotiationError::InvalidAvailable { index })?;
        if elements.len() != 1 || elements[0].weighted || elements[0].head == "*" || elements[0].tail == "*" {
            return Err(NegotiationError::InvalidAvailable { index });
        }
        out.push(media_of(&elements[0]));
    }
    Ok(out)
}

// An absent header is `*/*`: every representation at q = 1, specificity 0.
pub fn rank_media(accept: Option<&str>, available: &[String]) -> Result<Vec<Ranked>, NegotiationError> {
    let reps = parse_available_media(available)?;
    let ranges = parse_accept(accept.unwrap_or("*/*"))?;
    let mut out: Vec<Ranked> = Vec::new();
    for (index, rep) in reps.iter().enumerate() {
        let mut matches: Vec<Ranked> = Vec::new();
        for range in &ranges {
            if let Some(specificity) = media_specificity(range, rep) {
                matches.push(Ranked { index, q: range.q, specificity });
            }
        }
        if let Some(m) = best_match(&matches) {
            out.push(m);
        }
    }
    Ok(by_rank(out))
}

pub fn choose_media(accept: Option<&str>, available: &[String]) -> Result<Ranked, NegotiationError> {
    let ranked = rank_media(accept, available)?;
    head_of(ranked)
}

// ---- content codings ----

// The q of `coding` under `ranges` and its specificity (1 when listed,
// 0 through `*` or the identity default); `None` when it is not covered.
fn coding_quality(ranges: &[CodingRange], coding: &str) -> Option<(u32, u32)> {
    if let Some(r) = ranges.iter().find(|r| r.coding.eq_ignore_ascii_case(coding)) {
        return Some((r.q, 1));
    }
    if let Some(r) = ranges.iter().find(|r| r.coding == "*") {
        return Some((r.q, 0));
    }
    if coding.eq_ignore_ascii_case("identity") {
        Some((1, 0))
    } else {
        None
    }
}

// An absent header is `*`: every coding at q = 1, specificity 0.
pub fn rank_encodings(accept_encoding: Option<&str>, available: &[String]) -> Result<Vec<Ranked>, NegotiationError> {
    if let Some(index) = available.iter().position(|a| a.is_empty() || a == "*" || !a.chars().all(is_tchar)) {
        return Err(NegotiationError::InvalidAvailable { index });
    }
    let ranges = parse_accept_encoding(accept_encoding.unwrap_or("*"))?;
    let mut out: Vec<Ranked> = Vec::new();
    for (index, a) in available.iter().enumerate() {
        if let Some((q, specificity)) = coding_quality(&ranges, a) {
            if q > 0 {
                out.push(Ranked { index, q, specificity });
            }
        }
    }
    Ok(by_rank(out))
}

pub fn choose_encoding(accept_encoding: Option<&str>, available: &[String]) -> Result<Ranked, NegotiationError> {
    let ranked = rank_encodings(accept_encoding, available)?;
    head_of(ranked)
}

// ---- languages ----

// Basic filtering (RFC 4647 §3.3.1): the specificity of `range` on `tag`
// (its number of subtags), or `None` when it does not match.
fn language_specificity(range: &str, tag: &str) -> Option<u32> {
    if range == "*" {
        return Some(0);
    }
    let n = range.len();
    if tag.len() < n || !tag[..n].eq_ignore_ascii_case(range) || (tag.len() > n && tag.as_bytes()[n] != b'-') {
        return None;
    }
    Some(range.split('-').count() as u32)
}

// An absent header is `*`: every tag at q = 1, specificity 0.
pub fn rank_languages(accept_language: Option<&str>, available: &[String]) -> Result<Vec<Ranked>, NegotiationError> {
    if let Some(index) = available.iter().position(|a| !is_language_range(a, false)) {
        return Err(NegotiationError::InvalidAvailable { index });
    }
    let ranges = parse_accept_language(accept_language.unwrap_or("*"))?;
    let mut out: Vec<Ranked> = Vec::new();
    for (index, tag) in available.iter().enumerate() {
        let mut matches: Vec<Ranked> = Vec::new();
        for r in &ranges {
            if let Some(specificity) = language_specificity(&r.range, tag) {
                matches.push(Ranked { index, q: r.q, specificity });
            }
        }
        if let Some(m) = best_match(&matches) {
            out.push(m);
        }
    }
    Ok(by_rank(out))
}

pub fn choose_language(accept_language: Option<&str>, available: &[String]) -> Result<Ranked, NegotiationError> {
    let ranked = rank_languages(accept_language, available)?;
    head_of(ranked)
}
