#!/usr/bin/env bash
# Runs every QML-side test suite.
#
#   tests/run.sh
#
# Discovery is the point. CI runs this same script, so a suite added under
# tests/ starts running in both places without either being edited - which is
# how tests/rich-text.test.js ended up written but never run by CI.
#
# The Rust half of the plugin is tested by `cargo test --manifest-path
# backend/Cargo.toml`, which ./build.sh also runs.
set -uo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."
shopt -s nullglob
suites=(tests/*.test.js)

if [ ${#suites[@]} -eq 0 ]; then
  echo "no suites found under tests/ - expected tests/*.test.js" >&2
  exit 1
fi

# Every suite runs even after one fails: a green run should mean all of them
# passed, and a red one should say everything that is broken, not just the
# first thing.
status=0
for suite in "${suites[@]}"; do
  echo "==> $suite"
  node "$suite" || status=1
done

exit "$status"
