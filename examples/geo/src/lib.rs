// What the model checks and leaves out
//
// Two encodings of geographic points, written from their published
// descriptions: Google's Encoded Polyline Algorithm Format and Geohash.
//
// Coordinates
// - A `LatLng` exists only through `LatLng::new`, which refuses NaN
//   (`NotANumber`), ±infinity (`Infinite`), a latitude outside -90..=90 and a
//   longitude outside -180..=180. Every function below takes `LatLng`, so no
//   NaN or out-of-range value reaches the algorithms.
// - -0.0 is accepted and treated as 0: it scales and rounds to 0, encodes as
//   `?`, and falls in the upper half of every geohash bisection (as 0.0 does,
//   since `-0.0 >= 0.0`).
//
// Encoded Polyline
// - Precision is 5 (Google's) or 6 (`Precision::E5`, `Precision::E6`);
//   nothing else.
// - Steps as published: multiply by 10^precision, round, take the delta from
//   the previous point (the first from 0), shift left one bit, invert when
//   the value is negative, cut 5-bit chunks from the low end, OR 0x20 into
//   all but the last, add 63, emit as ASCII. Latitude before longitude.
// - Rounding is half away from zero on the scaled value, computed exactly:
//   -179.9832104 scales to -17998321.04 and rounds to -17998321; 0.5 rounds
//   to 1 and -0.5 to -1. The subset has no float `round`/`floor` and no
//   float-to-integer conversion, so the integer part is peeled off by
//   subtracting powers of two (exact in IEEE 754) and the remaining fraction
//   is compared with 0.5. This is not `floor(x + 0.5)`, which is off for
//   0.49999999999999994. The scaled value is a double product, so a decimal
//   input that is not exactly representable rounds as its double does.
// - Decoding rejects, with the byte index where it applies: a byte outside
//   63..=126 (`InvalidChar`), text that ends inside a chunk sequence
//   (`Truncated`), an odd number of values (`OddValueCount`), a value whose
//   accumulator would exceed 32 bits (`Overflow`; valid data stays below
//   2^30), and a decoded point outside the coordinate range
//   (`LatitudeOutOfRange` / `LongitudeOutOfRange`; this is what a
//   precision-6 text read as precision 5 usually produces). The empty text
//   decodes to no points.
// - Decoded values are integer / 10^precision in f64, as Google's decoders
//   do; integer to float is composed from powers of two (exact below 2^53).
// - `encode_coordinate` encodes one value on its own (the spec's worked
//   example); it accepts -180..=180 and reports `LongitudeOutOfRange` past
//   that, whichever axis the caller meant.
//
// Geohash
// - Alphabet `0123456789bcdefghjkmnpqrstuvwxyz`, bits interleaved longitude
//   first, each bisection taking the upper half when the value is >= the
//   midpoint. Length 1..=12 for encoding (`BadLength` otherwise).
// - Decoding accepts lowercase only; an uppercase letter, `a`, `i`, `l`,
//   `o` or any other character is `BadGeohashChar` at its byte index. Empty
//   text is `EmptyGeohash`, more than 12 characters `GeohashTooLong`.
// - A decoded cell gives its bounds and its centre (the midpoints).
//
// Left out: precisions other than 5 and 6, polylines with more than two
// dimensions, geohash neighbours, and distance on the sphere.

const BASE32: &str = "0123456789bcdefghjkmnpqrstuvwxyz";
const GEOHASH_MAX_LEN: usize = 12;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Precision {
    E5,
    E6,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LatLng {
    lat: f64,
    lng: f64,
}

impl LatLng {
    pub fn new(lat: f64, lng: f64) -> Result<LatLng, GeoError> {
        check_finite(lat)?;
        check_finite(lng)?;
        if lat < -90.0 || lat > 90.0 {
            return Err(GeoError::LatitudeOutOfRange);
        }
        if lng < -180.0 || lng > 180.0 {
            return Err(GeoError::LongitudeOutOfRange);
        }
        Ok(LatLng { lat, lng })
    }

    pub fn lat(&self) -> f64 {
        self.lat
    }

    pub fn lng(&self) -> f64 {
        self.lng
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
    pub fn min_lat(&self) -> f64 {
        self.min_lat
    }

    pub fn max_lat(&self) -> f64 {
        self.max_lat
    }

    pub fn min_lng(&self) -> f64 {
        self.min_lng
    }

    pub fn max_lng(&self) -> f64 {
        self.max_lng
    }

    pub fn centre(&self) -> LatLng {
        LatLng {
            lat: (self.min_lat + self.max_lat) / 2.0,
            lng: (self.min_lng + self.max_lng) / 2.0,
        }
    }
}

fn check_finite(x: f64) -> Result<(), GeoError> {
    if x != x {
        return Err(GeoError::NotANumber);
    }
    if x - x != 0.0 {
        return Err(GeoError::Infinite);
    }
    Ok(())
}

fn factor(precision: Precision) -> f64 {
    match precision {
        Precision::E5 => 100000.0,
        Precision::E6 => 1000000.0,
    }
}

// The integer part of `m` (0 <= m < 2^41) and the exact remaining fraction.
// Callers pass a scaled, range-checked coordinate (at most 1.8e8).
fn split_magnitude(m: f64) -> (i64, f64) {
    let mut rest = m;
    let mut p: f64 = 1099511627776.0;
    let mut bit: i64 = 1099511627776;
    let mut whole: i64 = 0;
    while bit > 0 {
        if rest >= p {
            rest -= p;
            whole += bit;
        }
        p /= 2.0;
        bit /= 2;
    }
    (whole, rest)
}

fn round_half_away(x: f64) -> i64 {
    let negative = x < 0.0;
    let m = if negative { -x } else { x };
    let (whole, fraction) = split_magnitude(m);
    let r = if fraction >= 0.5 { whole + 1 } else { whole };
    if negative {
        -r
    } else {
        r
    }
}

fn int_to_f64(v: i64) -> f64 {
    let negative = v < 0;
    let mut m = if negative { -v } else { v };
    let mut p: f64 = 1.0;
    let mut out: f64 = 0.0;
    while m > 0 {
        if m % 2 == 1 {
            out += p;
        }
        m /= 2;
        p *= 2.0;
    }
    if negative {
        -out
    } else {
        out
    }
}

// A 6-bit chunk (0..64) plus 63, as the ASCII character.
fn chunk_char(c: i64) -> char {
    let mut b: u8 = 0;
    let mut bit: i64 = 1;
    let mut add: u8 = 1;
    for _ in 0..6u32 {
        if c & bit != 0 {
            b += add;
        }
        bit <<= 1;
        add *= 2;
    }
    char::from(63 + b)
}

fn encode_value(v: i64) -> String {
    let mut u = v << 1;
    if v < 0 {
        u = !u;
    }
    let mut s = String::new();
    while u >= 0x20 {
        s.push(chunk_char((u & 0x1f) | 0x20));
        u >>= 5;
    }
    s.push(chunk_char(u));
    s
}

fn unzigzag(u: i64) -> i64 {
    if u & 1 != 0 {
        !(u >> 1)
    } else {
        u >> 1
    }
}

/// One coordinate value on its own, as the spec's worked example does.
pub fn encode_coordinate(value: f64, precision: Precision) -> Result<String, GeoError> {
    check_finite(value)?;
    if value < -180.0 || value > 180.0 {
        return Err(GeoError::LongitudeOutOfRange);
    }
    Ok(encode_value(round_half_away(value * factor(precision))))
}

pub fn encode_polyline(points: &[LatLng], precision: Precision) -> String {
    let f = factor(precision);
    let mut out = String::new();
    let mut prev_lat: i64 = 0;
    let mut prev_lng: i64 = 0;
    for p in points.iter() {
        let lat = round_half_away(p.lat * f);
        let lng = round_half_away(p.lng * f);
        out.push_str(&encode_value(lat - prev_lat));
        out.push_str(&encode_value(lng - prev_lng));
        prev_lat = lat;
        prev_lng = lng;
    }
    out
}

pub fn decode_polyline(text: &str, precision: Precision) -> Result<Vec<LatLng>, GeoError> {
    let bytes = text.as_bytes();
    let mut values: Vec<i64> = Vec::new();
    let mut acc: i64 = 0;
    let mut shift: u32 = 0;
    let mut i: usize = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if b < 63 || b > 126 {
            return Err(GeoError::InvalidChar(i));
        }
        if shift > 30 {
            return Err(GeoError::Overflow(i));
        }
        let chunk = i64::from(b - 63);
        acc |= (chunk & 0x1f) << shift;
        if acc > 0xFFFF_FFFF {
            return Err(GeoError::Overflow(i));
        }
        if chunk & 0x20 != 0 {
            shift += 5;
        } else {
            values.push(unzigzag(acc));
            acc = 0;
            shift = 0;
        }
        i += 1;
    }
    if shift != 0 {
        return Err(GeoError::Truncated);
    }
    if values.len() % 2 != 0 {
        return Err(GeoError::OddValueCount);
    }
    let f = factor(precision);
    let mut points: Vec<LatLng> = Vec::new();
    let mut lat: i64 = 0;
    let mut lng: i64 = 0;
    let mut k: usize = 0;
    while k < values.len() {
        lat += values[k];
        lng += values[k + 1];
        points.push(LatLng::new(int_to_f64(lat) / f, int_to_f64(lng) / f)?);
        k += 2;
    }
    Ok(points)
}

fn base32_index(c: char) -> Option<u32> {
    let mut i: u32 = 0;
    for a in BASE32.chars() {
        if a == c {
            return Some(i);
        }
        i += 1;
    }
    None
}

pub fn geohash_encode(point: LatLng, length: usize) -> Result<String, GeoError> {
    if length < 1 || length > GEOHASH_MAX_LEN {
        return Err(GeoError::BadLength);
    }
    let alphabet = BASE32.as_bytes();
    let mut lat_lo: f64 = -90.0;
    let mut lat_hi: f64 = 90.0;
    let mut lng_lo: f64 = -180.0;
    let mut lng_hi: f64 = 180.0;
    let mut use_lng = true;
    let mut out = String::new();
    let mut written: usize = 0;
    let mut idx: u32 = 0;
    let mut nbits: u32 = 0;
    while written < length {
        if use_lng {
            let mid = (lng_lo + lng_hi) / 2.0;
            if point.lng >= mid {
                idx = idx * 2 + 1;
                lng_lo = mid;
            } else {
                idx *= 2;
                lng_hi = mid;
            }
        } else {
            let mid = (lat_lo + lat_hi) / 2.0;
            if point.lat >= mid {
                idx = idx * 2 + 1;
                lat_lo = mid;
            } else {
                idx *= 2;
                lat_hi = mid;
            }
        }
        use_lng = !use_lng;
        nbits += 1;
        if nbits == 5 {
            out.push(char::from(alphabet[idx as usize]));
            written += 1;
            idx = 0;
            nbits = 0;
        }
    }
    Ok(out)
}

pub fn geohash_decode(hash: &str) -> Result<GeohashCell, GeoError> {
    if hash.is_empty() {
        return Err(GeoError::EmptyGeohash);
    }
    let mut lat_lo: f64 = -90.0;
    let mut lat_hi: f64 = 90.0;
    let mut lng_lo: f64 = -180.0;
    let mut lng_hi: f64 = 180.0;
    let mut use_lng = true;
    let mut count: usize = 0;
    let mut pos: usize = 0;
    for c in hash.chars() {
        if count == GEOHASH_MAX_LEN {
            return Err(GeoError::GeohashTooLong);
        }
        let idx = match base32_index(c) {
            Some(n) => n,
            None => return Err(GeoError::BadGeohashChar(pos)),
        };
        let mut mask: u32 = 16;
        while mask > 0 {
            let upper = idx & mask != 0;
            if use_lng {
                let mid = (lng_lo + lng_hi) / 2.0;
                if upper {
                    lng_lo = mid;
                } else {
                    lng_hi = mid;
                }
            } else {
                let mid = (lat_lo + lat_hi) / 2.0;
                if upper {
                    lat_lo = mid;
                } else {
                    lat_hi = mid;
                }
            }
            use_lng = !use_lng;
            mask /= 2;
        }
        count += 1;
        pos += c.len_utf8();
    }
    Ok(GeohashCell {
        min_lat: lat_lo,
        max_lat: lat_hi,
        min_lng: lng_lo,
        max_lng: lng_hi,
    })
}
