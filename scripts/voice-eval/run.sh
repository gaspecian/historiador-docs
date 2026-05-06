#!/usr/bin/env bash
#
# Voice regression eval runner (Sprint 11, phase B8).
#
# Executes every prompt file under prompts/eval/ against the stub
# LLM provider and asserts the behavioural checks declared in each
# file's front-matter. CI runs this nightly; regressions page on-
# call via the existing alert channel.
#
# Usage:
#   scripts/voice-eval/run.sh
#
# Exit codes:
#   0 — every scenario passed
#   1 — one or more scenarios failed (stdout has the details)
#
# This is a skeleton — it discovers the files, validates they
# parse as YAML-front-matter + body, and reports what the real
# run WOULD execute. The actual eval harness (turn replay,
# tool-call extraction, tone heuristics) builds on the
# `historiador_api::application::editor::*` primitives plus the
# stub LLM provider from `crates/llm`.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
EVAL_DIR="$ROOT/prompts/eval"

if [[ ! -d "$EVAL_DIR" ]]; then
  echo "eval directory not found at $EVAL_DIR" >&2
  exit 1
fi

shopt -s nullglob
files=( "$EVAL_DIR"/*.md )
if [[ ${#files[@]} -eq 0 ]]; then
  echo "no eval scenarios under $EVAL_DIR" >&2
  exit 1
fi

failed=0
for f in "${files[@]}"; do
  name="$(basename "$f")"
  # README is docs, not a scenario.
  if [[ "$name" == "README.md" ]]; then
    continue
  fi

  # Minimal structural validation: every scenario needs a
  # front-matter block and an `id` key. A missing front-matter is
  # a hard failure — the harness never fed the model anything
  # useful.
  if ! head -n 1 "$f" | grep -q '^---$'; then
    echo "FAIL $name — missing YAML front-matter" >&2
    failed=$((failed + 1))
    continue
  fi
  if ! grep -q '^id:' "$f"; then
    echo "FAIL $name — missing 'id' front-matter key" >&2
    failed=$((failed + 1))
    continue
  fi

  echo "OK   $name"
done

if [[ $failed -gt 0 ]]; then
  echo "$failed eval scenario(s) failed validation" >&2
  exit 1
fi

echo "all ${#files[@]} eval scenarios validated"
