// Saving and loading the case set-up as a JSON file. Built and read in the browser; nothing is uploaded.
import { defaults, type FormState } from './form';

export const CASE_FORMAT = 'basetherm-case';
export const CASE_VERSION = 1;

export interface CaseFile {
  format: typeof CASE_FORMAT;
  version: number;
  appVersion: string;
  savedAt: string;
  form: FormState;
  /** The climate file, embedded so a case is self-contained. */
  epw: { name: string; text: string } | null;
}

export function serializeCase(form: FormState, epw: { name: string; text: string } | null): string {
  const file: CaseFile = {
    format: CASE_FORMAT,
    version: CASE_VERSION,
    appVersion: __APP_VERSION__,
    savedAt: new Date().toISOString(),
    form: JSON.parse(JSON.stringify(form)) as FormState,
    epw,
  };
  return JSON.stringify(file, null, 1);
}

const isObj = (v: unknown): v is Record<string, unknown> => typeof v === 'object' && v !== null && !Array.isArray(v);
const isPt = (p: unknown) => isObj(p) && Number.isFinite(p.x) && Number.isFinite(p.y);
const isPoly = (p: unknown) => Array.isArray(p) && p.every(isPt);

/** Fill what a file lacks (older versions, hand edits) from the defaults, keeping what it has. */
function merge<T>(base: T, over: unknown): T {
  if (Array.isArray(base)) return (Array.isArray(over) ? over : base) as T;
  if (isObj(base)) {
    if (!isObj(over)) return base;
    const out: Record<string, unknown> = {};
    for (const k of Object.keys(base)) out[k] = merge((base as Record<string, unknown>)[k], over[k]);
    return out as T;
  }
  return (typeof over === typeof base ? over : base) as T;
}

/** Parse and check a case file; throws an Error with a readable message. */
export function parseCase(text: string): { form: FormState; epw: { name: string; text: string } | null } {
  let raw: unknown;
  try {
    raw = JSON.parse(text);
  } catch {
    throw new Error('This is not a Basetherm case file (not valid JSON).');
  }
  if (!isObj(raw) || raw.format !== CASE_FORMAT || !isObj(raw.form)) throw new Error('This is not a Basetherm case file.');
  if (typeof raw.version !== 'number' || raw.version > CASE_VERSION) {
    throw new Error('This case was saved by a newer version of Basetherm.');
  }
  const f = raw.form;
  if (!isPoly(f.footprint)) throw new Error('The case file has a damaged outline.');
  const levels = f.levels;
  if (!Array.isArray(levels) || !levels.length) throw new Error('The case file has no levels.');
  for (const l of levels) {
    if (!isObj(l) || typeof l.name !== 'string' || !Number.isFinite(l.height) || !Array.isArray(l.rooms)) {
      throw new Error('The case file has a damaged level.');
    }
    for (const r of l.rooms) {
      if (!isObj(r) || typeof r.id !== 'string' || !isPoly(r.poly) || !Number.isFinite(r.temp)) {
        throw new Error('The case file has a damaged room.');
      }
      if (!Array.isArray(r.monthly) || r.monthly.length !== 12 || !r.monthly.every(Number.isFinite)) {
        throw new Error('The case file has a damaged room temperature schedule.');
      }
    }
  }
  if (!Array.isArray(f.soil) || !f.soil.length || !f.soil.every((s) => isObj(s) && Number.isFinite(s.top))) {
    throw new Error('The case file has damaged soil layers.');
  }
  const form = merge(structuredClone(defaults), f);
  form.footprint = f.footprint as FormState['footprint'];
  form.levels = levels as FormState['levels'];
  // Older files: freezing was a per-layer tickbox and the properties of a class were not stored.
  const old = f.soil as (FormState['soil'][number] & { freezing?: boolean })[];
  form.soil = old.map(({ freezing: _freezing, ...s }) =>
    s.preset === 'custom' && Number.isFinite(s.k) && Number.isFinite(s.cv)
      ? { ...s, kFrozen: s.kFrozen ?? s.k, cvFrozen: s.cvFrozen ?? s.cv, latent: s.latent ?? 0 }
      : s,
  );
  // Older files could cover only the middle of the floor; that option was removed.
  if ((form.floorInsulation.coverage as string) === 'core') form.floorInsulation.coverage = 'all';
  form.simulateFreezing =
    typeof f.simulateFreezing === 'boolean' ? f.simulateFreezing : old.some((s) => s.freezing === true);
  const e = raw.epw;
  const epw = isObj(e) && typeof e.text === 'string' && typeof e.name === 'string' ? { name: e.name, text: e.text } : null;
  return { form, epw };
}
