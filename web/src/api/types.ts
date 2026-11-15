// Messages between the UI and the solver worker, and the JSON the WASM API reads and writes.
// Keep in step with crates/wasm-api (schema version 2). SI units throughout.

export interface Material {
  k: number; // W/(m K)
  cv: number; // J/(m3 K)
  kFrozen?: number;
  cvFrozen?: number;
  latent?: number; // J/m3; > 0 switches freezing on
  tFreeze?: number; // C
  halfWidth?: number; // K
}

export type Schedule = { constant: number } | { monthly: number[] } | { hourly: number[] };

/** A room: a rectilinear polygon, corners `[x, y]` in interior coordinates, m. */
export interface Room {
  name: string;
  poly: [number, number][];
  temperature: Schedule;
}

/** Where a layer exists (omitted: everywhere): a wall layer to a depth, a floor layer near the edge or in the core. */
export interface LayerLimit {
  kind: 'depth' | 'perimeter' | 'core';
  value: number; // m
}

export interface Layer {
  thickness: number;
  material: Material;
  limit?: LayerLimit;
}

/** A soil class from the table at a degree of saturation (0..1). */
export interface SoilSpec {
  preset: string;
  saturation: number;
  freezing: boolean;
  halfWidth?: number;
}

export interface SoilPreset {
  id: string;
  name: string;
  porosity: number;
  kDry: number;
  kSat: number;
}

export interface SoilProperties {
  k: number;
  kFrozen: number;
  cv: number;
  cvFrozen: number;
  latent: number;
}

export interface Project {
  version: 2;
  preset: 'draft' | 'standard' | 'fine';
  surface: {
    albedo: number;
    emissivity: number;
    convection: 'waltonRough' | { constant: number } | { windLinear: { a: number; b: number } };
  };
  options?: {
    coarseYears?: number;
    hourlyYears?: number;
    /** Convergence target: relative change of the peak hourly heat loss between hourly years (0.005 = 0.5 %). */
    driftTol?: number;
    /** Daily-step warm-up before the hourly years (default true). */
    warmUp?: boolean;
    /** Start from the periodic solution (default true); false: the undisturbed ground profile. */
    periodicInit?: boolean;
    /** Domain sides and bottom follow the undisturbed ground (default true); false: adiabatic. */
    conditionEdges?: boolean;
  };
  building: {
    lx: number;
    ly: number;
    /** Footprint corners `[x, y]`; omitted: the rectangle `[0, lx] x [0, ly]`. */
    footprint?: [number, number][];
    levels: { name: string; height: number; rooms: Room[] }[];
    wall: Layer[];
    floor: Layer[];
    groundFloorRooms: Room[];
    hWall: number;
    hFloor: number;
    soil: { top: number; material?: Material; soil?: SoilSpec }[];
    insulation?: { width: number; thickness: number; depth: number; material: Material };
    domain?: { margin: number; depthBelowFloor: number };
  };
}

export interface Peak {
  watts: number;
  hour: number;
  label: string;
}

export interface Peaks {
  loss: Peak | null;
  gain: Peak | null;
}

/** One simulated year of the run. */
export interface YearRecord {
  /** Hourly year (else a daily-step warm-up year). */
  hourly: boolean;
  /** Annual heat flow of the whole basement, kWh; loss is negative. */
  kwh: number;
  /** Peak hourly heat loss of the whole basement, W (negative); null for a warm-up year. */
  peak: number | null;
  /** Change from the previous year (peak loss for hourly years, annual flow for warm-up years); null for the first year. */
  drift: number | null;
}

export interface Summary {
  history: YearRecord[];
  rooms: {
    name: string;
    floorKwh: number;
    wallKwh: number;
    totalKwh: number;
    peaks: Peaks;
  }[];
  totalKwh: number;
  basementPeaks: Peaks;
  /** Change of the peak hourly heat loss in the last hourly year; null if only one year was run. */
  drift: number | null;
  converged: boolean;
  coarseYears: number;
  hourlyYears: number;
  symmetry: string;
  report: string;
}

export interface EpwSummary {
  city: string;
  country: string;
  latitude: number;
  longitude: number;
  meanAirTemperature: number;
}

/** The mesh a run would use (crates/wasm-api/src/mesh.rs). */
export interface MeshInfo {
  dims: [number, number, number];
  cells: number;
  symmetryX: boolean;
  symmetryY: boolean;
  notes: string[];
  /** Cell faces, m: x and y from the interior corner, z depth below grade. */
  x: number[];
  y: number[];
  z: number[];
  hMin: [number, number, number];
  hMax: [number, number, number];
  warn: boolean;
  overCap: boolean;
}

export interface Progress {
  stage: 'setup' | 'warm-up' | 'hourly';
  year: number;
  fraction: number;
}

/** Monthly mean temperatures (C) on the two centre planes of the building. */
export interface Sections {
  /** Cell faces of the whole domain, m: x and y from the interior corner, z depth below grade. */
  x: number[];
  y: number[];
  z: number[];
  /** Plane y = ly/2: `[month][x cell][depth cell]` flattened; NaN in the building void. */
  longitudinal: Float64Array;
  /** Plane x = lx/2: `[month][y cell][depth cell]` flattened; NaN in the building void. */
  perpendicular: Float64Array;
  /** Monthly mean heat flux, W/m2: `[month][cell][depth cell][along the plane, down]` flattened; NaN in the void. */
  fluxLongitudinal: Float64Array;
  fluxPerpendicular: Float64Array;
}

export interface RunOutput {
  summary: Summary;
  /** Hourly flows in W, [hour][room][floor, wall] flattened; negative is loss. */
  hourly: Float64Array;
  sections: Sections;
}

export type WorkerRequest =
  | { id: number; kind: 'epw'; text: string }
  | { id: number; kind: 'run'; project: string; epw: string }
  | { id: number; kind: 'mesh'; project: string }
  | { id: number; kind: 'soil-presets' }
  | { id: number; kind: 'soil'; spec: string };

export type WorkerResponse =
  | { id: number; kind: 'epw'; summary: EpwSummary }
  | { id: number; kind: 'mesh'; info: MeshInfo }
  | { id: number; kind: 'soil-presets'; presets: SoilPreset[] }
  | { id: number; kind: 'soil'; properties: SoilProperties }
  | { id: number; kind: 'progress'; progress: Progress }
  | { id: number; kind: 'done'; output: RunOutput }
  | { id: number; kind: 'error'; message: string };
