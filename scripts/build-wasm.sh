#!/usr/bin/env bash
# Build the solver to WASM and generate the JS glue (both directories are git-ignored).
# Needs wasm-bindgen-cli matching the wasm-bindgen version in Cargo.lock:
#   cargo install wasm-bindgen-cli --version 0.2.129 --locked
#
#   scripts/build-wasm.sh            both variants (default)
#   scripts/build-wasm.sh threads    threaded variant only -> web/src/wasm
#   scripts/build-wasm.sh single     single-threaded variant only (stable) -> web/src/wasm-single
#
# The threaded build (rayon over Web Workers) needs a nightly toolchain with rust-src:
#   rustup toolchain install nightly --profile minimal -c rust-src -t wasm32-unknown-unknown
# and a cross-origin isolated page (COOP/COEP) so SharedArrayBuffer exists. The worker
# (web/src/api/solver.worker.ts) loads the single-threaded variant when it is not available.
set -euo pipefail
cd "$(dirname "$0")/.."
export PATH="$HOME/.cargo/bin:$PATH"
mode="${1:-all}"
case "$mode" in all | threads | single) ;; *) echo "usage: $0 [all|threads|single]" >&2; exit 2 ;; esac

if [ "$mode" != threads ]; then
  cargo build -p basetherm-wasm-api --target wasm32-unknown-unknown --release
  wasm-bindgen --target web --out-dir web/src/wasm-single --out-name basetherm \
    target/wasm32-unknown-unknown/release/basetherm_wasm_api.wasm
fi
if [ "$mode" != single ]; then
  RUSTFLAGS='-C target-feature=+atomics,+bulk-memory,+mutable-globals -C link-arg=--shared-memory -C link-arg=--import-memory -C link-arg=--max-memory=2147483648 -C link-arg=--export=__wasm_init_tls -C link-arg=--export=__tls_size -C link-arg=--export=__tls_align -C link-arg=--export=__tls_base' \
    cargo +nightly build -p basetherm-wasm-api --target wasm32-unknown-unknown --release \
    --features parallel -Z build-std=std,panic_abort --target-dir target/wasm-threads
  wasm-bindgen --target web --out-dir web/src/wasm --out-name basetherm \
    target/wasm-threads/wasm32-unknown-unknown/release/basetherm_wasm_api.wasm
  # wasm-bindgen-rayon's worker helper imports the glue as '../../..' (the output directory).
  printf "export * from './basetherm.js';\nexport { default } from './basetherm.js';\n" > web/src/wasm/index.js
fi
