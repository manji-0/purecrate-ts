//! `survey`: how much of an existing crate is inside the subset. Each public
//! function and type is judged with everything it transitively refers to, so
//! the verdict is what `build` would say about that item alone.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

use purecrate_check::accept;
use purecrate_ir::{Callee, Crate, Expr, Item, Pattern, Reason, Ty, VariantFields, ORDERING};
use purecrate_syntax::{module_decls, survey_files, LineCol, ParseError, Unit, UnitKind};

pub struct Report {
    pub krate: String,
    pub files: Vec<PathBuf>,
    /// `mod x;` declarations whose file was not found.
    pub missing: Vec<String>,
    pub verdicts: Vec<Verdict>,
    /// Units that are neither public functions nor public types, e.g. trait
    /// impls, by what they are.
    pub others: BTreeMap<&'static str, usize>,
}

pub struct Verdict {
    pub name: String,
    pub owner: Option<String>,
    pub kind: &'static str,
    pub file: usize,
    pub at: LineCol,
    pub outcome: Outcome,
}

pub enum Outcome {
    Accepted,
    /// The item itself is outside the subset.
    Rejected(Vec<Cause>),
    /// The item is fine, but something it refers to is not.
    Blocked {
        by: String,
        causes: Vec<Cause>,
    },
}

#[derive(Clone)]
pub struct Cause {
    pub reason: Reason,
    pub detail: Option<String>,
    pub message: String,
}

impl Cause {
    fn parse(e: &ParseError) -> Self {
        Self { reason: e.reason, detail: e.detail.clone(), message: e.message.clone() }
    }
}

/// The root file and every module file reachable from it.
pub fn collect_files(root: &Path) -> (Vec<(PathBuf, String)>, Vec<String>) {
    let mut files = Vec::new();
    let mut missing = Vec::new();
    let mut queue = vec![(root.to_path_buf(), module_dir(root, true))];
    while let Some((path, dir)) = queue.pop() {
        let Ok(text) = fs::read_to_string(&path) else {
            missing.push(path.display().to_string());
            continue;
        };
        for decl in module_decls(&text).unwrap_or_default() {
            let base = decl.iter().fold(dir.clone(), |d, seg| d.join(seg));
            let flat = base.with_extension("rs");
            let nested = base.join("mod.rs");
            match (flat.exists(), nested.exists()) {
                (true, _) => queue.push((flat.clone(), base)),
                (false, true) => queue.push((nested, base)),
                (false, false) => missing.push(format!("{} (from {})", decl.join("::"), path.display())),
            }
        }
        files.push((path, text));
    }
    files.sort_by(|a, b| a.0.cmp(&b.0));
    (files, missing)
}

/// Where `mod x;` in `file` looks for `x.rs`.
fn module_dir(file: &Path, is_root: bool) -> PathBuf {
    let parent = file.parent().unwrap_or(Path::new(".")).to_path_buf();
    let stem = file.file_stem().and_then(|s| s.to_str()).unwrap_or("");
    if is_root || stem == "mod" {
        parent
    } else {
        parent.join(stem)
    }
}

pub fn survey(
    krate: &str,
    files: Vec<(PathBuf, String)>,
    missing: Vec<String>,
    all_causes: bool,
) -> Result<Report, String> {
    let sources: Vec<&str> = files.iter().map(|(_, t)| t.as_str()).collect();
    let units = survey_files(&sources, all_causes).map_err(|(i, e)| format!("{}:{e}", files[i].0.display()))?;
    let index = Index::new(&units);
    let mut verdicts = Vec::new();
    let mut others = BTreeMap::new();
    for (i, unit) in units.iter().enumerate() {
        let kind = match &unit.kind {
            UnitKind::Fn => "fn",
            UnitKind::Method { .. } => "method",
            UnitKind::Struct => "struct",
            UnitKind::Enum => "enum",
            UnitKind::Alias => "alias",
            UnitKind::Other { what } => {
                *others.entry(*what).or_insert(0) += 1;
                continue;
            }
        };
        if !unit.public {
            continue;
        }
        verdicts.push(Verdict {
            name: unit.name.clone(),
            owner: match &unit.kind {
                UnitKind::Method { owner } => Some(owner.clone()),
                _ => None,
            },
            kind,
            file: unit.file,
            at: unit.at,
            outcome: judge(krate, &units, &index, i),
        });
    }
    Ok(Report {
        krate: krate.to_string(),
        files: files.into_iter().map(|(p, _)| p).collect(),
        missing,
        verdicts,
        others,
    })
}

struct Index {
    types: HashMap<String, Vec<usize>>,
    fns: HashMap<String, Vec<usize>>,
    methods: HashMap<(String, String), Vec<usize>>,
}

impl Index {
    fn new(units: &[Unit]) -> Self {
        let mut index = Index { types: HashMap::new(), fns: HashMap::new(), methods: HashMap::new() };
        for (i, u) in units.iter().enumerate() {
            match &u.kind {
                UnitKind::Struct | UnitKind::Enum | UnitKind::Alias => {
                    index.types.entry(u.name.clone()).or_default().push(i)
                }
                UnitKind::Fn => index.fns.entry(u.name.clone()).or_default().push(i),
                UnitKind::Method { owner } => index.methods.entry((owner.clone(), u.name.clone())).or_default().push(i),
                UnitKind::Other { .. } => {}
            }
        }
        index
    }

    fn lookup(&self, r: &Ref) -> &[usize] {
        let found = match r {
            Ref::Type(t) => self.types.get(t),
            Ref::Fn(f) => self.fns.get(f),
            Ref::Method(t, m) => self.methods.get(&(t.clone(), m.clone())),
            Ref::ReceiverCall(_) => None,
        };
        found.map(Vec::as_slice).unwrap_or(&[])
    }
}

fn judge(krate: &str, units: &[Unit], index: &Index, start: usize) -> Outcome {
    let item = match &units[start].lowered {
        Ok(item) => item,
        Err(e) => return Outcome::Rejected(vec![Cause::parse(e)]),
    };
    // With `--all-causes`: what the item's own lowering stood in for.
    let recovered: Vec<Cause> = units[start].causes.iter().map(Cause::parse).collect();
    let mut seen = BTreeSet::from([start]);
    let mut stack = vec![(start, item)];
    let mut blockers = Vec::new();
    // `x.m()` names no type: it can only be `m` on a type the closure
    // already holds, so each such pair is added as either side appears.
    let mut types: BTreeSet<String> = BTreeSet::new();
    let mut receiver_calls: BTreeSet<String> = BTreeSet::new();
    while let Some((i, item)) = stack.pop() {
        let mut refs = Vec::new();
        if let UnitKind::Struct | UnitKind::Enum = &units[i].kind {
            if types.insert(units[i].name.clone()) {
                refs.extend(receiver_calls.iter().map(|m| Ref::Method(units[i].name.clone(), m.clone())));
            }
        }
        if let UnitKind::Method { owner } = &units[i].kind {
            refs.push(Ref::Type(owner.clone()));
        }
        for r in references(item) {
            match r {
                Ref::ReceiverCall(m) => {
                    if receiver_calls.insert(m.clone()) {
                        refs.extend(types.iter().map(|t| Ref::Method(t.clone(), m.clone())));
                    }
                }
                other => refs.push(other),
            }
        }
        for r in refs {
            for &j in index.lookup(&r) {
                if !seen.insert(j) {
                    continue;
                }
                match &units[j].lowered {
                    Ok(dep) if units[j].causes.is_empty() => stack.push((j, dep)),
                    Ok(_) => blockers.push((j, units[j].causes.iter().map(Cause::parse).collect())),
                    Err(e) => blockers.push((j, vec![Cause::parse(e)])),
                }
            }
        }
    }
    if let Some((j, causes)) = blockers.into_iter().min_by_key(|(j, _)| *j) {
        if !recovered.is_empty() {
            return Outcome::Rejected(recovered);
        }
        return Outcome::Blocked { by: display_name(&units[j]), causes };
    }
    let order: Vec<usize> = seen.into_iter().collect();
    let items = order.iter().map(|&j| units[j].lowered.clone().expect("blockers were handled")).collect();
    match accept(&Crate::new(krate, items)) {
        Ok(_) if recovered.is_empty() => Outcome::Accepted,
        Ok(_) => Outcome::Rejected(recovered),
        Err(diagnostics) => {
            let at = |d: &purecrate_check::Diagnostic| order[d.item];
            let cause = |d: &purecrate_check::Diagnostic| Cause {
                reason: d.reason,
                detail: d.detail.clone(),
                message: d.message.clone(),
            };
            let own: Vec<Cause> =
                recovered.into_iter().chain(diagnostics.iter().filter(|d| at(d) == start).map(cause)).collect();
            if !own.is_empty() {
                return Outcome::Rejected(own);
            }
            let first = &diagnostics[0];
            Outcome::Blocked {
                by: display_name(&units[at(first)]),
                causes: diagnostics.iter().filter(|d| at(d) == at(first)).map(cause).collect(),
            }
        }
    }
}

fn display_name(u: &Unit) -> String {
    match &u.kind {
        UnitKind::Method { owner } => format!("{owner}::{}", u.name),
        _ => u.name.clone(),
    }
}

enum Ref {
    Type(String),
    Fn(String),
    Method(String, String),
    /// `x.m()`: `m` on whichever type `x` has.
    ReceiverCall(String),
}

fn references(item: &Item) -> Vec<Ref> {
    let mut out = Vec::new();
    match item {
        Item::Struct(s) => {
            s.fields.iter().for_each(|f| ty_refs(&f.ty, &mut out));
            // `#[serde(try_from = "T")]` reads `T` through `impl TryFrom<T>`.
            if let Some(from) = &s.wire_from {
                ty_refs(from, &mut out);
                out.push(Ref::Method(s.name.as_str().to_string(), "try_from".to_string()));
            }
        }
        Item::Enum(e) => {
            for v in &e.variants {
                match &v.fields {
                    VariantFields::Unit => {}
                    VariantFields::Tuple(ts) => ts.iter().for_each(|t| ty_refs(t, &mut out)),
                    VariantFields::Struct(fs) => fs.iter().for_each(|f| ty_refs(&f.ty, &mut out)),
                }
            }
        }
        Item::Alias(a) => ty_refs(&a.ty, &mut out),
        Item::Fn(f) => {
            f.params.iter().for_each(|p| ty_refs(&p.ty, &mut out));
            ty_refs(&f.ret, &mut out);
            expr_refs(&f.body, &mut out);
        }
        Item::Const(c) => {
            ty_refs(&c.ty, &mut out);
            expr_refs(&c.value, &mut out);
        }
    }
    out
}

fn ty_refs(ty: &Ty, out: &mut Vec<Ref>) {
    match ty {
        Ty::Named(n) => out.push(Ref::Type(n.as_str().to_string())),
        ty => ty.children().into_iter().for_each(|t| ty_refs(t, out)),
    }
}

fn expr_refs(expr: &Expr, out: &mut Vec<Ref>) {
    match expr {
        Expr::Call { callee, .. } => match callee {
            Callee::Fn(n) => out.push(Ref::Fn(n.as_str().to_string())),
            Callee::Method { ty, name } => {
                out.push(Ref::Type(ty.as_str().to_string()));
                out.push(Ref::Method(ty.as_str().to_string(), name.as_str().to_string()));
            }
            Callee::Variant { ty, .. } | Callee::StructNew(ty) => out.push(Ref::Type(ty.as_str().to_string())),
            _ => {}
        },
        Expr::MethodCall { name, args, .. } => {
            out.push(Ref::ReceiverCall(name.as_str().to_string()));
            // `cmp` gives std's `Ordering`, which the parser adds as an item.
            if name.as_str() == "cmp" {
                out.push(Ref::Type(ORDERING.to_string()));
            }
            // `opt.map(f)` and the like name a function (`check::resolve`).
            if matches!(name.as_str(), "map" | "all" | "any" | "position" | "then_with") {
                for a in args {
                    if let Expr::Var(f) = a.unpositioned() {
                        out.push(Ref::Fn(f.as_str().to_string()));
                    }
                }
            }
        }
        Expr::Construct { ty, .. } => out.push(Ref::Type(ty.as_str().to_string())),
        Expr::Match { arms, .. } => {
            for arm in arms {
                pattern_refs(&arm.pattern, out);
            }
        }
        _ => {}
    }
    expr.own_types().into_iter().for_each(|t| ty_refs(t, out));
    for child in expr.children() {
        expr_refs(child, out);
    }
}

fn pattern_refs(p: &Pattern, out: &mut Vec<Ref>) {
    match p {
        Pattern::Variant { ty, .. } => out.push(Ref::Type(ty.as_str().to_string())),
        Pattern::OptionSome(inner) | Pattern::ResultOk(inner) | Pattern::ResultErr(inner) => pattern_refs(inner, out),
        Pattern::Or(alts) | Pattern::Tuple(alts) => alts.iter().for_each(|alt| pattern_refs(alt, out)),
        _ => {}
    }
}

impl Report {
    fn tally(&self, types: bool) -> Tally {
        let mut t = Tally::default();
        for v in self.verdicts.iter().filter(|v| is_type(v.kind) == types) {
            t.total += 1;
            match &v.outcome {
                Outcome::Accepted => t.accepted += 1,
                Outcome::Rejected(causes) => {
                    t.rejected += 1;
                    count(&mut t.own, causes);
                }
                Outcome::Blocked { causes, .. } => {
                    t.blocked += 1;
                    count(&mut t.blocking, causes);
                }
            }
        }
        t
    }

    pub fn human(&self) -> String {
        let mut out = format!("crate {} ({} file(s))\n", self.krate, self.files.len());
        for (label, types) in [("public functions", false), ("public types", true)] {
            let t = self.tally(types);
            out.push_str(&format!(
                "{label}: {} — accepted {} ({}), rejected {}, blocked by a dependency {}\n",
                t.total,
                t.accepted,
                percent(t.accepted, t.total),
                t.rejected,
                t.blocked
            ));
            for (title, map) in [("  own reasons", &t.own), ("  blocking reasons", &t.blocking)] {
                if map.is_empty() {
                    continue;
                }
                out.push_str(&format!("{title}:\n"));
                for (code, (n, details)) in sorted(map).into_iter().take(8) {
                    let top: Vec<String> =
                        sorted_details(details).into_iter().take(4).map(|(d, k)| format!("{d}×{k}")).collect();
                    let suffix = if top.is_empty() { String::new() } else { format!("  ({})", top.join(", ")) };
                    out.push_str(&format!("    {n:>4}  {code}{suffix}\n"));
                }
            }
        }
        if !self.others.is_empty() {
            let parts: Vec<String> = self.others.iter().map(|(k, n)| format!("{k} {n}")).collect();
            out.push_str(&format!("not judged: {}\n", parts.join(", ")));
        }
        for m in &self.missing {
            out.push_str(&format!("warning: module file not found: {m}\n"));
        }
        out
    }

    pub fn json(&self) -> String {
        let mut o = Json::default();
        o.open('{');
        o.field("crate", &json_str(&self.krate));
        o.field("files", &json_array(self.files.iter().map(|p| json_str(&p.display().to_string()))));
        o.field("missing_modules", &json_array(self.missing.iter().map(|m| json_str(m))));
        for (key, types) in [("functions", false), ("types", true)] {
            let t = self.tally(types);
            o.field(
                key,
                &format!(
                    "{{\"total\":{},\"accepted\":{},\"rejected\":{},\"blocked\":{},\"own\":{},\"blocking\":{}}}",
                    t.total,
                    t.accepted,
                    t.rejected,
                    t.blocked,
                    reason_counts(&t.own),
                    reason_counts(&t.blocking)
                ),
            );
        }
        o.field(
            "not_judged",
            &format!(
                "{{{}}}",
                self.others.iter().map(|(k, n)| format!("{}:{n}", json_str(k))).collect::<Vec<_>>().join(",")
            ),
        );
        o.field("items", &json_array(self.verdicts.iter().map(|v| self.verdict_json(v))));
        o.close('}');
        o.0
    }

    fn verdict_json(&self, v: &Verdict) -> String {
        let (status, by, causes) = match &v.outcome {
            Outcome::Accepted => ("accepted", None, &[][..]),
            Outcome::Rejected(c) => ("rejected", None, c.as_slice()),
            Outcome::Blocked { by, causes } => ("blocked", Some(by), causes.as_slice()),
        };
        let mut fields = vec![
            format!("\"name\":{}", json_str(&v.name)),
            format!("\"kind\":{}", json_str(v.kind)),
            format!("\"at\":{}", json_str(&format!("{}:{}:{}", self.files[v.file].display(), v.at.line, v.at.col))),
            format!("\"status\":{}", json_str(status)),
        ];
        if let Some(owner) = &v.owner {
            fields.push(format!("\"owner\":{}", json_str(owner)));
        }
        if let Some(by) = by {
            fields.push(format!("\"blocked_by\":{}", json_str(by)));
        }
        if !causes.is_empty() {
            fields.push(format!(
                "\"causes\":{}",
                json_array(causes.iter().map(|c| {
                    let detail = c.detail.as_deref().map(json_str).unwrap_or_else(|| "null".into());
                    format!(
                        "{{\"code\":{},\"detail\":{detail},\"message\":{}}}",
                        json_str(c.reason.code()),
                        json_str(&c.message)
                    )
                }))
            ));
        }
        format!("{{{}}}", fields.join(","))
    }
}

fn is_type(kind: &str) -> bool {
    matches!(kind, "struct" | "enum" | "alias")
}

/// Per reason code: items counted once per code, and how often each detail
/// occurred.
type ByReason = BTreeMap<&'static str, (usize, BTreeMap<String, usize>)>;

#[derive(Default)]
struct Tally {
    total: usize,
    accepted: usize,
    rejected: usize,
    blocked: usize,
    own: ByReason,
    blocking: ByReason,
}

fn count(map: &mut ByReason, causes: &[Cause]) {
    let mut codes = BTreeSet::new();
    for c in causes {
        let entry = map.entry(c.reason.code()).or_default();
        if codes.insert(c.reason.code()) {
            entry.0 += 1;
        }
        if let Some(d) = &c.detail {
            *entry.1.entry(d.clone()).or_default() += 1;
        }
    }
}

/// A reason code with its count and the count per detail.
type ReasonRow<'a> = (&'static str, &'a (usize, BTreeMap<String, usize>));

fn sorted(map: &ByReason) -> Vec<ReasonRow<'_>> {
    let mut v: Vec<_> = map.iter().map(|(k, v)| (*k, v)).collect();
    v.sort_by(|a, b| b.1 .0.cmp(&a.1 .0).then(a.0.cmp(b.0)));
    v
}

fn sorted_details(details: &BTreeMap<String, usize>) -> Vec<(&str, usize)> {
    let mut v: Vec<_> = details.iter().map(|(k, n)| (k.as_str(), *n)).collect();
    v.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
    v
}

fn reason_counts(map: &ByReason) -> String {
    let parts: Vec<String> = sorted(map)
        .into_iter()
        .map(|(code, (n, details))| {
            let ds: Vec<String> =
                sorted_details(details).into_iter().map(|(d, k)| format!("{}:{k}", json_str(d))).collect();
            format!("{}:{{\"items\":{n},\"details\":{{{}}}}}", json_str(code), ds.join(","))
        })
        .collect();
    format!("{{{}}}", parts.join(","))
}

fn percent(n: usize, total: usize) -> String {
    if total == 0 {
        "-".into()
    } else {
        format!("{:.0}%", n as f64 * 100.0 / total as f64)
    }
}

#[derive(Default)]
struct Json(String, bool);

impl Json {
    fn open(&mut self, c: char) {
        self.0.push(c);
        self.1 = false;
    }
    fn close(&mut self, c: char) {
        self.0.push(c);
    }
    fn field(&mut self, key: &str, value: &str) {
        if self.1 {
            self.0.push(',');
        }
        self.1 = true;
        self.0.push_str(&json_str(key));
        self.0.push(':');
        self.0.push_str(value);
    }
}

fn json_array(items: impl Iterator<Item = String>) -> String {
    format!("[{}]", items.collect::<Vec<_>>().join(","))
}

fn json_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}
