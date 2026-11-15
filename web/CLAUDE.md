# web

TypeScript (strict) + Svelte 5 + Vite UI. One component per file. The solver runs in a Web Worker behind a thin typed interface (`src/api/`); the UI never touches solver arrays directly. Self-host every asset (no CDNs).

- `src/api/`: `types.ts` (project JSON and messages, mirrors `crates/wasm-api`), `solver.worker.ts`, `solver.ts` (client; cancel terminates the worker).
- `src/lib/`: form state and conversion to SI (`form.ts`), components.
- `src/wasm/` (threaded) and `src/wasm-single/` are generated and git-ignored; the worker picks one at runtime: run `npm run wasm` first (needs `wasm-bindgen-cli`, see `docs/DESIGN.md`, Layout and commands).
- Checks: `npm run check` (svelte-check), `npm run build`.
- do not run browser headless, do no make screenshot. let the user check the interface
