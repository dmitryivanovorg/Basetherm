// Runs the WASM solver off the main thread. A run is one synchronous call, so it cannot be
// interrupted from inside; the client cancels by terminating this worker.
import type { WorkerRequest, WorkerResponse } from './types';

type Solver = Pick<typeof import('../wasm-single/basetherm'), 'epwSummary' | 'runProject' | 'meshInfo' | 'soilPresets' | 'soilProperties'>;

// Threads need SharedArrayBuffer, i.e. a cross-origin isolated page. Without it (or if the threaded
// module fails to load or start) fall back to the single-threaded build (scripts/build-wasm.sh single).
async function load(): Promise<Solver> {
  // The worker named 'mesh' only previews meshes: no thread pool for it.
  if (self.name !== 'mesh' && self.crossOriginIsolated && typeof SharedArrayBuffer !== 'undefined') {
    try {
      const wasm = await import('../wasm/basetherm');
      await wasm.default();
      // ?threads=N on the worker URL overrides the pool size (benchmarks, determinism checks).
      const asked = Number(new URLSearchParams(self.location.search).get('threads'));
      const n = asked >= 1 ? asked : navigator.hardwareConcurrency || 1;
      await (wasm as { initThreadPool?: (n: number) => Promise<unknown> }).initThreadPool?.(
        Math.max(1, Math.floor(n)),
      );
      return wasm;
    } catch (err) {
      console.warn('Threaded solver unavailable, using the single-threaded build:', err);
    }
  }
  const wasm = await import('../wasm-single/basetherm');
  await wasm.default();
  return wasm;
}
const ready = load();

function post(m: WorkerResponse, transfer: Transferable[] = []) {
  postMessage(m, transfer);
}

onmessage = async (e: MessageEvent<WorkerRequest>) => {
  const req = e.data;
  try {
    const { epwSummary, runProject, meshInfo, soilPresets, soilProperties } = await ready;
    if (req.kind === 'epw') {
      post({ id: req.id, kind: 'epw', summary: JSON.parse(epwSummary(req.text)) });
    } else if (req.kind === 'soil-presets') {
      post({ id: req.id, kind: 'soil-presets', presets: JSON.parse(soilPresets()) });
    } else if (req.kind === 'soil') {
      post({ id: req.id, kind: 'soil', properties: JSON.parse(soilProperties(req.spec)) });
    } else if (req.kind === 'mesh') {
      post({ id: req.id, kind: 'mesh', info: JSON.parse(meshInfo(req.project)) });
    } else {
      const result = runProject(req.project, req.epw, (stage: string, year: number, fraction: number) =>
        post({ id: req.id, kind: 'progress', progress: { stage: stage as never, year, fraction } }),
      );
      const hourly = result.hourly();
      const summary = JSON.parse(result.summary());
      const sections = {
        ...JSON.parse(result.sectionAxes()),
        longitudinal: result.longitudinal(),
        perpendicular: result.perpendicular(),
        fluxLongitudinal: result.fluxLongitudinal(),
        fluxPerpendicular: result.fluxPerpendicular(),
      };
      result.free();
      post({ id: req.id, kind: 'done', output: { summary, hourly, sections } }, [
        hourly.buffer,
        sections.longitudinal.buffer,
        sections.perpendicular.buffer,
        sections.fluxLongitudinal.buffer,
        sections.fluxPerpendicular.buffer,
      ]);
    }
  } catch (err) {
    post({ id: req.id, kind: 'error', message: err instanceof Error ? err.message : String(err) });
  }
};
