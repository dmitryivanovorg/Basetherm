// Plan geometry in metres: polygons as point lists, y up. No UI dependency.

export interface Pt {
  x: number;
  y: number;
}

const EPS = 1e-7;
const round = (v: number) => Math.round(v * 1e6) / 1e6;
const pt = (x: number, y: number): Pt => ({ x: round(x), y: round(y) });

/** Signed area, positive for counter-clockwise. */
export function signedArea(p: Pt[]): number {
  let s = 0;
  for (let i = 0; i < p.length; i++) {
    const a = p[i]!, b = p[(i + 1) % p.length]!;
    s += a.x * b.y - b.x * a.y;
  }
  return s / 2;
}

export const area = (p: Pt[]) => Math.abs(signedArea(p));

export function bounds(p: Pt[]) {
  const xs = p.map((q) => q.x), ys = p.map((q) => q.y);
  return { x0: Math.min(...xs), x1: Math.max(...xs), y0: Math.min(...ys), y1: Math.max(...ys) };
}

export function centroid(p: Pt[]): Pt {
  const a = signedArea(p);
  if (Math.abs(a) < EPS) return pt(p[0]?.x ?? 0, p[0]?.y ?? 0);
  let cx = 0, cy = 0;
  for (let i = 0; i < p.length; i++) {
    const s = p[i]!, t = p[(i + 1) % p.length]!;
    const f = s.x * t.y - t.x * s.y;
    cx += (s.x + t.x) * f;
    cy += (s.y + t.y) * f;
  }
  return { x: cx / (6 * a), y: cy / (6 * a) };
}

/** A point inside the polygon (the centroid if it is inside, else the middle of a scan line). */
export function labelPoint(p: Pt[]): Pt {
  const c = centroid(p);
  if (contains(p, c)) return c;
  const ys = bounds(p);
  const y = (ys.y0 + ys.y1) / 2;
  const xs: number[] = [];
  for (let i = 0; i < p.length; i++) {
    const a = p[i]!, b = p[(i + 1) % p.length]!;
    if ((a.y <= y && b.y > y) || (b.y <= y && a.y > y)) xs.push(a.x + ((y - a.y) / (b.y - a.y)) * (b.x - a.x));
  }
  xs.sort((m, n) => m - n);
  return xs.length >= 2 ? { x: (xs[0]! + xs[1]!) / 2, y } : c;
}

export function contains(p: Pt[], q: Pt): boolean {
  let inside = false;
  for (let i = 0, j = p.length - 1; i < p.length; j = i++) {
    const a = p[i]!, b = p[j]!;
    if (a.y > q.y !== b.y > q.y && q.x < ((b.x - a.x) * (q.y - a.y)) / (b.y - a.y) + a.x) inside = !inside;
  }
  return inside;
}

function cross(o: Pt, a: Pt, b: Pt) {
  return (a.x - o.x) * (b.y - o.y) - (a.y - o.y) * (b.x - o.x);
}

function segmentsCross(a: Pt, b: Pt, c: Pt, d: Pt): boolean {
  const d1 = cross(a, b, c), d2 = cross(a, b, d), d3 = cross(c, d, a), d4 = cross(c, d, b);
  return d1 * d2 < -EPS && d3 * d4 < -EPS;
}

/** Why a closed outline is not a valid shape, or null. */
export function outlineProblem(p: Pt[]): string | null {
  if (p.length < 3) return 'The outline needs at least three corners.';
  if (area(p) < 0.01) return 'The outline encloses no area.';
  for (let i = 0; i < p.length; i++) {
    for (let j = i + 1; j < p.length; j++) {
      const adjacent = j === i + 1 || (i === 0 && j === p.length - 1);
      if (!adjacent && segmentsCross(p[i]!, p[(i + 1) % p.length]!, p[j]!, p[(j + 1) % p.length]!))
        return 'The outline crosses itself.';
    }
  }
  return null;
}

/** Drop repeated and collinear corners. */
export function clean(p: Pt[]): Pt[] {
  let out = p.filter((q, i) => {
    const n = p[(i + 1) % p.length]!;
    return Math.hypot(q.x - n.x, q.y - n.y) > EPS;
  });
  let changed = true;
  while (changed && out.length > 3) {
    changed = false;
    for (let i = 0; i < out.length; i++) {
      const a = out[(i + out.length - 1) % out.length]!, b = out[i]!, c = out[(i + 1) % out.length]!;
      if (Math.abs(cross(a, b, c)) < 1e-9) {
        out = out.filter((_, k) => k !== i);
        changed = true;
        break;
      }
    }
  }
  return out;
}

/** The bounding box if the polygon is an axis-aligned rectangle, else null. */
export function asRectangle(p: Pt[]) {
  const b = bounds(p);
  const onEdge = (q: Pt) =>
    [b.x0, b.x1].some((x) => Math.abs(q.x - x) < 1e-6) || [b.y0, b.y1].some((y) => Math.abs(q.y - y) < 1e-6);
  const full = (b.x1 - b.x0) * (b.y1 - b.y0);
  return full > 0 && p.every(onEdge) && Math.abs(area(p) - full) < 1e-6 * full ? b : null;
}

/** Closest point on the segment `a`-`b` to `q`, and the distance. */
export function snapToSegment(a: Pt, b: Pt, q: Pt) {
  const dx = b.x - a.x, dy = b.y - a.y, l2 = dx * dx + dy * dy;
  const t = l2 === 0 ? 0 : Math.max(0, Math.min(1, ((q.x - a.x) * dx + (q.y - a.y) * dy) / l2));
  const p = { x: a.x + t * dx, y: a.y + t * dy };
  return { p, d: Math.hypot(q.x - p.x, q.y - p.y) };
}

/**
 * Cut a polygon along the infinite line through `a` and `b`. The line must meet the boundary in
 * exactly two points. Returns the two parts, or a reason it failed.
 */
export function splitPolygon(poly: Pt[], a: Pt, b: Pt): { parts: [Pt[], Pt[]] } | { error: string } {
  const len = Math.hypot(b.x - a.x, b.y - a.y);
  if (len < 1e-6) return { error: 'The cut line has no length.' };
  const dist = (q: Pt) => ((b.x - a.x) * (q.y - a.y) - (b.y - a.y) * (q.x - a.x)) / len;
  const ring: { p: Pt; on: boolean }[] = [];
  for (let i = 0; i < poly.length; i++) {
    const p = poly[i]!, q = poly[(i + 1) % poly.length]!;
    const dp = dist(p), dq = dist(q);
    ring.push({ p, on: Math.abs(dp) <= 1e-7 });
    if (Math.abs(dp) > 1e-7 && Math.abs(dq) > 1e-7 && dp * dq < 0) {
      const t = dp / (dp - dq);
      ring.push({ p: pt(p.x + t * (q.x - p.x), p.y + t * (q.y - p.y)), on: true });
    }
  }
  // Runs of consecutive on-line corners (a boundary edge lying on the cut line is one run). A run
  // where the boundary passes from one side of the line to the other is a crossing; one that only
  // touches the line is ignored.
  const n = ring.length;
  const side = (i: number) => Math.sign(dist(ring[i]!.p));
  const runs: { first: number; last: number }[] = [];
  for (let i = 0; i < n; i++) {
    if (!ring[i]!.on || ring[(i + n - 1) % n]!.on) continue;
    let last = i;
    while (ring[(last + 1) % n]!.on) last++;
    if (last - i >= n - 1) return { error: 'The cut runs along the whole boundary.' };
    let before = (i + n - 1) % n, after = (last + 1) % n;
    while (ring[before]!.on) before = (before + n - 1) % n;
    while (ring[after]!.on) after = (after + 1) % n;
    if (side(before) !== side(after)) runs.push({ first: i, last: last % n });
  }
  if (runs.length !== 2) return { error: 'The cut must cross the room boundary exactly twice.' };
  // On a run along an edge, cut at the end that faces the other crossing.
  const [r0, r1] = runs as [(typeof runs)[number], (typeof runs)[number]];
  const closest = (r: typeof r0, o: typeof r0) => {
    const ends = [r.first, r.last];
    const target = [o.first, o.last].map((k) => ring[k]!.p);
    return ends.reduce((best, e) =>
      Math.min(...target.map((t) => Math.hypot(ring[e]!.p.x - t.x, ring[e]!.p.y - t.y))) <
      Math.min(...target.map((t) => Math.hypot(ring[best]!.p.x - t.x, ring[best]!.p.y - t.y)))
        ? e
        : best,
    );
  };
  const [i, j] = [closest(r0, r1), closest(r1, r0)].sort((p, q) => p - q) as [number, number];
  const mid = { x: (ring[i]!.p.x + ring[j]!.p.x) / 2, y: (ring[i]!.p.y + ring[j]!.p.y) / 2 };
  if (!contains(poly, mid) && !poly.some((q) => Math.hypot(q.x - mid.x, q.y - mid.y) < 1e-6))
    return { error: 'The cut runs outside the room.' };
  const one = ring.slice(i, j + 1).map((r) => r.p);
  const two = [...ring.slice(j), ...ring.slice(0, i + 1)].map((r) => r.p);
  const parts = [clean(one), clean(two)] as [Pt[], Pt[]];
  if (parts.some((q) => q.length < 3 || area(q) < 0.01)) return { error: 'A part would be too small.' };
  return { parts };
}

/** Whether every edge of the (cleaned) polygon is horizontal or vertical. */
export function isRectilinear(p: Pt[]): boolean {
  const c = clean(p);
  return c.length >= 4 && c.every((q, i) => {
    const n = c[(i + 1) % c.length]!;
    return Math.abs(q.x - n.x) < 1e-6 || Math.abs(q.y - n.y) < 1e-6;
  });
}
