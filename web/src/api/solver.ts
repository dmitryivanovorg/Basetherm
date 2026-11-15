// Typed client for the solver worker. The UI never touches solver memory, only these results.
import type { EpwSummary, MeshInfo, Progress, Project, RunOutput, WorkerRequest, WorkerResponse } from './types';

type Body = WorkerRequest extends infer R ? (R extends unknown ? Omit<R, 'id'> : never) : never;

export class Cancelled extends Error {
  constructor() {
    super('cancelled');
  }
}

export class SolverClient {
  private worker: Worker | null = null;
  private nextId = 1;
  private reject: ((e: Error) => void) | null = null;

  private ensure(): Worker {
    this.worker ??= new Worker(new URL('./solver.worker.ts', import.meta.url), { type: 'module' });
    return this.worker;
  }

  private call<T>(req: Body, onProgress?: (p: Progress) => void) {
    const worker = this.ensure();
    const id = this.nextId++;
    return new Promise<T>((resolve, reject) => {
      this.reject = reject;
      worker.onmessage = (e: MessageEvent<WorkerResponse>) => {
        const m = e.data;
        if (m.id !== id) return;
        if (m.kind === 'progress') onProgress?.(m.progress);
        else if (m.kind === 'error') reject(new Error(m.message));
        else if (m.kind === 'epw') resolve(m.summary as T);
        else if (m.kind === 'mesh') resolve(m.info as T);
        else if (m.kind === 'done') resolve(m.output as T);
      };
      worker.onerror = (e) => reject(new Error(e.message || 'solver worker failed'));
      worker.postMessage({ ...req, id });
    });
  }

  epwSummary(text: string): Promise<EpwSummary> {
    return this.call({ kind: 'epw', text });
  }

  run(project: Project, epw: string, onProgress: (p: Progress) => void): Promise<RunOutput> {
    return this.call({ kind: 'run', project: JSON.stringify(project), epw }, onProgress);
  }

  /** Stop the current run by discarding the worker; the next call starts a fresh one. */
  cancel() {
    this.worker?.terminate();
    this.worker = null;
    this.reject?.(new Cancelled());
    this.reject = null;
  }
}
