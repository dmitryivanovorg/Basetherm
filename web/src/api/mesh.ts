// Mesh preview client. It has its own single-threaded worker, so previews are answered while a
// run keeps the solver worker busy. Only the newest request matters: older ones are dropped.
import type { MeshInfo, Project, WorkerResponse } from './types';

export class MeshClient {
  private worker: Worker | null = null;
  private nextId = 1;

  private ensure(): Worker {
    this.worker ??= new Worker(new URL('./solver.worker.ts', import.meta.url), {
      type: 'module',
      name: 'mesh',
    });
    return this.worker;
  }

  /** Resolves with the mesh, rejects with the message of a bad project, or with `Superseded`. */
  info(project: Project): Promise<MeshInfo> {
    const worker = this.ensure();
    const id = this.nextId++;
    return new Promise((resolve, reject) => {
      worker.addEventListener('message', function on(e: MessageEvent<WorkerResponse>) {
        const m = e.data;
        if (m.id < id) return;
        worker.removeEventListener('message', on);
        if (m.id > id) reject(new Superseded());
        else if (m.kind === 'mesh') resolve(m.info);
        else if (m.kind === 'error') reject(new Error(m.message));
      });
      worker.postMessage({ kind: 'mesh', id, project: JSON.stringify(project) });
    });
  }
}

export class Superseded extends Error {
  constructor() {
    super('superseded');
  }
}
