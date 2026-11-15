# Basetherm v2.0.0

Client-side web app simulating heat transfer from a building basement into the ground and air (3D enthalpy conduction with freezing, implicit ADI, hourly climate). Rewrite of the legacy Delphi program.

Full design, methods, validation, budgets, and status: `docs/DESIGN.md`. Read the relevant section before touching the solver.

## Rules

- The solver runs on the client; the server only serves static files. Self-host all assets (no CDNs).
- Solver core has no UI/DOM dependency. SI units internally.
- Never change physics or numerical method silently: propose, wait for approval, then record it in `docs/DESIGN.md`.
- Keep the solver deterministic (bitwise identical for any thread count).
- A numerical change is not done until the validation suite passes (see `docs/DESIGN.md`, Validation).
- Sign convention: heat loss (interior to ground) negative, gain positive.

## Stack and commands

- Rust Cargo workspace in `crates/` (WASM via `wasm-bindgen`); Svelte + TypeScript + Vite UI in `web/`.
- `cargo fmt --all`, `cargo clippy --workspace --all-targets`, `cargo test --workspace` must be clean. `unsafe` is forbidden.
