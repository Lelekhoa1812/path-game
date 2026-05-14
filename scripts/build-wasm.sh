#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."
wasm-pack build crates/map_loader_wasm --target web --out-dir ../../public/map_loader_wasm

