#!/usr/bin/env bash
# Regenerates vendor/ from Cargo.lock (needs the network). `.cargo/config.toml`
# replaces crates.io with vendor/, so every build is offline.
#
# `cargo vendor` copies every package in the lock file for every platform:
# uuid's wasm32 dependencies and serde_core's never-built serde_derive pin
# pull in about twenty crates nothing here compiles. Those are cut down to
# their manifest, which is all cargo reads of a package it does not build.
# The crates that are built lose their tests, benches, and examples. Each
# `.cargo-checksum.json` keeps the package checksum (checked against
# Cargo.lock) and lists only the files that remain.
#
#   scripts/vendor.sh     # after changing a dependency or `cargo update`
set -euo pipefail
cd "$(dirname "$0")/.."

# The release targets (.github/workflows/release.yml).
targets="x86_64-unknown-linux-gnu aarch64-unknown-linux-gnu aarch64-apple-darwin x86_64-apple-darwin"

rm -rf vendor.new
cargo vendor --locked --versioned-dirs --quiet vendor.new >/dev/null

built=$(for t in $targets; do
  cargo tree --locked --config 'source.vendored-sources.directory="vendor.new"' --workspace -e normal,build,dev --target "$t" --prefix none --no-dedupe --format '{p}'
done | grep -v ' (/' | sed 's/^\([^ ]*\) v\([^ ]*\).*/\1-\2/' | sort -u)

for dir in vendor.new/*/; do
  pkg=$(basename "$dir")
  if grep -qx "$pkg" <<<"$built"; then
    rm -rf "$dir"/{tests,benches,examples}
  else
    find "$dir" -mindepth 1 -maxdepth 1 ! -name Cargo.toml ! -name .cargo-checksum.json -exec rm -rf {} +
  fi
  # Drop the files removed above from the checksum list.
  node -e '
    const fs = require("fs"), path = require("path");
    const dir = process.argv[1], file = path.join(dir, ".cargo-checksum.json");
    const sum = JSON.parse(fs.readFileSync(file, "utf8"));
    for (const f of Object.keys(sum.files)) if (!fs.existsSync(path.join(dir, f))) delete sum.files[f];
    fs.writeFileSync(file, JSON.stringify(sum));
  ' "$dir"
done

rm -rf vendor
mv vendor.new vendor
du -sh vendor
