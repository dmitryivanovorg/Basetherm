// Form state (UI units: m, mm, C) and its conversion to the solver project (SI).
import type { Layer, LayerLimit, Material, Project, Room } from '../api/types';
import { area, bounds, clean, isRectilinear, type Pt } from './geometry';

export interface RoomForm {
  id: string;
  name: string;
  poly: Pt[];
  mode: 'constant' | 'monthly';
  temp: number; // C, constant
  monthly: number[]; // C, January to December
}

export interface LevelForm {
  name: string;
  height: number; // m
  rooms: RoomForm[];
}

export interface SoilForm {
  top: number; // m below grade
  /** `custom` (k and cv below) or the id of a class of the soil table. */
  preset: string;
  /** Degree of saturation of a table soil, 0..1; only picks the defaults of the properties below. */
  saturation: number;
  /** Properties used by the solver. A class and saturation fill them in as defaults; the user may overwrite any. */
  k?: number; // thawed conductivity, W/(m K)
  kFrozen?: number;
  cv?: number; // thawed volumetric heat capacity, J/(m3 K)
  cvFrozen?: number;
  latent?: number; // J/m3, used when the freeze/thaw cycle is simulated
}

export const soilFilled = (s: SoilForm): s is Required<SoilForm> =>
  [s.k, s.kFrozen, s.cv, s.cvFrozen, s.latent].every((v) => Number.isFinite(v));

export interface SolidForm {
  mm: number;
  k: number;
  cv: number;
}

export interface InsulationForm extends SolidForm {
  enabled: boolean;
}

export interface FormState {
  /** Interior outline of the building, shared by all levels; empty until drawn. */
  footprint: Pt[];
  /** Slab on grade: one floor at grade (the first level's rooms), no basement walls. */
  slab: boolean;
  /** Basement levels from grade downward. */
  levels: LevelForm[];
  seq: number;
  soil: SoilForm[];
  /** Simulate the freeze/thaw cycle: frozen properties and latent heat are used below 0 C. */
  simulateFreezing: boolean;
  wall: SolidForm;
  floor: SolidForm;
  hWall: number;
  hFloor: number;
  /** Vertical insulation on the walls; `full` = over the whole basement depth. */
  wallInsulation: InsulationForm & { position: 'outside' | 'inside'; full: boolean; depth: number };
  /** `coverage`: all of the floor, or a strip of `width` along the walls. */
  floorInsulation: InsulationForm & {
    position: 'below' | 'above';
    coverage: 'all' | 'perimeter';
    width: number;
  };
  /** Horizontal insulation next to the outer wall face. */
  groundInsulation: InsulationForm & { width: number; depth: number };
  margin: number;
  depthBelowFloor: number;
  preset: Project['preset'];
  albedo: number;
  emissivity: number;
  /** Daily-step warm-up before the hourly years. */
  warmUp: boolean;
  coarseYears: number;
  hourlyYears: number;
  /** Start from the periodic solution; off: the undisturbed ground profile. */
  periodicInit: boolean;
  /** Convergence target, % of the peak hourly heat loss: the run stops when it changes less than this between hourly years. */
  convergence: number;
  /** Couple the domain sides and bottom to the undisturbed ground column; off: they are adiabatic. */
  conditionEdges: boolean;
}

export const MONTHS = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'];

// Illustrative starting values for the construction; soil classes come from the soil table (docs/DESIGN.md).
export const defaults: FormState = {
  footprint: [],
  slab: false,
  levels: [{ name: 'Basement', height: 2.4, rooms: [] }],
  seq: 0,
  // The properties are filled in from the class when the app starts (lib/soilDefaults.ts).
  soil: [{ top: 0, preset: 'silt', saturation: 0.7 }],
  simulateFreezing: false,
  wall: { mm: 250, k: 1.4, cv: 2.0e6 },
  floor: { mm: 150, k: 1.4, cv: 2.0e6 },
  hWall: 8,
  hFloor: 6,
  wallInsulation: { enabled: false, mm: 100, k: 0.035, cv: 3.0e4, position: 'outside', full: true, depth: 1 },
  floorInsulation: { enabled: false, mm: 100, k: 0.035, cv: 3.0e4, position: 'below', coverage: 'all', width: 1.2 },
  groundInsulation: { enabled: false, mm: 50, k: 0.035, cv: 3.0e4, width: 1, depth: 0.3 },
  margin: 10,
  depthBelowFloor: 15,
  preset: 'draft',
  albedo: 0.25,
  emissivity: 0.9,
  warmUp: true,
  periodicInit: true,
  coarseYears: 20,
  hourlyYears: 3,
  convergence: 0.1,
  conditionEdges: true,
};

export function newRoom(f: FormState, poly: Pt[], like?: RoomForm): RoomForm {
  f.seq++;
  return {
    id: `r${f.seq}`,
    name: `Room ${f.seq}`,
    poly: poly.map((p) => ({ ...p })),
    mode: like?.mode ?? 'constant',
    temp: like?.temp ?? 20,
    monthly: [...(like?.monthly ?? Array<number>(12).fill(like?.temp ?? 20))],
  };
}

/** Start a new shape: every level becomes one room covering it. */
export function setFootprint(f: FormState, poly: Pt[]) {
  f.footprint = poly;
  f.seq = 0;
  for (const l of f.levels) {
    const room = newRoom(f, poly, l.rooms[0]);
    room.name = 'Room 1';
    l.rooms = [room];
  }
}

/** A level with the same shape as the others, as a single room. */
export function addLevel(f: FormState) {
  const last = f.levels.at(-1);
  let n = f.levels.length + 1;
  while (f.levels.some((l) => l.name === `Level ${n}`)) n++;
  const room = newRoom(f, f.footprint, last?.rooms[0]);
  room.name = 'Room 1';
  f.levels.push({ name: `Level ${n}`, height: last?.height ?? 2.4, rooms: [room] });
}

export function resetLevel(f: FormState, level: number) {
  const l = f.levels[level];
  if (!l) return;
  const room = newRoom(f, f.footprint, l.rooms[0]);
  room.name = 'Room 1';
  l.rooms = [room];
}

/** Copy the room layout (names, shapes, temperatures) of one level onto another. */
export function copyLayout(f: FormState, from: number, to: number) {
  const src = f.levels[from], dst = f.levels[to];
  if (!src || !dst || from === to) return;
  dst.rooms = src.rooms.map((r) => {
    f.seq++;
    return { ...(JSON.parse(JSON.stringify(r)) as RoomForm), id: `r${f.seq}` };
  });
}

/** Rooms in the order the solver reports them: level by level (slab: the first level only). */
export function flatRooms(f: FormState): { level: number; levelName: string; room: RoomForm }[] {
  const levels = f.slab ? f.levels.slice(0, 1) : f.levels;
  return levels.flatMap((l, level) => l.rooms.map((room) => ({ level, levelName: l.name, room })));
}

/** A room name not used on the level yet. */
export function uniqueRoomName(l: LevelForm): string {
  let n = l.rooms.length + 1;
  while (l.rooms.some((r) => r.name === `Room ${n}`)) n++;
  return `Room ${n}`;
}

export const roomArea = (r: RoomForm) => area(r.poly);

export function totalDepth(f: FormState): number {
  return f.slab ? 0 : f.levels.reduce((s, l) => s + l.height, 0);
}

const material = (s: { k: number; cv: number }): Material => ({ k: s.k, cv: s.cv });
const soilMaterial = (s: Required<SoilForm>, freezing: boolean): Material =>
  freezing
    ? { k: s.k, cv: s.cv, kFrozen: s.kFrozen, cvFrozen: s.cvFrozen, latent: s.latent, tFreeze: 0 }
    : { k: s.k, cv: s.cv };
const layer = (s: SolidForm, limit?: LayerLimit): Layer => ({
  thickness: s.mm / 1000,
  material: material(s),
  ...(limit ? { limit } : {}),
});
const r6 = (v: number) => Math.round(v * 1e6) / 1e6;

/** What stops the solver from running this form as drawn (empty = runnable). */
export function blockers(f: FormState): string[] {
  const out: string[] = [];
  if (f.footprint.length < 3) return ['Draw the outline of the building on the plan.'];
  if (!isRectilinear(f.footprint))
    out.push('The solver needs horizontal and vertical walls only; redraw the outline with right angles.');
  else if (flatRooms(f).some((r) => !isRectilinear(r.room.poly)))
    out.push('Rooms need horizontal and vertical walls only; split with horizontal or vertical cuts.');
  if (!(f.convergence > 0)) out.push('The convergence target must be positive.');
  if (!(f.hourlyYears >= 2)) out.push('At least two hourly years are needed to check convergence of the peak loss.');
  const w = f.wallInsulation;
  if (!f.slab && w.enabled && !w.full && !(w.depth > 0)) out.push('Vertical insulation depth must be positive.');
  const fi = f.floorInsulation;
  if (fi.enabled && fi.coverage !== 'all' && !(fi.width > 0)) out.push('The floor insulation width must be positive.');
  for (const s of f.soil) {
    if (s.preset !== 'custom' && !(s.saturation >= 0 && s.saturation <= 1))
      out.push('Soil saturation must be between 0 and 100 %.');
    if (!soilFilled(s)) out.push('Soil properties are loading or incomplete.');
    else if (!(s.k > 0 && s.kFrozen > 0 && s.cv > 0 && s.cvFrozen > 0 && s.latent >= 0))
      out.push('Soil conductivities and heat capacities must be positive, latent heat not negative.');
  }
  return out;
}

const poly = (p: Pt[], ox: number, oy: number): [number, number][] =>
  clean(p).map((q) => [r6(q.x - ox), r6(q.y - oy)]);

const room = (r: RoomForm, ox: number, oy: number): Room => ({
  name: r.name,
  poly: poly(r.poly, ox, oy),
  temperature: r.mode === 'monthly' ? { monthly: [...r.monthly] } : { constant: r.temp },
});

/** The solver project, or null while `blockers(f)` is not empty. */
export function toProject(f: FormState): Project | null {
  if (blockers(f).length) return null;
  const b = bounds(f.footprint);
  const [ox, oy] = [b.x0, b.y0];
  const rooms = (l: LevelForm) => l.rooms.map((r) => room(r, ox, oy));
  const wi = f.wallInsulation, fi = f.floorInsulation, gi = f.groundInsulation;
  const depth = totalDepth(f);
  const wall = [layer(f.wall)];
  if (wi.enabled) {
    const partial = !wi.full && wi.depth < depth - 1e-9;
    wall[wi.position === 'inside' ? 'unshift' : 'push'](
      layer(wi, partial ? { kind: 'depth', value: wi.depth } : undefined),
    );
  }
  const floor = [layer(f.floor)];
  if (fi.enabled) {
    const limit: LayerLimit | undefined =
      fi.coverage === 'all' ? undefined : { kind: 'perimeter', value: fi.width };
    floor[fi.position === 'above' ? 'unshift' : 'push'](layer(fi, limit));
  }
  const shape = poly(f.footprint, ox, oy);
  return {
    version: 2,
    preset: f.preset,
    surface: { albedo: f.albedo, emissivity: f.emissivity, convection: 'waltonRough' },
    options: {
      coarseYears: f.coarseYears,
      hourlyYears: f.hourlyYears,
      driftTol: f.convergence / 100,
      warmUp: f.warmUp,
      periodicInit: f.periodicInit,
      conditionEdges: f.conditionEdges,
    },
    building: {
      lx: r6(b.x1 - b.x0),
      ly: r6(b.y1 - b.y0),
      footprint: shape,
      levels: f.slab ? [] : f.levels.map((l) => ({ name: l.name, height: l.height, rooms: rooms(l) })),
      groundFloorRooms: f.slab ? rooms(f.levels[0]!) : [],
      wall: f.slab ? [] : wall,
      floor,
      hWall: f.hWall,
      hFloor: f.hFloor,
      soil: f.soil.map((s) => ({
        top: s.top,
        material: soilMaterial(s as Required<SoilForm>, f.simulateFreezing),
      })),
      insulation: gi.enabled
        ? { width: gi.width, thickness: gi.mm / 1000, depth: gi.depth, material: material(gi) }
        : undefined,
      domain: { margin: f.margin, depthBelowFloor: f.depthBelowFloor },
    },
  };
}
