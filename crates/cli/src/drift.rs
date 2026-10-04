//! Compare what `build` would write with what is on disk. Byte-exact: the
//! emitter is deterministic, and `build` replaces the whole directory, so any
//! extra file is drift too.

use std::collections::BTreeMap;
use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Differs,
    Missing,
    Extra,
}

impl fmt::Display for Kind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Kind::Differs => "differs",
            Kind::Missing => "missing",
            Kind::Extra => "extra",
        })
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct Drift {
    pub path: String,
    pub kind: Kind,
}

/// Both maps are keyed by `/`-separated paths relative to the package root.
pub fn compare(expected: &BTreeMap<String, String>, actual: &BTreeMap<String, Vec<u8>>) -> Vec<Drift> {
    let mut out: Vec<Drift> = expected
        .iter()
        .filter_map(|(path, source)| {
            let kind = match actual.get(path) {
                None => Kind::Missing,
                Some(bytes) if bytes.as_slice() != source.as_bytes() => Kind::Differs,
                Some(_) => return None,
            };
            Some(Drift { path: path.clone(), kind })
        })
        .collect();
    out.extend(
        actual
            .keys()
            .filter(|path| !expected.contains_key(*path))
            .map(|path| Drift { path: path.clone(), kind: Kind::Extra }),
    );
    out.sort_by(|a, b| a.path.cmp(&b.path));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map<V: Clone>(xs: &[(&str, V)]) -> BTreeMap<String, V> {
        xs.iter().map(|(k, v)| (k.to_string(), v.clone())).collect()
    }

    #[test]
    fn identical_trees_have_no_drift() {
        let expected = map(&[("a.ts", "x".to_string())]);
        let actual = map(&[("a.ts", b"x".to_vec())]);
        assert!(compare(&expected, &actual).is_empty());
    }

    #[test]
    fn every_kind_is_reported_in_path_order() {
        let expected = map(&[("b.ts", "new".to_string()), ("c.ts", "c".to_string())]);
        let actual = map(&[("a.ts", b"stale".to_vec()), ("b.ts", b"old".to_vec())]);
        let kinds: Vec<(String, Kind)> = compare(&expected, &actual).into_iter().map(|d| (d.path, d.kind)).collect();
        assert_eq!(
            kinds,
            [
                ("a.ts".to_string(), Kind::Extra),
                ("b.ts".to_string(), Kind::Differs),
                ("c.ts".to_string(), Kind::Missing),
            ]
        );
    }

    #[test]
    fn line_endings_are_not_normalized() {
        let expected = map(&[("a.ts", "x\n".to_string())]);
        let actual = map(&[("a.ts", b"x\r\n".to_vec())]);
        assert_eq!(compare(&expected, &actual)[0].kind, Kind::Differs);
    }
}
