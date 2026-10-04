#!/usr/bin/env python3
"""Reduce what a generated seed fails on to a small crate.

    scripts/gen-reduce.py GENERATOR SEED [--fn NAME] [--keep DIR] [--cli BIN]

GENERATOR is one of expressions, statements, patterns, widths, text (as in
scripts/gen-sweep.sh). The seed's crate is generated and built with
target/debug/purecrate-ts; the first function tsc refuses (or that makes
`build` crash) is cut out of it, with the crate's own types where the
generator has them, and lines and `{ .. }` blocks are dropped while the same
refusal (or crash) remains. The result is printed, and left in DIR (default:
a temporary directory) as DIR/min.rs. `--cli` builds with another binary
(a release's, to see what it made of the same crate).

A value mismatch (the sweep says FAIL with `rust .. / ts ..`) is not
reduced: pass `--fn` with the function the log names (`t26F30` is
`t26_f30`), and the function is cut out as it stands, for a fixture.
"""

import argparse
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
TESTS = {
    "expressions": ("generated_equivalence::generated_expressions_match_rust", "s"),
    "statements": ("generated_statements_equivalence::generated_statements_match_rust", "t"),
    "patterns": ("generated_patterns_equivalence::generated_patterns_match_rust", "p"),
    "widths": ("generated_widths_equivalence::generated_widths_match_rust", "w"),
    "text": ("generated_text_equivalence::generated_text_matches_rust", "x"),
}


def run(cmd, **kw):
    return subprocess.run(cmd, capture_output=True, text=True, **kw)


def test_binary():
    out = run(["cargo", "test", "--offline", "-q", "-p", "purecrate-ts", "--test", "it", "--no-run",
               "--message-format=json"], cwd=ROOT)
    for line in out.stdout.splitlines():
        try:
            m = json.loads(line)
        except ValueError:
            continue
        exe = m.get("executable")
        if m.get("reason") == "compiler-artifact" and exe and "/deps/it-" in exe:
            return exe
    sys.exit("gen-reduce: no test binary built")


def tsc():
    neutral = os.path.join(tempfile.gettempdir(), "purecrate-npx")
    os.makedirs(neutral, exist_ok=True)
    out = run(["npx", "-y", "-p", "typescript@6", "-c", "command -v tsc"], cwd=neutral)
    path = out.stdout.strip()
    if not path:
        sys.exit("gen-reduce: cannot resolve tsc through npx")
    return path


def outcome(cli, tsc_path, source, work):
    """What `build` and tsc make of `source`: ("crash",), ("rejected",),
    ("ok",), or ("tsc", code, file) for the first refusal."""
    d = tempfile.mkdtemp(dir=work)
    os.makedirs(os.path.join(d, "c", "src"))
    with open(os.path.join(d, "c", "src", "lib.rs"), "w") as f:
        f.write(source)
    b = run([cli, "build", os.path.join(d, "c"), "--out", os.path.join(d, "out")])
    if b.returncode not in (0, 1):
        return ("crash",)
    if b.returncode != 0:
        return ("rejected",)
    t = run([tsc_path, "-p", os.path.join(d, "out")])
    m = re.search(r"src/([a-z0-9-]+)\.ts\(\d+,\d+\): error (TS\d+)", t.stdout)
    return ("tsc", m.group(2), m.group(1)) if m else ("ok",)


def cut(source, name, with_types):
    m = re.search(r"pub fn %s\(.*?\n}\n" % re.escape(name), source, re.S)
    if not m:
        sys.exit(f"gen-reduce: no function {name} in the generated crate")
    head = source[: source.index("pub fn ")] if with_types else ""
    return "#![allow(unused)]\n" + head + m.group(0)


def reduce(lines, keeps):
    """Drop `{ .. }` blocks, largest first, then single lines, while
    `keeps` holds."""

    def blocks(ls):
        out = []
        for i, l in enumerate(ls):
            if l.rstrip().endswith("{"):
                depth = 0
                for j in range(i, len(ls)):
                    depth += ls[j].count("{") - ls[j].count("}")
                    if depth <= 0 and j > i:
                        out.append((i, j + 1))
                        break
        return sorted(out, key=lambda r: r[0] - r[1])

    changed = True
    while changed:
        changed = False
        for a, b in blocks(lines):
            trial = lines[:a] + lines[b:]
            if a > 0 and keeps(trial):
                lines, changed = trial, True
                break
        if changed:
            continue
        i = 0
        while i < len(lines):
            trial = lines[:i] + lines[i + 1:]
            if keeps(trial):
                lines, changed = trial, True
            else:
                i += 1
    return lines


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("generator", choices=sorted(TESTS))
    ap.add_argument("seed", type=int)
    ap.add_argument("--fn", dest="fn", help="the function to cut out (snake case), skipping the search")
    ap.add_argument("--keep", help="directory for the crate and the result")
    ap.add_argument("--cli", help="the purecrate-ts binary to build with (default: target/debug, built first)")
    args = ap.parse_args()

    work = args.keep or tempfile.mkdtemp(prefix="gen-reduce.")
    os.makedirs(work, exist_ok=True)
    test, prefix = TESTS[args.generator]
    print(f"gen-reduce: building (work in {work})", file=sys.stderr)
    binary = test_binary()
    cli = args.cli
    if not cli:
        if run(["cargo", "build", "--offline", "-q", "-p", "purecrate-ts"], cwd=ROOT).returncode != 0:
            sys.exit("gen-reduce: cargo build failed")
        cli = os.path.join(ROOT, "target", "debug", "purecrate-ts")

    dump = os.path.join(work, "seed.rs")
    env = dict(os.environ, PURECRATE_SKIP_NODE="1", PURECRATE_GEN_SEED=str(args.seed), PURECRATE_GEN_DUMP=dump)
    run([binary, "--exact", test], cwd=os.path.join(ROOT, "crates", "cli"), env=env)
    source = open(dump).read()
    with_types = args.generator == "patterns"

    if args.fn:
        one = cut(source, args.fn, with_types)
        with open(os.path.join(work, "min.rs"), "w") as f:
            f.write(one)
        print(one)
        return

    tsc_path = tsc()
    whole = outcome(cli, tsc_path, source, work)
    if whole[0] == "crash":
        # Find the function `build` crashes on, one at a time.
        names = re.findall(r"pub fn (%s\d+_f\d+)\(" % prefix, source)
        target = next((n for n in names if outcome(cli, tsc_path, cut(source, n, with_types), work)[0] == "crash"), None)
        if target is None:
            sys.exit("gen-reduce: the crate crashes `build`, no single function does")
        want = ("crash",)
    elif whole[0] == "tsc":
        stem = whole[2]
        target = re.sub(r"^([a-z])(\d+)f(\d+)$", r"\1\2_f\3", stem)
        want = ("tsc", whole[1])
    else:
        sys.exit(f"gen-reduce: seed {args.seed} builds and type-checks ({whole[0]}); "
                 "for a value mismatch, pass --fn with the function the sweep's log names")

    one = cut(source, target, with_types)
    keeps = lambda ls: outcome(cli, tsc_path, "\n".join(ls), work)[: len(want)] == want
    if not keeps(one.split("\n")):
        sys.exit(f"gen-reduce: {target} alone does not reproduce {want}")
    print(f"gen-reduce: reducing {target} ({' '.join(want)})", file=sys.stderr)
    small = "\n".join(reduce(one.split("\n"), keeps))
    with open(os.path.join(work, "min.rs"), "w") as f:
        f.write(small)
    print(small)
    print(f"\ngen-reduce: {os.path.join(work, 'min.rs')}", file=sys.stderr)
    if not args.keep:
        for e in os.listdir(work):
            p = os.path.join(work, e)
            if os.path.isdir(p):
                shutil.rmtree(p, ignore_errors=True)


if __name__ == "__main__":
    main()
