//! `examples/negotiate`, RFC 9110 §12 proactive negotiation over `Accept`,
//! `Accept-Encoding`, and `Accept-Language`:
//!
//! - RFC 9110's examples come out as published: §12.5.1's precedence
//!   example, its effective-q table (with the `text/html;level=3` row as
//!   corrected by verified erratum 7138), `audio/*; q=0.2, audio/basic`,
//!   §12.5.3's five `Accept-Encoding` fields and its identity rules,
//!   §12.5.4's `Accept-Language` field, and the qvalue grammar of §12.4.2
//!   at its edges, malformed offsets included;
//! - the same rules in idiomatic Rust (`idiomatic`, the line count design/07
//!   §2 compares against: `split` at separators outside quoted-strings,
//!   `trim_matches`, `to_ascii_lowercase`, `split_once`, `parse`, a stable
//!   `sort_by_key`, `max_by_key`) agree on the parsed ranges, the full
//!   ranking, and the choice or 406, over seeded fields drawn from a grammar
//!   (ranges, token and quoted parameters with escapes and commas, valid and
//!   invalid q, OWS, empty elements, case), over every one-byte deletion,
//!   replacement, and insertion in a set of fields, and over available
//!   lists with duplicates and invalid entries, absent and empty headers
//!   included. A malformed field is compared as a class: the reference does
//!   not scan byte by byte, so it cannot name the byte where the model
//!   stopped; the model's offset is checked to lie on a character boundary
//!   of the field, and the exact offsets are asserted below and compared
//!   with the generated package;
//! - the generated package agrees with Rust on a sample of each, ties,
//!   q = 0, identity, and malformed offsets after multibyte text
//!   over-represented.

use crate::support::{self, Rng};

purecrate_canon::fixture!(mod negotiate = "../../../examples/negotiate/src/lib.rs");

use negotiate::{CodingRange, LanguageRange, MediaParameter, MediaRange, NegotiationError, Ranked};

/// The model's rules as one would write them with `str` methods and std's
/// sorts. Not converted; the reference only. A ranking entry is
/// `(index, q, specificity)` with the model's specificity numbers (media:
/// 2000 / 1000 / 0 for `type/subtype` / `type/*` / `*/*` plus one per
/// parameter). Errors are classes: `Malformed` carries no offset.
mod idiomatic {
    use std::cmp::Reverse;

    #[derive(Debug, Clone, Copy, PartialEq)]
    pub enum Error {
        Malformed,
        InvalidAvailable(usize),
        NotAcceptable,
    }
    use Error::*;

    pub type Ranking = Vec<(usize, u32, u32)>;

    #[derive(Debug, Clone, PartialEq)]
    pub struct Element {
        pub head: String,
        pub params: Vec<(String, String)>,
        pub q: Option<u32>,
    }

    #[derive(Debug, Clone, PartialEq)]
    pub struct Media {
        pub ty: String,
        pub sub: String,
        pub params: Vec<(String, String)>,
        pub q: u32,
    }

    fn is_token(s: &str) -> bool {
        !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&b))
    }

    fn trim_ows(s: &str) -> &str {
        s.trim_matches([' ', '\t'])
    }

    /// `s` split at each `sep` that is not inside a quoted-string.
    fn split_unquoted(s: &str, sep: char) -> Vec<&str> {
        let (mut parts, mut start, mut quoted, mut escaped) = (Vec::new(), 0, false, false);
        for (i, c) in s.char_indices() {
            if escaped {
                escaped = false;
            } else if quoted && c == '\\' {
                escaped = true;
            } else if c == '"' {
                quoted = !quoted;
            } else if c == sep && !quoted {
                parts.push(&s[start..i]);
                start = i + 1;
            }
        }
        parts.push(&s[start..]);
        parts
    }

    /// The contents of `v` when all of it is one quoted-string.
    fn unquote(v: &str) -> Option<String> {
        let inner = v.strip_prefix('"')?.strip_suffix('"')?;
        let mut out = String::new();
        let mut chars = inner.chars();
        while let Some(c) = chars.next() {
            let c = match c {
                '\\' => chars.next()?,
                '"' => return None,
                c => c,
            };
            if c.is_ascii_control() && c != '\t' {
                return None;
            }
            out.push(c);
        }
        Some(out)
    }

    /// §12.4.2, in thousandths.
    fn qvalue(v: &str) -> Option<u32> {
        let (whole, frac) = v.split_once('.').unwrap_or((v, ""));
        if frac.len() > 3 || !frac.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        let thousandths: u32 = format!("{frac:0<3}").parse().ok()?;
        match whole {
            "0" => Some(thousandths),
            "1" if thousandths == 0 => Some(1000),
            _ => None,
        }
    }

    fn element(e: &str, slashed: bool) -> Result<Element, Error> {
        let mut parts = split_unquoted(e, ';').into_iter();
        let head = trim_ows(parts.next().unwrap_or_default()).to_ascii_lowercase();
        let well_formed = match head.split_once('/') {
            Some((ty, sub)) => slashed && is_token(ty) && is_token(sub),
            None => !slashed && is_token(&head),
        };
        if !well_formed {
            return Err(Malformed);
        }
        let (mut params, mut q) = (Vec::new(), None);
        for p in parts.map(trim_ows).filter(|p| !p.is_empty()) {
            let (name, value) = p.split_once('=').ok_or(Malformed)?;
            let name = name.to_ascii_lowercase();
            if !is_token(&name) || (name == "q" && q.is_some()) {
                return Err(Malformed);
            }
            if name == "q" {
                q = Some(qvalue(value).ok_or(Malformed)?);
            } else if value.starts_with('"') {
                params.push((name, unquote(value).ok_or(Malformed)?));
            } else if is_token(value) {
                params.push((name, value.to_string()));
            } else {
                return Err(Malformed);
            }
        }
        Ok(Element { head, params, q })
    }

    /// A comma list (§5.6.1): empty elements and OWS around them skipped.
    pub fn parse_list(field: &str, slashed: bool) -> Result<Vec<Element>, Error> {
        split_unquoted(field, ',')
            .into_iter()
            .map(trim_ows)
            .filter(|e| !e.is_empty())
            .map(|e| element(e, slashed))
            .collect()
    }

    fn media(e: &Element) -> Media {
        let (ty, sub) = e.head.split_once('/').expect("a slashed head");
        Media { ty: ty.into(), sub: sub.into(), params: e.params.clone(), q: e.q.unwrap_or(1000) }
    }

    pub fn parse_accept(field: &str) -> Result<Vec<Media>, Error> {
        let ranges: Vec<Media> = parse_list(field, true)?.iter().map(media).collect();
        if ranges.iter().any(|r| r.ty == "*" && r.sub != "*") {
            return Err(Malformed);
        }
        Ok(ranges)
    }

    /// Elements that take only q, as (head, q).
    fn weights(field: &str, valid: impl Fn(&str) -> bool) -> Result<Vec<(String, u32)>, Error> {
        let elements = parse_list(field, false)?;
        if elements.iter().any(|e| !e.params.is_empty() || !valid(&e.head)) {
            return Err(Malformed);
        }
        Ok(elements.into_iter().map(|e| (e.head, e.q.unwrap_or(1000))).collect())
    }

    pub fn parse_accept_encoding(field: &str) -> Result<Vec<(String, u32)>, Error> {
        weights(field, |_| true)
    }

    /// RFC 4647 §2.1 basic language range, `*` aside.
    fn is_basic_range(s: &str) -> bool {
        let mut subtags = s.split('-');
        let first = subtags.next().unwrap_or_default();
        (1..=8).contains(&first.len())
            && first.bytes().all(|b| b.is_ascii_alphabetic())
            && subtags.all(|t| (1..=8).contains(&t.len()) && t.bytes().all(|b| b.is_ascii_alphanumeric()))
    }

    pub fn parse_accept_language(field: &str) -> Result<Vec<(String, u32)>, Error> {
        weights(field, |r| r == "*" || is_basic_range(r))
    }

    /// The (q, specificity) of the most specific match, the first in the
    /// field winning a tie: `max_by_key` keeps the last maximum, so the
    /// ranges go in reverse.
    fn best<R>(ranges: &[R], f: impl Fn(&R) -> Option<(u32, u32)>) -> Option<(u32, u32)> {
        ranges.iter().rev().filter_map(f).max_by_key(|&(_, s)| s)
    }

    /// Acceptable entries best first; `sort_by_key` is stable, so ties keep
    /// the server's order.
    fn ranked(n: usize, quality: impl Fn(usize) -> Option<(u32, u32)>) -> Ranking {
        let mut out: Ranking = (0..n).filter_map(|i| quality(i).map(|(q, s)| (i, q, s))).filter(|r| r.1 > 0).collect();
        out.sort_by_key(|&(_, q, s)| Reverse((q, s)));
        out
    }

    fn specificity(range: &Media, rep: &Media) -> Option<u32> {
        let level = match (range.ty.as_str(), range.sub.as_str()) {
            ("*", _) => 0,
            (ty, "*") if ty == rep.ty => 1000,
            (ty, sub) if ty == rep.ty && sub == rep.sub => 2000,
            _ => return None,
        };
        let present = |(n, v): &(String, String)| {
            rep.params.iter().any(|(m, w)| n == m && if n == "charset" { v.eq_ignore_ascii_case(w) } else { v == w })
        };
        range.params.iter().all(present).then(|| level + range.params.len() as u32)
    }

    pub fn rank_media(accept: Option<&str>, available: &[String]) -> Result<Ranking, Error> {
        let reps = available
            .iter()
            .enumerate()
            .map(|(i, a)| {
                match parse_list(a, true).as_deref() {
                    Ok([e]) if e.q.is_none() => Some(media(e)).filter(|m| m.ty != "*" && m.sub != "*"),
                    _ => None,
                }
                .ok_or(InvalidAvailable(i))
            })
            .collect::<Result<Vec<Media>, Error>>()?;
        let Some(field) = accept else { return Ok(ranked(reps.len(), |_| Some((1000, 0)))) };
        let ranges = parse_accept(field)?;
        Ok(ranked(reps.len(), |i| best(&ranges, |r| Some((r.q, specificity(r, &reps[i])?)))))
    }

    pub fn rank_encodings(accept_encoding: Option<&str>, available: &[String]) -> Result<Ranking, Error> {
        if let Some(i) = available.iter().position(|a| !is_token(a) || a == "*") {
            return Err(InvalidAvailable(i));
        }
        let Some(field) = accept_encoding else { return Ok(ranked(available.len(), |_| Some((1000, 0)))) };
        let ranges = parse_accept_encoding(field)?;
        let listed = |c: &str| ranges.iter().find(|(r, _)| r == c).map(|&(_, q)| q);
        Ok(ranked(available.len(), |i| {
            let coding = available[i].to_ascii_lowercase();
            (listed(&coding).map(|q| (q, 1)))
                .or_else(|| listed("*").map(|q| (q, 0)))
                .or_else(|| (coding == "identity").then_some((1, 0)))
        }))
    }

    pub fn rank_languages(accept_language: Option<&str>, available: &[String]) -> Result<Ranking, Error> {
        if let Some(i) = available.iter().position(|a| !is_basic_range(a)) {
            return Err(InvalidAvailable(i));
        }
        let Some(field) = accept_language else { return Ok(ranked(available.len(), |_| Some((1000, 0)))) };
        let ranges = parse_accept_language(field)?;
        Ok(ranked(available.len(), |i| {
            let tag = available[i].to_ascii_lowercase();
            best(&ranges, |(range, q)| match range.as_str() {
                "*" => Some((*q, 0)),
                r if tag == r || tag.starts_with(&format!("{r}-")) => Some((*q, r.split('-').count() as u32)),
                _ => None,
            })
        }))
    }

    pub fn choose(ranking: Result<Ranking, Error>) -> Result<(usize, u32, u32), Error> {
        ranking?.first().copied().ok_or(NotAcceptable)
    }
}

use idiomatic::Error as E;

/// Which header a field is for.
#[derive(Clone, Copy, Debug)]
enum Header {
    Accept,
    Encoding,
    Language,
}

fn class(e: NegotiationError, field: &str) -> E {
    match e {
        NegotiationError::Malformed { offset } => {
            assert!(field.is_char_boundary(offset), "offset {offset} in {field:?}");
            E::Malformed
        }
        NegotiationError::InvalidAvailable { index } => E::InvalidAvailable(index),
        NegotiationError::NotAcceptable => E::NotAcceptable,
    }
}

fn entry(r: &Ranked) -> (usize, u32, u32) {
    (r.index, r.q, r.specificity)
}

fn media_of(r: &MediaRange) -> idiomatic::Media {
    idiomatic::Media {
        ty: r.ty.clone(),
        sub: r.subtype.clone(),
        params: r.params.iter().map(|p| (p.name.clone(), p.value.clone())).collect(),
        q: r.q,
    }
}

/// The model's parse and the reference's, as debug text.
fn parses(h: Header, field: &str) -> (String, String) {
    let cl = |e| class(e, field);
    match h {
        Header::Accept => (
            format!(
                "{:?}",
                negotiate::parse_accept(field).map(|v| v.iter().map(media_of).collect::<Vec<_>>()).map_err(cl)
            ),
            format!("{:?}", idiomatic::parse_accept(field)),
        ),
        Header::Encoding => (
            format!(
                "{:?}",
                negotiate::parse_accept_encoding(field)
                    .map(|v| v.into_iter().map(|c| (c.coding, c.q)).collect::<Vec<_>>())
                    .map_err(cl)
            ),
            format!("{:?}", idiomatic::parse_accept_encoding(field)),
        ),
        Header::Language => (
            format!(
                "{:?}",
                negotiate::parse_accept_language(field)
                    .map(|v| v.into_iter().map(|l| (l.range, l.q)).collect::<Vec<_>>())
                    .map_err(cl)
            ),
            format!("{:?}", idiomatic::parse_accept_language(field)),
        ),
    }
}

/// The model's ranking and choice, as the reference's classes.
fn model_rank(h: Header, field: Option<&str>, available: &[String]) -> Result<idiomatic::Ranking, E> {
    let r = match h {
        Header::Accept => negotiate::rank_media(field, available),
        Header::Encoding => negotiate::rank_encodings(field, available),
        Header::Language => negotiate::rank_languages(field, available),
    };
    r.map(|v| v.iter().map(entry).collect()).map_err(|e| class(e, field.unwrap_or_default()))
}

fn model_choose(h: Header, field: Option<&str>, available: &[String]) -> Result<(usize, u32, u32), E> {
    let r = match h {
        Header::Accept => negotiate::choose_media(field, available),
        Header::Encoding => negotiate::choose_encoding(field, available),
        Header::Language => negotiate::choose_language(field, available),
    };
    r.map(|v| entry(&v)).map_err(|e| class(e, field.unwrap_or_default()))
}

fn reference_rank(h: Header, field: Option<&str>, available: &[String]) -> Result<idiomatic::Ranking, E> {
    match h {
        Header::Accept => idiomatic::rank_media(field, available),
        Header::Encoding => idiomatic::rank_encodings(field, available),
        Header::Language => idiomatic::rank_languages(field, available),
    }
}

fn strings(xs: &[&str]) -> Vec<String> {
    xs.iter().map(|s| s.to_string()).collect()
}

fn malformed(offset: usize) -> NegotiationError {
    NegotiationError::Malformed { offset }
}

fn ranked(entries: &[(usize, u32, u32)]) -> Vec<Ranked> {
    entries.iter().map(|&(index, q, specificity)| Ranked { index, q, specificity }).collect()
}

fn param(name: &str, value: &str) -> MediaParameter {
    MediaParameter { name: name.into(), value: value.into() }
}

// ---- RFC 9110 ----

/// §12.5.1: "more specific media ranges or media types override less
/// specific ones".
const PRECEDENCE: &str = "text/*, text/plain, text/plain;format=flowed, */*";

/// §12.5.1's effective-q example.
const TABLE: &str =
    "text/*;q=0.3, text/plain;q=0.7, text/plain;format=flowed, text/plain;format=fixed;q=0.4, */*;q=0.5";

/// The table's rows, in its order. The last row is printed as 0.7 in RFC
/// 9110, but no range in the field names `text/html`; the most specific
/// match is `text/*;q=0.3`. Erratum 7138 (Verified, 2022-11-09) corrects
/// the row to 0.3: the 0.7 came from RFC 7231 §5.3.2's field, which had
/// `text/html;q=0.7`. The model gives 0.3.
const TABLE_ROWS: [(&str, u32); 6] = [
    ("text/plain;format=flowed", 1000),
    ("text/plain", 700),
    ("text/html", 300),
    ("image/jpeg", 500),
    ("text/plain;format=fixed", 400),
    ("text/html;level=3", 300),
];

/// §12.5.3's five example fields.
const ENCODING_EXAMPLES: [&str; 5] =
    ["compress, gzip", "", "*", "compress;q=0.5, gzip;q=1.0", "gzip;q=1.0, identity; q=0.5, *;q=0"];

/// §12.5.4's example field.
const LANGUAGE_EXAMPLE: &str = "da, en-gb;q=0.8, en;q=0.7";

/// §12.4.2 qvalues that are well formed, with their thousandths.
const GOOD_Q: [(&str, u32); 14] = [
    ("0", 0),
    ("0.", 0),
    ("0.0", 0),
    ("0.000", 0),
    ("0.001", 1),
    ("0.5", 500),
    ("0.50", 500),
    ("0.500", 500),
    ("0.05", 50),
    ("0.999", 999),
    ("1", 1000),
    ("1.", 1000),
    ("1.0", 1000),
    ("1.000", 1000),
];

/// Values in a q's place that are not qvalues; each is malformed at the
/// value's first byte.
const BAD_Q: [&str; 17] = [
    "1.001", "1.0000", "0.1234", "0.0001", "2", ".5", "01", "00.5", "-0", "+1", "0.5x", "1.5", "1..", "0.a", "\"0.5\"",
    "", "1e0",
];

#[test]
fn the_rfc_examples_come_out_as_published() {
    // §12.5.1 precedence: flowed, then text/plain, then text/*, then */*.
    let avail = strings(&["image/png", "text/html", "text/plain", "text/plain;format=flowed"]);
    assert_eq!(
        negotiate::rank_media(Some(PRECEDENCE), &avail),
        Ok(ranked(&[(3, 1000, 2001), (2, 1000, 2000), (1, 1000, 1000), (0, 1000, 0)]))
    );

    // §12.5.1 effective q, one representation at a time and all at once.
    for (rep, q) in TABLE_ROWS {
        let got = negotiate::choose_media(Some(TABLE), &strings(&[rep])).map(|r| r.q);
        assert_eq!(got, Ok(q), "{rep}");
    }
    let rows: Vec<String> = TABLE_ROWS.iter().map(|(r, _)| r.to_string()).collect();
    assert_eq!(
        negotiate::rank_media(Some(TABLE), &rows),
        Ok(ranked(&[(0, 1000, 2001), (1, 700, 2000), (3, 500, 0), (4, 400, 2001), (2, 300, 1000), (5, 300, 1000)]))
    );
    assert_eq!(
        negotiate::parse_accept(TABLE).map(|v| v.iter().map(|r| (r.subtype.clone(), r.params.len(), r.q)).collect()),
        Ok(vec![
            ("*".into(), 0, 300),
            ("plain".into(), 0, 700),
            ("plain".into(), 1, 1000),
            ("plain".into(), 1, 400),
            ("*".into(), 0, 500)
        ])
    );

    // "audio/*; q=0.2, audio/basic": audio/basic preferred, other audio at 0.2.
    let audio = "audio/*; q=0.2, audio/basic";
    assert_eq!(
        negotiate::rank_media(Some(audio), &strings(&["audio/x-wav", "audio/basic", "text/plain"])),
        Ok(ranked(&[(1, 1000, 2000), (0, 200, 1000)]))
    );
    assert_eq!(negotiate::choose_media(Some(audio), &strings(&["text/html"])), Err(NegotiationError::NotAcceptable));
    // "text/plain; q=0.5, text/html, text/x-dvi; q=0.8, text/x-c".
    assert_eq!(
        negotiate::rank_media(
            Some("text/plain; q=0.5, text/html, text/x-dvi; q=0.8, text/x-c"),
            &strings(&["text/plain", "text/x-dvi", "text/html", "text/x-c"])
        ),
        Ok(ranked(&[(2, 1000, 2000), (3, 1000, 2000), (1, 800, 2000), (0, 500, 2000)]))
    );

    // Media details from the header.
    assert_eq!(
        negotiate::parse_accept(r#"TEXT/Plain;Format="fl\owed";charset=UTF-8;Q=0.5"#),
        Ok(vec![MediaRange {
            ty: "text".into(),
            subtype: "plain".into(),
            params: vec![param("format", "flowed"), param("charset", "UTF-8")],
            q: 500,
        }])
    );
    assert_eq!(negotiate::parse_accept("text/plain;q=0.5;format=flowed").map(|v| v[0].params.len()), Ok(1));
    assert_eq!(negotiate::parse_accept(r#"a/b;x="1,2;3""#).map(|v| v[0].params.clone()), Ok(vec![param("x", "1,2;3")]));
    let flowed = strings(&["text/plain;format=flowed", "text/plain;format=FLOWED", "text/plain;charset=utf-8"]);
    assert_eq!(negotiate::rank_media(Some(r#"text/plain;format="flowed""#), &flowed), Ok(ranked(&[(0, 1000, 2001)])));
    assert_eq!(negotiate::rank_media(Some("text/plain;CHARSET=UTF-8"), &flowed), Ok(ranked(&[(2, 1000, 2001)])));
    // Equal specificity: the first range in the field gives the q.
    assert_eq!(negotiate::choose_media(Some("text/plain;q=0.5, TEXT/PLAIN;q=0.9"), &flowed).map(|r| r.q), Ok(500));
    // q = 0 on the most specific match excludes, whatever the others say.
    assert_eq!(negotiate::rank_media(Some("*/*, text/plain;q=0"), &flowed), Ok(vec![]));
    assert_eq!(negotiate::parse_accept("*/html"), Err(malformed(0)));
    assert_eq!(negotiate::parse_accept("a/b, */html"), Err(malformed(5)));
    assert_eq!(negotiate::parse_accept("a/b, ,c/d,").map(|v| v.len()), Ok(2));
    assert_eq!(negotiate::parse_accept(" \t "), Ok(vec![]));
    assert_eq!(negotiate::rank_media(Some(""), &flowed), Ok(vec![]));
    assert_eq!(negotiate::rank_media(None, &flowed), Ok(ranked(&[(0, 1000, 0), (1, 1000, 0), (2, 1000, 0)])));
    assert_eq!(negotiate::parse_accept("a/b;x =1"), Err(malformed(5)));
    assert_eq!(negotiate::parse_accept("a/b;x= 1"), Err(malformed(6)));
    assert_eq!(negotiate::parse_accept(r#"a/b;x="1"#), Err(malformed(8)));
    assert_eq!(negotiate::parse_accept("a/b;x=\"\u{1}\""), Err(malformed(7)));
    assert_eq!(negotiate::parse_accept("é/b"), Err(malformed(0)));
    assert_eq!(negotiate::parse_accept("a/b;x=\"é\" y"), Err(malformed(11)));
    for (bad, index) in [("text/*", 1), ("*/*", 1), ("a/b;q=1", 1), ("a/b, c/d", 1), ("", 1), ("a/b;x=\"", 1)] {
        assert_eq!(
            negotiate::rank_media(Some("*/*"), &strings(&["a/b", bad])),
            Err(NegotiationError::InvalidAvailable { index }),
            "{bad:?}"
        );
    }
    assert_eq!(negotiate::rank_media(Some("*/*"), &strings(&[" a/b ,"])), Ok(ranked(&[(0, 1000, 0)])));
    // The available list is checked before the field.
    assert_eq!(
        negotiate::rank_media(Some("*/x"), &strings(&["*/*"])),
        Err(NegotiationError::InvalidAvailable { index: 0 })
    );

    // §12.5.3.
    let codings = strings(&["identity", "gzip", "compress", "br"]);
    let encodings = ENCODING_EXAMPLES.map(|f| negotiate::rank_encodings(Some(f), &codings));
    assert_eq!(
        encodings,
        [
            Ok(ranked(&[(1, 1000, 1), (2, 1000, 1), (0, 1, 0)])),
            Ok(ranked(&[(0, 1, 0)])),
            Ok(ranked(&[(0, 1000, 0), (1, 1000, 0), (2, 1000, 0), (3, 1000, 0)])),
            Ok(ranked(&[(1, 1000, 1), (2, 500, 1), (0, 1, 0)])),
            Ok(ranked(&[(1, 1000, 1), (0, 500, 1)])),
        ]
    );
    assert_eq!(negotiate::rank_encodings(None, &codings).map(|v| v.len()), Ok(4));
    // Identity: excluded only by `identity;q=0`, or `*;q=0` without an identity entry.
    let identity = |f: &str| negotiate::rank_encodings(Some(f), &strings(&["IDENTITY"]));
    assert_eq!(identity("identity;q=0"), Ok(vec![]));
    assert_eq!(identity("*;q=0"), Ok(vec![]));
    assert_eq!(identity("*;q=0, identity;q=0.1"), Ok(ranked(&[(0, 100, 1)])));
    assert_eq!(identity("gzip;q=0"), Ok(ranked(&[(0, 1, 0)])));
    assert_eq!(identity("*;q=0.2"), Ok(ranked(&[(0, 200, 0)])));
    assert_eq!(
        negotiate::rank_encodings(Some("gzip;q=0.001"), &codings),
        Ok(ranked(&[(1, 1, 1), (0, 1, 0)])),
        "the least listed coding still precedes identity"
    );
    assert_eq!(negotiate::choose_encoding(Some("gzip"), &strings(&["br"])), Err(NegotiationError::NotAcceptable));
    assert_eq!(negotiate::rank_encodings(Some("gzip;q=0.5, GZIP"), &codings), Ok(ranked(&[(1, 500, 1), (0, 1, 0)])));
    assert_eq!(negotiate::rank_encodings(Some("x-gzip"), &strings(&["gzip"])), Ok(vec![]));
    assert_eq!(negotiate::parse_accept_encoding("gzip;level=1"), Err(malformed(0)));
    assert_eq!(negotiate::parse_accept_encoding("gzip/1"), Err(malformed(4)));
    for (bad, index) in [("*", 0), ("", 0), ("g zip", 0), ("gzip;q=1", 0)] {
        assert_eq!(
            negotiate::rank_encodings(None, &strings(&[bad])),
            Err(NegotiationError::InvalidAvailable { index }),
            "{bad:?}"
        );
    }

    // §12.5.4.
    let tags = strings(&["en-US", "en-GB", "da", "fr", "en"]);
    assert_eq!(
        negotiate::rank_languages(Some(LANGUAGE_EXAMPLE), &tags),
        Ok(ranked(&[(2, 1000, 1), (1, 800, 2), (0, 700, 1), (4, 700, 1)]))
    );
    assert_eq!(
        negotiate::parse_accept_language("EN-gb;q=0.8, *;q=0.1"),
        Ok(vec![LanguageRange { range: "en-gb".into(), q: 800 }, LanguageRange { range: "*".into(), q: 100 }])
    );
    assert_eq!(
        negotiate::rank_languages(Some("*;q=0.1, fr;q=0"), &tags),
        Ok(ranked(&[(0, 100, 0), (1, 100, 0), (2, 100, 0), (4, 100, 0)]))
    );
    assert_eq!(negotiate::rank_languages(Some("e"), &strings(&["en"])), Ok(vec![]));
    assert_eq!(negotiate::rank_languages(Some("en-us-x"), &strings(&["en-US"])), Ok(vec![]));
    assert_eq!(negotiate::rank_languages(Some(""), &tags), Ok(vec![]));
    for bad in ["en-*", "e1", "en--us", "abcdefghi", "en_gb", "en-", "-en"] {
        assert_eq!(negotiate::parse_accept_language(bad), Err(malformed(0)), "{bad}");
    }
    for (bad, index) in [("*", 1), ("en-", 1), ("1en", 1)] {
        assert_eq!(
            negotiate::rank_languages(None, &strings(&["en", bad])),
            Err(NegotiationError::InvalidAvailable { index }),
            "{bad:?}"
        );
    }

    // §12.4.2.
    for (v, q) in GOOD_Q {
        for name in ["q", "Q"] {
            let field = format!("gzip;{name}={v}");
            assert_eq!(
                negotiate::parse_accept_encoding(&field),
                Ok(vec![CodingRange { coding: "gzip".into(), q }]),
                "{field}"
            );
        }
    }
    for v in BAD_Q {
        assert_eq!(negotiate::parse_accept_encoding(&format!("gzip;q={v}")), Err(malformed(7)), "{v}");
    }
    assert_eq!(negotiate::parse_accept_encoding("gzip;q=0.5;q=0.5"), Err(malformed(13)));
    assert_eq!(
        negotiate::parse_accept_encoding("gzip; q=0.5 ;"),
        Ok(vec![CodingRange { coding: "gzip".into(), q: 500 }])
    );
    assert_eq!(negotiate::parse_accept_encoding("gzip;q=0,5").map(|v| v.len()), Ok(2));
    assert_eq!(negotiate::parse_accept_encoding("gzip;q =0.5"), Err(malformed(6)));
}

// ---- inputs ----

const TYPES: [&str; 8] = ["text", "TEXT", "Text", "image", "audio", "*", "application", "x-y.z+w"];
const SUBTYPES: [&str; 7] = ["plain", "PLAIN", "html", "*", "jpeg", "vnd.a+json", "basic"];
const PARAM_NAMES: [&str; 6] = ["format", "Format", "level", "charset", "CHARSET", "a"];
const PARAM_VALUES: [&str; 17] = [
    "flowed",
    "FLOWED",
    "fixed",
    "1",
    "3",
    "utf-8",
    "UTF-8",
    "\"flowed\"",
    "\"fl\\owed\"",
    "\"a,b\"",
    "\"a;b, c/d\"",
    "\"x\\\"q=1\"",
    "\"\"",
    "\"\\\\\"",
    "\"é😀\"",
    "\"\\é\"",
    "\"a\tb\"",
];
const QS: [&str; 12] = ["0", "0.", "0.5", "0.50", "0.001", "0.7", "0.3", "1", "1.", "1.000", "0.000", "0.999"];
const CODINGS: [&str; 10] =
    ["gzip", "GZIP", "compress", "deflate", "br", "identity", "Identity", "*", "zstd", "x-gzip"];
const LANGUAGES: [&str; 13] =
    ["*", "en", "EN", "en-gb", "en-GB", "en-US", "da", "fr", "de-de", "zh-hant", "zh-Hant-TW", "zh", "i"];
const BAD_HEADS: [&str; 7] = ["*/plain", "text", "gz/ip", "en-*", "e1", "\u{e9}", "text/"];

const AVAILABLE_MEDIA: [&str; 18] = [
    "text/plain",
    "text/plain;format=flowed",
    "text/plain;format=fixed",
    "text/html",
    "text/html;level=1",
    "text/html;level=3",
    "image/jpeg",
    "image/png",
    "audio/basic",
    "audio/x-wav",
    "TEXT/Plain;Format=flowed",
    "text/plain;charset=UTF-8",
    "text/plain;charset=\"utf-8\"",
    "application/json",
    "text/plain;format=\"flowed\";level=1",
    "text/plain;a=\"x,y\"",
    "x-y.z+w/vnd.a+json",
    "text/plain;a=\"é😀\"",
];
const AVAILABLE_CODINGS: [&str; 9] =
    ["gzip", "GZIP", "compress", "deflate", "br", "identity", "IDENTITY", "zstd", "x-gzip"];
const AVAILABLE_LANGUAGES: [&str; 11] =
    ["en", "en-US", "en-gb", "EN-GB", "da", "fr", "fr-CA", "de-DE-1996", "zh-Hant-TW", "i", "x"];
const BAD_AVAILABLE: [&str; 8] = ["*", "text/*", "*/*", "text/plain;q=1", "", "en-", "a b", "a/b, c/d"];

fn ows(rng: &mut Rng) -> &'static str {
    rng.pick(&["", "", "", " ", "\t", " \t "])
}

fn q_param(rng: &mut Rng) -> String {
    let name = rng.pick(&["q", "q", "q", "Q"]);
    let value = if rng.below(12) == 0 { rng.pick(&BAD_Q) } else { rng.pick(&QS) };
    format!("{name}={value}")
}

fn element(rng: &mut Rng, h: Header) -> String {
    let mut e = match (h, rng.below(30)) {
        (_, 0) => rng.pick(&BAD_HEADS).to_string(),
        (Header::Accept, _) => format!("{}/{}", rng.pick(&TYPES), rng.pick(&SUBTYPES)),
        (Header::Encoding, _) => rng.pick(&CODINGS).to_string(),
        (Header::Language, _) => rng.pick(&LANGUAGES).to_string(),
    };
    let params = match h {
        Header::Accept => rng.below(4),
        _ => rng.below(3) / 2 + rng.below(8) / 7,
    };
    for _ in 0..params {
        let p = match (h, rng.below(20)) {
            (_, 0) => String::new(),
            (Header::Accept, n) if n > 6 => format!("{}={}", rng.pick(&PARAM_NAMES), rng.pick(&PARAM_VALUES)),
            (_, 1) => format!("level={}", rng.pick(&PARAM_VALUES)),
            _ => q_param(rng),
        };
        e = format!("{e}{};{}{p}", ows(rng), ows(rng));
    }
    e
}

/// A field from the grammar: elements, empty ones, OWS, edge commas.
fn field(rng: &mut Rng, h: Header) -> String {
    let n = rng.below(6);
    let mut parts: Vec<String> =
        (0..n).map(|_| if rng.below(10) == 0 { String::new() } else { element(rng, h) }).collect();
    if rng.below(8) == 0 {
        parts.insert(0, String::new());
    }
    if rng.below(8) == 0 {
        parts.push(String::new());
    }
    let mut out = ows(rng).to_string();
    for (i, p) in parts.iter().enumerate() {
        if i > 0 {
            out.push_str(&format!("{},{}", ows(rng), ows(rng)));
        }
        out.push_str(p);
    }
    out.push_str(ows(rng));
    out
}

/// Absent one time in ten, empty one in twenty.
fn header(rng: &mut Rng, h: Header) -> Option<String> {
    match rng.below(20) {
        0 | 1 => None,
        2 => Some(String::new()),
        _ => Some(field(rng, h)),
    }
}

fn available(rng: &mut Rng, h: Header) -> Vec<String> {
    let pool: &[&str] = match h {
        Header::Accept => &AVAILABLE_MEDIA,
        Header::Encoding => &AVAILABLE_CODINGS,
        Header::Language => &AVAILABLE_LANGUAGES,
    };
    // Small pools now and then, so duplicates and ties are common.
    let width = if rng.below(3) == 0 { 3 } else { pool.len() };
    let n = rng.below(8);
    (0..n)
        .map(|_| if rng.below(40) == 0 { rng.pick(&BAD_AVAILABLE) } else { pool[rng.below(width as u64) as usize] })
        .map(String::from)
        .collect()
}

const HEADERS: [Header; 3] = [Header::Accept, Header::Encoding, Header::Language];

/// Fields to mutate: the RFC's and a few drawn ones.
fn mutation_bases(h: Header) -> Vec<String> {
    let mut out: Vec<String> = match h {
        Header::Accept => vec![
            PRECEDENCE.into(),
            TABLE.into(),
            "audio/*; q=0.2, audio/basic".into(),
            r#"text/plain;format="a,b\"c";q=0.5, */*;q=0"#.into(),
        ],
        Header::Encoding => ENCODING_EXAMPLES.iter().filter(|f| !f.is_empty()).map(|f| f.to_string()).collect(),
        Header::Language => vec![LANGUAGE_EXAMPLE.into(), "en-US, *;q=0.1, fr;Q=0".into()],
    };
    let mut rng = Rng::new(0x6e60 + h as u64);
    while out.len() < 10 {
        let f = field(&mut rng, h);
        if f.is_ascii() && f.len() > 8 && parses(h, &f).0.starts_with("Ok") {
            out.push(f);
        }
    }
    out
}

const MUTATIONS: [char; 21] = [
    ',', ';', '=', '"', '\\', '/', '*', ' ', '\t', 'q', 'Q', '0', '1', '.', '-', 'a', '\u{1}', '\u{7f}', '\r', '(', 'é',
];

/// Every one-byte deletion, replacement, and insertion of `base` (ASCII,
/// so a byte is a character) over `MUTATIONS`.
fn mutations(base: &str) -> Vec<String> {
    let mut out = Vec::new();
    for i in 0..=base.len() {
        if i < base.len() {
            out.push(format!("{}{}", &base[..i], &base[i + 1..]));
        }
        for c in MUTATIONS {
            if i < base.len() {
                out.push(format!("{}{c}{}", &base[..i], &base[i + 1..]));
            }
            out.push(format!("{}{c}{}", &base[..i], &base[i..]));
        }
    }
    out
}

fn compare(h: Header, field: Option<&str>, available: &[String]) {
    if let Some(f) = field {
        let (model, reference) = parses(h, f);
        assert_eq!(model, reference, "{h:?} parse {f:?}");
    }
    let model = model_rank(h, field, available);
    assert_eq!(model, reference_rank(h, field, available), "{h:?} rank {field:?} over {available:?}");
    assert_eq!(
        model_choose(h, field, available),
        idiomatic::choose(model),
        "{h:?} choose {field:?} over {available:?}"
    );
}

#[test]
fn constrained_rust_is_the_idiomatic_rules() {
    let mut rng = Rng::new(0x9110);
    let mut compared = 0;
    let mut outcomes = [0usize; 3];
    for h in HEADERS {
        for _ in 0..4000 {
            let f = header(&mut rng, h);
            let avail = available(&mut rng, h);
            compare(h, f.as_deref(), &avail);
            match model_rank(h, f.as_deref(), &avail) {
                Ok(v) if v.is_empty() => outcomes[1] += 1,
                Ok(_) => outcomes[0] += 1,
                Err(_) => outcomes[2] += 1,
            }
            compared += 1;
        }
        let pool = match h {
            Header::Accept => strings(&AVAILABLE_MEDIA),
            Header::Encoding => strings(&AVAILABLE_CODINGS),
            Header::Language => strings(&AVAILABLE_LANGUAGES),
        };
        for base in mutation_bases(h) {
            for m in mutations(&base) {
                compare(h, Some(&m), &pool);
                compared += 1;
            }
        }
    }
    // Every kind of outcome is well represented.
    assert!(outcomes.iter().all(|&n| n > 800), "{outcomes:?}");
    assert!(compared > 50_000, "{compared}");
}

// ---- Rust and TS ----

#[test]
fn negotiate_matches_rust() {
    let mut rng = Rng::new(0x406);
    let cases = support::cases(|cases| {
        // The RFC's fields.
        let rows: Vec<String> = TABLE_ROWS.iter().map(|(r, _)| r.to_string()).collect();
        let media = strings(&["image/png", "text/html", "text/plain", "text/plain;format=flowed", "audio/basic"]);
        for f in [PRECEDENCE, TABLE, "audio/*; q=0.2, audio/basic"] {
            cases.push(case!(negotiate::parse_accept(f)));
            cases.push(case!(negotiate::rank_media(Some(f), &rows)));
            cases.push(case!(negotiate::rank_media(Some(f), &media)));
            cases.push(case!(negotiate::choose_media(Some(f), &media)));
        }
        let codings = strings(&["identity", "gzip", "compress", "br", "GZIP", "IDENTITY"]);
        let extra = ["identity;q=0", "*;q=0", "*;q=0, identity;q=0.1", "gzip;q=0.001", "gzip;q=0.5, gzip", "x-gzip"];
        for f in ENCODING_EXAMPLES.iter().chain(&extra) {
            cases.push(case!(negotiate::parse_accept_encoding(f)));
            cases.push(case!(negotiate::rank_encodings(Some(*f), &codings)));
            cases.push(case!(negotiate::choose_encoding(Some(*f), &codings[3..4])));
        }
        cases.push(case!(negotiate::rank_encodings(None::<&str>, &codings)));
        let tags = strings(&["en-US", "en-GB", "da", "fr", "en", "en-us"]);
        for f in [LANGUAGE_EXAMPLE, "*;q=0.1, fr;q=0", "", "en-*"] {
            cases.push(case!(negotiate::parse_accept_language(f)));
            cases.push(case!(negotiate::rank_languages(Some(f), &tags)));
            cases.push(case!(negotiate::choose_language(Some(f), &tags[3..4])));
        }
        cases.push(case!(negotiate::rank_languages(None::<&str>, &tags)));
        for (v, _) in GOOD_Q {
            cases.push(case!(negotiate::parse_accept_encoding(&format!("gzip;Q={v}"))));
        }
        for v in BAD_Q {
            cases.push(case!(negotiate::parse_accept_encoding(&format!("gzip;q={v}"))));
        }
        // Malformed offsets after multibyte text: the scanner indexes bytes.
        for f in [
            "a/b;x=\"é\" y",
            "a/b;x=\"😀\";y=\"\\é\";q=2",
            "a/b;x=\"é😀\", */c",
            "a/b;x=\"\\😀\"z",
            "a/b;x=\"é",
            "a/b;x=\"é\\",
            "a/b;x=\"é\u{1}\"",
            "a/b;x=\"é\\\u{7f}\"",
            "é/b",
            "a/b;é=1",
            "a/b;x=é",
            "a/b, ,é",
        ] {
            cases.push(case!(negotiate::parse_accept(f)));
            cases.push(case!(negotiate::rank_media(Some(f), &media)));
        }
        for bad in BAD_AVAILABLE {
            let avail = strings(&["text/plain", bad]);
            cases.push(case!(negotiate::rank_media(Some("*/*"), &avail)));
            cases.push(case!(negotiate::rank_encodings(None::<&str>, &strings(&["gzip", bad]))));
            cases.push(case!(negotiate::rank_languages(Some("*"), &strings(&["en", bad]))));
        }
        // Ties: duplicates in the available list under fields that tie.
        let dup_media =
            strings(&["text/plain", "text/html", "text/plain", "TEXT/PLAIN", "text/plain;format=flowed", "text/html"]);
        let dup_codings = strings(&["gzip", "br", "GZIP", "identity", "br", "identity"]);
        let dup_tags = strings(&["en", "en-US", "EN", "en-us", "da", "en"]);
        for f in ["*/*", "text/*;q=0.5, */*;q=0.5", "text/plain;q=0.5, text/*", "text/html;q=0, */*"] {
            cases.push(case!(negotiate::rank_media(Some(f), &dup_media)));
        }
        for f in ["*", "gzip, br", "*;q=0.5, br;q=0.5", "identity;q=0, *", "gzip;q=0"] {
            cases.push(case!(negotiate::rank_encodings(Some(f), &dup_codings)));
        }
        for f in ["*", "en, da", "en-us;q=0.5, en;q=0.5", "en;q=0, *"] {
            cases.push(case!(negotiate::rank_languages(Some(f), &dup_tags)));
        }
        // Drawn fields: each parsed, ranked, and chosen.
        for h in HEADERS {
            for _ in 0..140 {
                let f = header(&mut rng, h);
                let avail = available(&mut rng, h);
                let fs = f.as_deref();
                match h {
                    Header::Accept => {
                        cases.push(case!(negotiate::parse_accept(fs.unwrap_or_default())));
                        cases.push(case!(negotiate::rank_media(fs, &avail)));
                        cases.push(case!(negotiate::choose_media(fs, &avail)));
                    }
                    Header::Encoding => {
                        cases.push(case!(negotiate::parse_accept_encoding(fs.unwrap_or_default())));
                        cases.push(case!(negotiate::rank_encodings(fs, &avail)));
                        cases.push(case!(negotiate::choose_encoding(fs, &avail)));
                    }
                    Header::Language => {
                        cases.push(case!(negotiate::parse_accept_language(fs.unwrap_or_default())));
                        cases.push(case!(negotiate::rank_languages(fs, &avail)));
                        cases.push(case!(negotiate::choose_language(fs, &avail)));
                    }
                }
            }
            // Mutations of each base: every 89th, so offsets spread out.
            for (k, base) in mutation_bases(h).iter().enumerate() {
                for m in mutations(base).iter().skip(k).step_by(89) {
                    match h {
                        Header::Accept => cases.push(case!(negotiate::parse_accept(m.as_str()))),
                        Header::Encoding => cases.push(case!(negotiate::parse_accept_encoding(m.as_str()))),
                        Header::Language => cases.push(case!(negotiate::parse_accept_language(m.as_str()))),
                    }
                }
            }
        }
    });
    assert!((1000..2500).contains(&cases.len()), "{} cases", cases.len());
    support::assert_equivalent("negotiate", negotiate::SOURCE, &cases);
}
