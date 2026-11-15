# basetherm-wasm-api

wasm-bindgen entry points, typed interface for the Web Worker. Only crate that knows about WASM.

Keep files under about 300 lines. No DOM or UI dependency (see root `CLAUDE.md`). SI units only.

Entry points: `runProject`, `epwSummary`, `meshInfo` (mesh preview, `mesh.rs`), `schemaVersion` (`lib.rs`), `soilPresets` and `soilProperties` (`soil.rs`). JSON project schema in `project.rs` (versioned; bump `SCHEMA_VERSION` and `web/src/api/types.ts` together). Results in `output.rs`. Build: `scripts/build-wasm.sh`. `scripts/build-wasm.sh` builds both: threaded (nightly, `parallel` feature, `wasm-bindgen-rayon`; needs cross-origin isolation) into `web/src/wasm`, and single-threaded (stable) into `web/src/wasm-single`; pass `threads` or `single` for one. The worker picks at runtime.
