#!/bin/bash
# Builds hams-polish (the local cleanup-model helper) and puts it where Tauri expects a
# bundled helper: src-tauri/binaries/hams-polish-<target triple>. Also drops a copy
# beside the debug build so `cargo run` and `tauri dev` find it.
set -euo pipefail
cd "$(dirname "$0")/../src-tauri"

cargo build --release -p hams-polish
triple="$(rustc -vV | sed -n 's/^host: //p')"
mkdir -p binaries target/debug
cp target/release/hams-polish "binaries/hams-polish-$triple"
cp target/release/hams-polish target/debug/hams-polish
echo "hams-polish ready for $triple"
