#!/usr/bin/env bash
# v0 acceptance: Rust tests (golden + TS/Rust equivalence via node), drift of
# each example's committed packages, then tsc over them (each carries its
# runtime; those with a schema find its library in examples/node_modules) and
# over the runtime and adapter sources that every package copies, under each
# supported TypeScript major (see TS_MAJORS in
# crates/cli/tests/it/support/mod.rs), then oxlint and `oxfmt --check` over
# the generated output.
# `npm ci` in examples/ installs the schema libraries, oxlint, and oxfmt.
set -euo pipefail
cd "$(dirname "$0")/.."

# `--tag vX.Y.Z` in the install docs must match the workspace version, so a
# release cannot leave README or the authoring skill on an old pin.
version=$(sed -n 's/^version = "\(.*\)"$/\1/p' Cargo.toml | head -1)
for f in README.md skills/purecrate-authoring/SKILL.md; do
  if ! grep -qE -- "--tag v$version" "$f"; then
    echo "verify: $f has no --tag v$version (workspace version)" >&2
    exit 1
  fi
  others=$(grep -oE -- '--tag v[0-9]+\.[0-9]+\.[0-9]+' "$f" | grep -v -- "v$version" || true)
  if [ -n "$others" ]; then
    echo "verify: $f pins a tag other than v$version:" >&2
    echo "$others" >&2
    exit 1
  fi
done
# The runtime packages are private copies, but their version should still
# follow the workspace so it is not mistaken for a separate 0.1.0 line.
for p in packages/boundary packages/boundary-zod packages/boundary-valibot packages/boundary-arktype; do
  pv=$(sed -n 's/.*"version": "\([^"]*\)".*/\1/p' "$p/package.json" | head -1)
  if [ "$pv" != "$version" ]; then
    echo "verify: $p version is $pv, workspace is $version" >&2
    exit 1
  fi
done
for p in packages/boundary-zod packages/boundary-valibot packages/boundary-arktype; do
  if ! grep -q "\"purecrate\": \"^$version\"" "$p/package.json"; then
    echo "verify: $p peer-depends on purecrate other than ^$version" >&2
    exit 1
  fi
done

TS_MAJORS=(6 7)

cargo test --offline -q
# Every example's committed output (scripts/examples.sh regenerates them).
examples=()
for dir in examples/*/; do
  dir=${dir%/}
  [ -f "$dir/src/lib.rs" ] || continue
  cargo run --offline -q -p purecrate-ts -- check "$dir" --out "$dir/ts/plain"
  examples+=("$dir/ts/plain")
  # Nested module files and multi-line derives are visible to check, not to
  # a `src/*.rs` grep. `--schema` refuses when there is no wire form.
  wired=
  for lib in zod valibot arktype; do
    if err=$(cargo run --offline -q -p purecrate-ts -- check "$dir" --out "$dir/ts/$lib" --schema "$lib" 2>&1); then
      wired=1
      examples+=("$dir/ts/$lib")
      continue
    fi
    if [ -z "$wired" ] && printf '%s\n' "$err" | grep -q 'no public struct or enum derives'; then
      break
    fi
    printf '%s\n' "$err" >&2
    exit 1
  done
done
# The adapters import the runtime package, which exports `dist`; here its
# sources stand in for it (the purecrate-source condition).
for major in "${TS_MAJORS[@]}"; do
  for dir in packages/boundary packages/boundary-zod packages/boundary-valibot packages/boundary-arktype "${examples[@]}"; do
    (cd "$dir" && npx -y -p "typescript@$major" tsc -p . --customConditions purecrate-source) \
      || { echo "verify: tsc $major failed in $dir" >&2; exit 1; }
  done
done
# Every example's output passes oxlint with examples/.oxlintrc.json, type-aware,
# from examples/node_modules.
(cd examples && npx --no-install oxlint --type-aware --deny-warnings .) \
  || { echo "verify: oxlint rejects the generated output" >&2; exit 1; }
# ..and is laid out as oxfmt lays it out (examples/.oxfmtrc.json).
(cd examples && npx --no-install oxfmt --check '*/ts/*/src/**/*.ts') \
  || { echo "verify: oxfmt would reformat the generated output" >&2; exit 1; }
echo "verify: ok"
