#!/usr/bin/env bash
# Fetch every corpus entry at its pinned commit and survey it.
# Writes corpus/results.jsonl (one JSON object per entry, with "entry" and
# "category" added) and prints the human summaries.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
corpus="$root/corpus"
bin="$root/target/debug/purecrate-ts"
git="${GIT:-git}"

cargo build --offline -q -p purecrate-ts

mkdir -p "$corpus/src"
: > "$corpus/results.jsonl"
while IFS=$'\t' read -r name category repo commit path; do
  [[ -z "$name" || "$name" == \#* ]] && continue
  checkout="$corpus/src/$(basename "$repo")"
  if [[ ! -d "$checkout/.git" ]]; then
    "$git" init -q "$checkout"
    "$git" -C "$checkout" remote add origin "$repo"
  fi
  if [[ "$("$git" -C "$checkout" rev-parse -q --verify HEAD 2>/dev/null)" != "$commit" ]]; then
    "$git" -C "$checkout" fetch -q --depth 1 origin "$commit"
    "$git" -C "$checkout" checkout -q --detach FETCH_HEAD
  fi
  target="$checkout/$path"
  echo "== $name ($category)"
  "$bin" survey "$target" || true
  "$bin" survey "$target" --json \
    | sed "s|^{|{\"entry\":\"$name\",\"category\":\"$category\",|" \
    | sed "s|$corpus/src/||g" >> "$corpus/results.jsonl" || true
done < "$corpus/manifest.tsv"
