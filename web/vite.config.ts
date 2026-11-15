import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';

declare const process: { env: Record<string, string | undefined> };
// `npm run` exports the package version.
const version = process.env.npm_package_version ?? '2.0.0';

// Cross-origin isolation, needed for WASM threads (SharedArrayBuffer); see docs/DESIGN.md.
const isolation = {
  'Cross-Origin-Opener-Policy': 'same-origin',
  'Cross-Origin-Embedder-Policy': 'require-corp',
};

export default defineConfig({
  plugins: [svelte()],
  define: { __APP_VERSION__: JSON.stringify(version) },
  worker: { format: 'es' },
  server: { headers: isolation },
  preview: { headers: isolation },
  base: './',
});
