//! `examples/geo`, Google's Encoded Polyline Algorithm Format at precision 5
//! and 6, and Geohash, on `f64` with no float methods:
//!
//! - Google's worked examples come out as published (the three points and
//!   -179.9832104 alone), and its text decodes back to the same doubles;
//!   precision 6 round-trips; `ezs42` is the published cell and
//!   (57.64911, 10.40744) at length 11 is `u4pruydqqvj`; every error the
//!   model's header lists comes out where it says;
//! - the same rules in idiomatic Rust (`idiomatic`, the line count design/07
//!   §2 compares against: `f64::round`, `as i64`, `as f64`, `is_nan`,
//!   iterators) agree, floats bit for bit (compared through `Debug`, which
//!   prints the shortest text that reads back to the same bits, so `-0.0`
//!   and `0.0` differ), on: coordinates whose scaled value is a half unit
//!   or one to four ulps either side, decimal texts ending in 5, ±0.0,
//!   subnormals, the range ends and one ulp past them, NaN and ±infinity;
//!   seeded random point lists, both precisions, encoded, decoded both ways
//!   and every prefix of the text; values around the 32-bit accumulator
//!   limit (2^30, 2^31, 2^32 after zigzag); every one- and two-byte text
//!   over an alphabet reaching past 63..=126; every geohash length 1..=12
//!   over a grid of points on cell boundaries and one ulp off them, and
//!   every hash so made decoded back. No last-bit difference is expected:
//!   `split_magnitude` subtracts a power of two `p` from a value in
//!   `[p, 2p)`, which is exact, so the fraction it compares with 0.5 is the
//!   exact one `f64::round` sees; `int_to_f64` adds powers of two below
//!   2^53, exact as `as f64`; geohash midpoints are dyadic;
//! - the generated package agrees with Rust on a sample of each, `-0.0`,
//!   half units and errors over-represented.

use std::fmt::Debug;

use crate::support::{self, Js, Rng};

purecrate_canon::fixture!(mod geo = "../../../examples/geo/src/lib.rs");

use geo::{GeoError, LatLng, Precision};

/// Both encodings as one would write them with float methods and casts,
/// under the model's scope (precision 5 or 6, lowercase geohash of 1..=12,
/// the same errors in the same order). Not converted; the reference only.
/// Types carry the constrained side's names, so `Debug` compares them.
mod idiomatic {
    const BASE32: &[u8] = b"0123456789bcdefghjkmnpqrstuvwxyz";

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum GeoError {
        NotANumber,
        Infinite,
        LatitudeOutOfRange,
        LongitudeOutOfRange,
        InvalidChar(usize),
        Truncated,
        OddValueCount,
        Overflow(usize),
        BadLength,
        EmptyGeohash,
        GeohashTooLong,
        BadGeohashChar(usize),
    }
    use GeoError::*;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum Precision {
        E5,
        E6,
    }

    impl Precision {
        fn factor(self) -> f64 {
            match self {
                Precision::E5 => 1e5,
                Precision::E6 => 1e6,
            }
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq)]
    pub struct LatLng {
        lat: f64,
        lng: f64,
    }

    fn finite(x: f64) -> Result<f64, GeoError> {
        match x {
            _ if x.is_nan() => Err(NotANumber),
            _ if x.is_infinite() => Err(Infinite),
            _ => Ok(x),
        }
    }

    impl LatLng {
        pub fn new(lat: f64, lng: f64) -> Result<LatLng, GeoError> {
            let (lat, lng) = (finite(lat)?, finite(lng)?);
            if !(-90.0..=90.0).contains(&lat) {
                return Err(LatitudeOutOfRange);
            }
            if !(-180.0..=180.0).contains(&lng) {
                return Err(LongitudeOutOfRange);
            }
            Ok(LatLng { lat, lng })
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq)]
    pub struct GeohashCell {
        min_lat: f64,
        max_lat: f64,
        min_lng: f64,
        max_lng: f64,
    }

    impl GeohashCell {
        pub fn centre(&self) -> LatLng {
            LatLng { lat: (self.min_lat + self.max_lat) / 2.0, lng: (self.min_lng + self.max_lng) / 2.0 }
        }
    }

    /// Zigzag, then 5-bit chunks from the low end, 0x20 on all but the last.
    pub fn encode_value(v: i64) -> String {
        let mut u = if v < 0 { !(v << 1) } else { v << 1 };
        let mut s = String::new();
        while u >= 0x20 {
            s.push(char::from(((u & 0x1f) | 0x20) as u8 + 63));
            u >>= 5;
        }
        s.push(char::from(u as u8 + 63));
        s
    }

    pub fn encode_coordinate(value: f64, precision: Precision) -> Result<String, GeoError> {
        if !(-180.0..=180.0).contains(&finite(value)?) {
            return Err(LongitudeOutOfRange);
        }
        Ok(encode_value((value * precision.factor()).round() as i64))
    }

    pub fn encode_polyline(points: &[LatLng], precision: Precision) -> String {
        let f = precision.factor();
        let mut prev = (0, 0);
        points
            .iter()
            .map(|p| {
                let (lat, lng) = ((p.lat * f).round() as i64, (p.lng * f).round() as i64);
                let s = encode_value(lat - prev.0) + &encode_value(lng - prev.1);
                prev = (lat, lng);
                s
            })
            .collect()
    }

    pub fn decode_polyline(text: &str, precision: Precision) -> Result<Vec<LatLng>, GeoError> {
        let (mut values, mut acc, mut shift) = (Vec::new(), 0i64, 0u32);
        for (i, b) in text.bytes().enumerate() {
            if !(63..=126).contains(&b) {
                return Err(InvalidChar(i));
            }
            let chunk = i64::from(b - 63);
            acc |= (chunk & 0x1f) << shift;
            // The model tests the shift before the OR; both name this byte.
            if shift > 30 || acc > i64::from(u32::MAX) {
                return Err(Overflow(i));
            }
            if chunk & 0x20 != 0 {
                shift += 5;
                continue;
            }
            values.push(if acc & 1 != 0 { !(acc >> 1) } else { acc >> 1 });
            (acc, shift) = (0, 0);
        }
        if shift != 0 {
            return Err(Truncated);
        }
        if values.len() % 2 != 0 {
            return Err(OddValueCount);
        }
        let f = precision.factor();
        let (mut lat, mut lng) = (0i64, 0i64);
        values
            .chunks(2)
            .map(|d| {
                (lat, lng) = (lat + d[0], lng + d[1]);
                LatLng::new(lat as f64 / f, lng as f64 / f)
            })
            .collect()
    }

    /// Keeps the upper or the lower half of `range`.
    fn halve(range: &mut (f64, f64), upper: bool) {
        let mid = (range.0 + range.1) / 2.0;
        if upper {
            range.0 = mid;
        } else {
            range.1 = mid;
        }
    }

    pub fn geohash_encode(point: LatLng, length: usize) -> Result<String, GeoError> {
        if !(1..=12).contains(&length) {
            return Err(BadLength);
        }
        let (mut lat, mut lng) = ((-90.0, 90.0), (-180.0, 180.0));
        let bits: Vec<usize> = (0..length * 5)
            .map(|i| {
                let (range, v) = if i % 2 == 0 { (&mut lng, point.lng) } else { (&mut lat, point.lat) };
                let upper = v >= (range.0 + range.1) / 2.0;
                halve(range, upper);
                usize::from(upper)
            })
            .collect();
        Ok(bits.chunks(5).map(|c| char::from(BASE32[c.iter().fold(0, |a, b| a * 2 + b)])).collect())
    }

    pub fn geohash_decode(hash: &str) -> Result<GeohashCell, GeoError> {
        if hash.is_empty() {
            return Err(EmptyGeohash);
        }
        let (mut lat, mut lng) = ((-90.0, 90.0), (-180.0, 180.0));
        for (n, (pos, c)) in hash.char_indices().enumerate() {
            if n == 12 {
                return Err(GeohashTooLong);
            }
            let idx = BASE32.iter().position(|&a| char::from(a) == c).ok_or(BadGeohashChar(pos))?;
            for bit in 0..5 {
                let upper = (idx >> (4 - bit)) & 1 == 1;
                halve(if (n * 5 + bit) % 2 == 0 { &mut lng } else { &mut lat }, upper);
            }
        }
        Ok(GeohashCell { min_lat: lat.0, max_lat: lat.1, min_lng: lng.0, max_lng: lng.1 })
    }
}

const PRECISIONS: [Precision; 2] = [Precision::E5, Precision::E6];

fn reference(p: Precision) -> idiomatic::Precision {
    match p {
        Precision::E5 => idiomatic::Precision::E5,
        Precision::E6 => idiomatic::Precision::E6,
    }
}

fn factor(p: Precision) -> f64 {
    match p {
        Precision::E5 => 1e5,
        Precision::E6 => 1e6,
    }
}

fn dbg(x: &dyn Debug) -> String {
    format!("{x:?}")
}

fn point(lat: f64, lng: f64) -> LatLng {
    LatLng::new(lat, lng).unwrap_or_else(|e| panic!("({lat:?}, {lng:?}): {e:?}"))
}

fn points(pairs: &[(f64, f64)]) -> Vec<LatLng> {
    pairs.iter().map(|&(lat, lng)| point(lat, lng)).collect()
}

/// The same points on the reference side.
fn mirrored(ps: &[LatLng]) -> Vec<idiomatic::LatLng> {
    ps.iter().map(|p| idiomatic::LatLng::new(p.lat(), p.lng()).expect("in range")).collect()
}

/// `x` moved `k` ulps (`next_up` / `next_down`, through zero).
fn ulps(x: f64, k: i32) -> f64 {
    (0..k.unsigned_abs()).fold(x, |y, _| if k > 0 { y.next_up() } else { y.next_down() })
}

const GOOGLE: [(f64, f64); 3] = [(38.5, -120.2), (40.7, -120.95), (43.252, -126.453)];
const GOOGLE_TEXT: &str = "_p~iF~ps|U_ulLnnqC_mqNvxq`@";

#[test]
fn the_published_examples_come_out_as_published() {
    use GeoError::*;
    let google = points(&GOOGLE);
    assert_eq!(geo::encode_polyline(&google, Precision::E5), GOOGLE_TEXT);
    assert_eq!(geo::decode_polyline(GOOGLE_TEXT, Precision::E5), Ok(google.clone()), "the same doubles back");
    assert_eq!(geo::encode_coordinate(-179.9832104, Precision::E5), Ok("`~oia@".to_string()));
    assert_eq!(geo::encode_coordinate(-0.0, Precision::E5), Ok("?".to_string()));
    assert_eq!(geo::encode_coordinate(0.000005, Precision::E5), Ok("A".to_string()), "0.5 rounds to 1");
    assert_eq!(geo::encode_coordinate(-0.000005, Precision::E5), Ok("@".to_string()), "-0.5 rounds to -1");
    // Not `floor(x + 0.5)`: a scaled value one ulp below 0.5 rounds to 0.
    let below_half = (-4..=4).map(|k| ulps(5e-6, k)).find(|x| x * 1e5 == 0.5f64.next_down()).expect("one lands there");
    assert_eq!(geo::encode_coordinate(below_half, Precision::E5), Ok("?".to_string()));
    assert_eq!(geo::encode_polyline(&[], Precision::E5), "");
    assert_eq!(geo::decode_polyline("", Precision::E6), Ok(vec![]));

    // Precision 6: the decoded points are the inputs rounded to 1e-6, and
    // they encode to the same text.
    let fine = points(&[(38.5, -120.2), (-33.8688197, 151.2092955), (89.9999995, -179.9999995), (-0.0, 0.0)]);
    let text = geo::encode_polyline(&fine, Precision::E6);
    let decoded = geo::decode_polyline(&text, Precision::E6).expect("decodes");
    let rounded: Vec<LatLng> =
        fine.iter().map(|p| point((p.lat() * 1e6).round() / 1e6 + 0.0, (p.lng() * 1e6).round() / 1e6 + 0.0)).collect();
    assert_eq!(dbg(&decoded), dbg(&rounded));
    assert_eq!(geo::encode_polyline(&decoded, Precision::E6), text);
    assert_eq!(decoded[3].lat().to_bits(), 0, "a decoded 0 is +0.0");
    assert_eq!(
        geo::decode_polyline(&geo::encode_polyline(&google, Precision::E6), Precision::E5),
        Err(LatitudeOutOfRange)
    );
    let east = points(&[(1.0, 100.0)]);
    assert_eq!(
        geo::decode_polyline(&geo::encode_polyline(&east, Precision::E6), Precision::E5),
        Err(LongitudeOutOfRange)
    );

    let cell = geo::geohash_decode("ezs42").expect("a cell");
    assert_eq!((cell.min_lat(), cell.max_lat()), (42.5830078125, 42.626953125));
    assert_eq!((cell.min_lng(), cell.max_lng()), (-5.625, -5.5810546875));
    assert_eq!(cell.centre(), point(42.60498046875, -5.60302734375));
    assert_eq!(geo::geohash_encode(point(42.6, -5.6), 5), Ok("ezs42".to_string()));
    assert_eq!(geo::geohash_encode(point(57.64911, 10.40744), 11), Ok("u4pruydqqvj".to_string()));
    assert_eq!(geo::geohash_encode(point(-0.0, -0.0), 1), Ok("s".to_string()), "-0.0 takes the upper half");
    assert_eq!(geo::geohash_encode(point(90.0, 180.0), 12), Ok("zzzzzzzzzzzz".to_string()));
    assert_eq!(geo::geohash_encode(point(-90.0, -180.0), 12), Ok("000000000000".to_string()));

    assert_eq!(LatLng::new(f64::NAN, 0.0), Err(NotANumber));
    assert_eq!(LatLng::new(f64::INFINITY, f64::NAN), Err(Infinite), "latitude first");
    assert_eq!(LatLng::new(0.0, f64::NEG_INFINITY), Err(Infinite));
    assert_eq!(LatLng::new(90.0f64.next_up(), 0.0), Err(LatitudeOutOfRange));
    assert_eq!(LatLng::new(0.0, (-180.0f64).next_down()), Err(LongitudeOutOfRange));
    assert_eq!(LatLng::new(-0.0, -0.0).map(|p| p.lat().to_bits()), Ok((-0.0f64).to_bits()), "-0.0 is kept");
    assert_eq!(geo::encode_coordinate(f64::NAN, Precision::E6), Err(NotANumber));
    assert_eq!(geo::encode_coordinate(-f64::INFINITY, Precision::E6), Err(Infinite));
    assert_eq!(geo::encode_coordinate(180.5, Precision::E5), Err(LongitudeOutOfRange));
    assert_eq!(geo::encode_coordinate(-90.5, Precision::E5).map(|s| s.len()), Ok(5), "any axis up to 180");
    assert_eq!(geo::decode_polyline("?!", Precision::E5), Err(InvalidChar(1)));
    assert_eq!(geo::decode_polyline("??é", Precision::E5), Err(InvalidChar(2)));
    assert_eq!(geo::decode_polyline("??_", Precision::E5), Err(Truncated));
    assert_eq!(geo::decode_polyline("???", Precision::E5), Err(OddValueCount));
    assert_eq!(geo::decode_polyline("~~~~~~~", Precision::E5), Err(Overflow(6)), "the accumulator past 32 bits");
    assert_eq!(geo::decode_polyline("______`?", Precision::E5), Err(Overflow(7)), "a shift past 30");
    assert_eq!(geo::geohash_encode(point(0.0, 0.0), 0), Err(BadLength));
    assert_eq!(geo::geohash_encode(point(0.0, 0.0), 13), Err(BadLength));
    assert_eq!(geo::geohash_decode(""), Err(EmptyGeohash));
    assert_eq!(geo::geohash_decode("ezs42ezs42ezs"), Err(GeohashTooLong));
    assert_eq!(geo::geohash_decode("ezs42ezs42ez!"), Err(GeohashTooLong), "length before the character");
    for bad in ["A", "a", "i", "l", "o", "E", " ", "é"] {
        assert_eq!(geo::geohash_decode(&format!("ez{bad}")), Err(BadGeohashChar(2)), "{bad:?}");
    }
}

/// Coordinates whose scaled value is a half unit, or as close to one as a
/// double lands, at either precision, with one to four ulps either side.
fn half_units() -> Vec<f64> {
    let ns: [i64; 14] =
        [0, 1, 2, 7, 12, 99, 12_345, 4_999_999, 8_999_999, 9_000_000, 17_998_321, 17_999_999, 89_999_999, 179_999_999];
    let mut out = Vec::new();
    for f in [1e5, 1e6] {
        for n in ns {
            let near = (n as f64 + 0.5) / f;
            if near > 180.0 {
                continue;
            }
            for k in -4..=4 {
                let x = ulps(near, k);
                out.extend([x, -x]);
            }
        }
    }
    out
}

/// Decimal texts ending in 5 at the sixth and seventh place, read as
/// doubles: some land above the half unit, some below.
fn decimal_fives(rng: &mut Rng) -> Vec<f64> {
    let mut out = Vec::new();
    for _ in 0..400 {
        let whole = rng.below(180);
        let places = rng.pick(&[5usize, 6]);
        let frac = rng.below(10u64.pow(places as u32));
        let text = format!("{whole}.{frac:0places$}5");
        let x: f64 = text.parse().expect("a decimal");
        out.extend([x, -x]);
    }
    out
}

/// Every edge a coordinate meets: zeros, subnormals, the range ends and one
/// ulp either side, the largest doubles, NaN and the infinities.
fn edges() -> Vec<f64> {
    let mut out = vec![0.0, -0.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY, f64::MAX, f64::MIN];
    for x in [5e-324, 1e-310, f64::MIN_POSITIVE, 1e-300, 1e-7, 4.9e-6, 5e-6, 5e-7, 0.5, 1.0] {
        out.extend([x, -x]);
    }
    for end in [90.0, 180.0, 89.999995, 179.999995, 89.9999995, 179.9999995] {
        for k in -2..=2 {
            out.extend([ulps(end, k), -ulps(end, k)]);
        }
    }
    out
}

fn coordinates() -> Vec<f64> {
    let mut out = edges();
    out.extend(half_units());
    out.extend(decimal_fives(&mut Rng::new(0x9e0)));
    out
}

/// A coordinate in `-limit..=limit`: a micro-degree, a half unit at one of
/// the precisions, or an edge.
fn coordinate(rng: &mut Rng, limit: f64) -> f64 {
    let span = limit as u64 * 2_000_000 + 1;
    let x = match rng.below(8) {
        0 => (rng.below(limit as u64 * 200_000) as f64 - limit * 1e5 + 0.5) / 1e5,
        1 => (rng.below(limit as u64 * 2_000_000) as f64 - limit * 1e6 + 0.5) / 1e6,
        2 => rng.pick(&[0.0, -0.0, limit, -limit, 5e-324, -5e-6, 5e-7]),
        _ => (rng.below(span) as f64 - limit * 1e6) / 1e6,
    };
    x.clamp(-limit, limit)
}

/// Seeded point lists of 0 to 40 points.
fn point_lists() -> Vec<Vec<LatLng>> {
    let mut rng = Rng::new(0x9e01);
    (0..300)
        .map(|_| {
            let n = rng.below(41) as usize;
            (0..n).map(|_| point(coordinate(&mut rng, 90.0), coordinate(&mut rng, 180.0))).collect()
        })
        .collect()
}

/// Texts around the 32-bit accumulator: values whose zigzag form is near
/// 2^30, 2^31 and 2^32, alone and as a latitude or longitude.
fn wide_texts() -> Vec<String> {
    let mut out = Vec::new();
    for base in [1i64 << 29, 1 << 30, 1 << 31] {
        for v in [base - 1, base, base + 1] {
            for v in [v, -v] {
                let e = idiomatic::encode_value(v);
                out.extend([e.clone(), format!("{e}?"), format!("?{e}"), format!("{e}{e}")]);
            }
        }
    }
    for v in [9_000_000i64, 9_000_001, 18_000_000, 18_000_001, 90_000_000, 180_000_000, 180_000_001] {
        for v in [v, -v] {
            let e = idiomatic::encode_value(v);
            out.extend([format!("{e}?"), format!("?{e}")]);
        }
    }
    out.extend(["~~~~~~~", "~~~~~~^", "~~~~~~C", "~~~~~~B", "______`?", "______?", "_______"].map(String::from));
    out
}

/// Every one- and two-byte text over an alphabet reaching past 63..=126,
/// and every prefix of Google's text and of a few encoded lists.
fn malformed_texts() -> Vec<String> {
    let alphabet = ['>', '?', '@', 'A', '_', '`', '~', '\u{7f}', ' ', 'é'];
    let mut out = Vec::new();
    for a in alphabet {
        out.push(a.to_string());
        for b in alphabet {
            out.push(format!("{a}{b}"));
        }
    }
    let lists = point_lists();
    let mut whole = vec![GOOGLE_TEXT.to_string()];
    whole.extend(
        lists.iter().filter(|l| (2..6).contains(&l.len())).take(4).map(|l| geo::encode_polyline(l, Precision::E6)),
    );
    for text in whole {
        out.extend((0..text.len()).map(|n| text[..n].to_string()));
    }
    out
}

/// Cell boundaries on an axis, `lo + (hi - lo) * k / 2^n`, and one ulp
/// either side of each (kept in range), with -0.0.
fn boundaries(lo: f64, hi: f64) -> Vec<f64> {
    let mut out = vec![-0.0];
    for n in [1u32, 2, 3, 5, 8, 13, 21, 30] {
        let parts = (1u64 << n) as f64;
        for k in [0u64, 1, 3, (1 << n) / 3, (1 << n) - 1, 1 << n] {
            let b = lo + (hi - lo) * k as f64 / parts;
            out.extend([b, b.next_up(), b.next_down()].into_iter().filter(|x| (lo..=hi).contains(x)));
        }
    }
    out.sort_by(f64::total_cmp);
    out.dedup_by(|a, b| a.to_bits() == b.to_bits());
    out
}

fn grid() -> Vec<LatLng> {
    let lats = boundaries(-90.0, 90.0);
    let lngs = boundaries(-180.0, 180.0);
    let mut out: Vec<LatLng> = lats.iter().flat_map(|&lat| lngs.iter().map(move |&lng| point(lat, lng))).collect();
    out.extend([point(42.6, -5.6), point(57.64911, 10.40744), point(-33.8688197, 151.2092955)]);
    out
}

fn malformed_hashes() -> Vec<String> {
    let mut out: Vec<String> =
        ["", "A", "a", "i", "l", "o", "é", "eé", "ezs4é", "ezs42", "0", "z", "u4pruydqqvj"].map(String::from).into();
    out.extend(
        ["zzzzzzzzzzzz", "000000000000", "ezs42ezs42ez", "ezs42ezs42ezs", "ezs42ezs42ez!", "ezs42ezs42e!"]
            .map(String::from),
    );
    out
}

#[test]
fn constrained_rust_is_the_idiomatic_rules() {
    let coords = coordinates();
    let mut halves = 0;
    for &x in &coords {
        for p in PRECISIONS {
            let c = geo::encode_coordinate(x, p);
            assert_eq!(dbg(&c), dbg(&idiomatic::encode_coordinate(x, reference(p))), "encode_coordinate({x:?}, {p:?})");
            halves += usize::from(((x * factor(p)) % 1.0).abs() == 0.5);
        }
    }
    assert!(halves > 50, "{halves} exact half units");
    let finite: Vec<f64> = coords.iter().copied().filter(|x| x.is_finite()).step_by(3).collect();
    for &lat in edges().iter().chain(&finite) {
        for &lng in edges().iter().chain(finite.iter().step_by(7)) {
            let c = LatLng::new(lat, lng);
            assert_eq!(dbg(&c), dbg(&idiomatic::LatLng::new(lat, lng)), "LatLng::new({lat:?}, {lng:?})");
            if let Ok(pt) = c {
                for p in PRECISIONS {
                    let text = geo::encode_polyline(&[pt], p);
                    assert_eq!(text, idiomatic::encode_polyline(&mirrored(&[pt]), reference(p)), "{pt:?} {p:?}");
                }
            }
        }
    }
    let lists = point_lists();
    for list in &lists {
        let mirror = mirrored(list);
        for p in PRECISIONS {
            let text = geo::encode_polyline(list, p);
            assert_eq!(text, idiomatic::encode_polyline(&mirror, reference(p)), "{list:?} {p:?}");
            for q in PRECISIONS {
                let c = geo::decode_polyline(&text, q);
                assert_eq!(dbg(&c), dbg(&idiomatic::decode_polyline(&text, reference(q))), "decode {text:?} {q:?}");
            }
            let back = geo::decode_polyline(&text, p).expect("its own text");
            let f = factor(p);
            let rounded: Vec<LatLng> =
                list.iter().map(|q| point((q.lat() * f).round() / f + 0.0, (q.lng() * f).round() / f + 0.0)).collect();
            assert_eq!(dbg(&back), dbg(&rounded), "round trip {text:?}");
            for n in (0..text.len()).step_by(3) {
                let prefix = &text[..n];
                assert_eq!(
                    dbg(&geo::decode_polyline(prefix, p)),
                    dbg(&idiomatic::decode_polyline(prefix, reference(p)))
                );
            }
        }
    }
    for text in wide_texts().iter().chain(&malformed_texts()) {
        for p in PRECISIONS {
            let c = geo::decode_polyline(text, p);
            assert_eq!(dbg(&c), dbg(&idiomatic::decode_polyline(text, reference(p))), "decode {text:?} {p:?}");
        }
    }
    let mut hashes = 0;
    for pt in grid() {
        let mirror = mirrored(&[pt])[0];
        for length in 0..=13 {
            let h = geo::geohash_encode(pt, length);
            assert_eq!(dbg(&h), dbg(&idiomatic::geohash_encode(mirror, length)), "geohash_encode({pt:?}, {length})");
            if let Ok(h) = h {
                let cell = geo::geohash_decode(&h).expect("its own hash");
                assert_eq!(dbg(&cell), dbg(&idiomatic::geohash_decode(&h).expect("its own hash")), "{h}");
                assert_eq!(dbg(&cell.centre()), dbg(&idiomatic::geohash_decode(&h).expect("decodes").centre()), "{h}");
                assert!(
                    (cell.min_lat()..=cell.max_lat()).contains(&pt.lat())
                        && (cell.min_lng()..=cell.max_lng()).contains(&pt.lng()),
                    "{pt:?} outside {h}"
                );
                hashes += 1;
            }
        }
    }
    assert!(hashes > 50_000, "{hashes} hashes");
    let alphabet: Vec<char> = "0123456789bcdefghjkmnpqrstuvwxyzaAé".chars().collect();
    let short = alphabet
        .iter()
        .map(|a| a.to_string())
        .chain(alphabet.iter().flat_map(|a| alphabet.iter().map(move |b| format!("{a}{b}"))));
    for h in malformed_hashes().into_iter().chain(short) {
        assert_eq!(dbg(&geo::geohash_decode(&h)), dbg(&idiomatic::geohash_decode(&h)), "geohash_decode({h:?})");
        if let Ok(cell) = geo::geohash_decode(&h) {
            assert_eq!(dbg(&cell.centre()), dbg(&idiomatic::geohash_decode(&h).expect("decodes").centre()), "{h}");
        }
    }
}

fn coordinate_case(x: f64, p: Precision) -> support::Case {
    support::run("encode_coordinate", format!("encodeCoordinate({}, {})", x.js(), p.js()), move || {
        geo::encode_coordinate(x, p)
    })
}

/// `LatLng::new` is a method, and the driver prints only what a free
/// function returns: the case borrows `decode_polyline`'s printer, the
/// point as a one-element list.
fn new_case(lat: f64, lng: f64) -> support::Case {
    let call = format!(
        "((r) => (r.kind === \"Ok\" ? {{ kind: \"Ok\", value: [r.value] }} : r))(pkg.LatLng.new({}, {}))",
        lat.js(),
        lng.js()
    );
    support::run("decode_polyline", call, move || LatLng::new(lat, lng).map(|p| vec![p]))
}

/// `GeohashCell::centre`, through `decode_polyline`'s printer as above.
fn centre_case(hash: String) -> support::Case {
    let call = format!(
        "((r) => (r.kind === \"Ok\" ? {{ kind: \"Ok\", value: [pkg.GeohashCell.centre(r.value)] }} : r))(geohashDecode({}))",
        hash.js()
    );
    support::run("decode_polyline", call, move || geo::geohash_decode(&hash).map(|c| vec![c.centre()]))
}

#[test]
fn geo_matches_rust() {
    let lists = point_lists();
    let cases = support::cases(|cases| {
        let google = points(&GOOGLE);
        for p in PRECISIONS {
            cases.push(case!(geo::encode_polyline(google.as_slice(), p)));
            cases.push(case!(geo::decode_polyline(GOOGLE_TEXT, p)));
        }
        cases.push(coordinate_case(-179.9832104, Precision::E5));
        for x in edges().into_iter().chain(half_units().into_iter().step_by(3)) {
            for p in PRECISIONS {
                cases.push(coordinate_case(x, p));
            }
        }
        for x in decimal_fives(&mut Rng::new(0x9e0)).into_iter().step_by(16) {
            cases.push(coordinate_case(x, Precision::E6));
        }
        let specials = [
            0.0,
            -0.0,
            f64::NAN,
            f64::INFINITY,
            f64::NEG_INFINITY,
            90.0,
            90.0f64.next_up(),
            -180.0,
            (-180.0f64).next_down(),
        ];
        for lat in specials {
            for lng in specials {
                cases.push(new_case(lat, lng));
            }
        }
        let zeros = points(&[(-0.0, -0.0), (0.0, -0.0), (-0.0, 0.0), (-0.000005, 0.000005), (-0.0000004, -0.0000005)]);
        for p in PRECISIONS {
            cases.push(case!(geo::encode_polyline(zeros.as_slice(), p)));
        }
        for (k, list) in lists.iter().enumerate().step_by(4) {
            let p = PRECISIONS[k / 4 % 2];
            cases.push(case!(geo::encode_polyline(list.as_slice(), p)));
            let text = geo::encode_polyline(list, p);
            for q in PRECISIONS {
                cases.push(case!(geo::decode_polyline(text.as_str(), q)));
            }
        }
        for text in wide_texts().iter().chain(malformed_texts().iter().step_by(2)) {
            cases.push(case!(geo::decode_polyline(text.as_str(), Precision::E5)));
        }
        let grid = grid();
        for (k, pt) in grid.iter().enumerate().step_by(53) {
            for length in [0, 1, 1 + k % 12, 12, 13] {
                cases.push(case!(geo::geohash_encode(*pt, length)));
            }
            let h = geo::geohash_encode(*pt, 1 + k % 12).expect("a hash");
            cases.push(case!(geo::geohash_decode(h.as_str())));
            cases.push(centre_case(h));
        }
        for h in malformed_hashes() {
            cases.push(case!(geo::geohash_decode(h.as_str())));
            cases.push(centre_case(h));
        }
    });
    assert!((1000..2500).contains(&cases.len()), "{} cases", cases.len());
    support::assert_equivalent("geo", geo::SOURCE, &cases);
}
