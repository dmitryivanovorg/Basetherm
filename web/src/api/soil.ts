// Soil class table and derived properties, computed by the solver's own material model (WASM) so
// the UI never duplicates the physics. Single-threaded worker; every request is answered.
import type { SoilPreset, SoilProperties, SoilSpec, WorkerResponse } from './types';

export class SoilClient {
  private worker: Worker | null = null;
  private nextId = 1;
  private waiting = new Map<number, { resolve: (m: WorkerResponse) => void; reject: (e: Error) => void }>();
  private table: Promise<SoilPreset[]> | null = null;

  private ask(msg: object): Promise<WorkerResponse> {
    if (!this.worker) {
      this.worker = new Worker(new URL('./solver.worker.ts', import.meta.url), { type: 'module', name: 'mesh' });
      this.worker.addEventListener('message', (e: MessageEvent<WorkerResponse>) => {
        const w = this.waiting.get(e.data.id);
        if (!w) return;
        this.waiting.delete(e.data.id);
        if (e.data.kind === 'error') w.reject(new Error(e.data.message));
        else w.resolve(e.data);
      });
    }
    const id = this.nextId++;
    return new Promise((resolve, reject) => {
      this.waiting.set(id, { resolve, reject });
      this.worker!.postMessage({ ...msg, id });
    });
  }

  presets(): Promise<SoilPreset[]> {
    this.table ??= this.ask({ kind: 'soil-presets' }).then((m) => (m.kind === 'soil-presets' ? m.presets : []));
    return this.table;
  }

  async properties(spec: SoilSpec): Promise<SoilProperties> {
    const m = await this.ask({ kind: 'soil', spec: JSON.stringify(spec) });
    if (m.kind !== 'soil') throw new Error('unexpected reply');
    return m.properties;
  }
}
export const soilClient = new SoilClient();
