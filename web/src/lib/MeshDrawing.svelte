<script lang="ts">
  // Plan view of one level (rooms over the mesh lines) and a vertical section through y = ly / 2.
  import type { MeshInfo, Project } from '../api/types';

  let { mesh, building, level }: {
    mesh: MeshInfo;
    building: Project['building'];
    /** Index into `building.levels`; ignored for a slab on grade. */
    level: number;
  } = $props();

  const slab = $derived(building.levels.length === 0);
  const rooms = $derived(slab ? building.groundFloorRooms : (building.levels[level]?.rooms ?? []));
  const depths = $derived(
    building.levels.reduce<number[]>((acc, l) => [...acc, (acc.at(-1) ?? 0) + l.height], []),
  );
  const outline = $derived<[number, number][]>(
    building.footprint ?? [[0, 0], [building.lx, 0], [building.lx, building.ly], [0, building.ly]],
  );
  const path = (p: [number, number][], v: { px: (x: number) => number; py: (y: number) => number }) =>
    `M${p.map(([x, y]) => `${v.px(x)} ${v.py(y)}`).join('L')}Z`;
  /** x-intervals of the footprint on the line `y`. */
  function spans(p: [number, number][], y: number): [number, number][] {
    const xs = p
      .flatMap((a, i) => {
        const b = p[(i + 1) % p.length]!;
        return a[0] === b[0] && y >= Math.min(a[1], b[1]) && y < Math.max(a[1], b[1]) ? [a[0]] : [];
      })
      .sort((u, v) => u - v);
    const out: [number, number][] = [];
    for (let i = 0; i + 1 < xs.length; i += 2) out.push([xs[i]!, xs[i + 1]!]);
    return out;
  }

  const lines = (faces: number[], lo: number, hi: number, vertical: boolean, at: (v: number) => number) =>
    faces
      .map((f) => (vertical ? `M${at(f)} ${lo}V${hi}` : `M${lo} ${at(f)}H${hi}`))
      .join('');

  // Both views are scaled to fit 640 px, with their own aspect ratio.
  const W = 640;
  function view(x: number[], y: number[]) {
    const x0 = x[0] ?? 0, y0 = y[0] ?? 0;
    const w = (x.at(-1) ?? 1) - x0, h = (y.at(-1) ?? 1) - y0;
    const s = W / w;
    return { s, H: Math.max(40, h * s), px: (v: number) => (v - x0) * s, py: (v: number) => (v - y0) * s };
  }
  const plan = $derived(view(mesh.x, mesh.y));
  const sect = $derived(view(mesh.x, mesh.z));
  const rect = (v: ReturnType<typeof view>, a0: number, a1: number, b0: number, b1: number) => ({
    x: v.px(a0), y: v.py(b0), width: v.px(a1) - v.px(a0), height: v.py(b1) - v.py(b0),
  });
  const lx = $derived(building.lx);
  const ly = $derived(building.ly);
  const depth = $derived(depths.at(-1) ?? 0);
</script>

<svg viewBox="0 0 {W} {plan.H}" role="img" aria-label="Mesh plan view">
  <path class="mesh" d={lines(mesh.x, 0, plan.H, true, plan.px) + lines(mesh.y, 0, W, false, plan.py)} />
  <path class="shell" d={path(outline, plan)} />
  {#each rooms as r}
    <path class="room" d={path(r.poly, plan)} />
    <text x={plan.px(r.poly.reduce((s, p) => s + p[0], 0) / r.poly.length)} y={plan.py(r.poly.reduce((s, p) => s + p[1], 0) / r.poly.length)}>{r.name}</text>
  {/each}
  {#if mesh.symmetryX}<line class="plane" x1={plan.px(lx / 2)} x2={plan.px(lx / 2)} y1="0" y2={plan.H} />{/if}
  {#if mesh.symmetryY}<line class="plane" y1={plan.py(ly / 2)} y2={plan.py(ly / 2)} x1="0" x2={W} />{/if}
</svg>

<svg viewBox="0 0 {W} {sect.H}" role="img" aria-label="Mesh section view">
  <path class="mesh" d={lines(mesh.x, 0, sect.H, true, sect.px) + lines(mesh.z, 0, W, false, sect.py)} />
  {#if depth > 0}
    {#each spans(outline, ly / 2) as [a, b]}
      <rect class="void" {...rect(sect, a, b, 0, depth)} />
      {#each depths.slice(0, -1) as d}
        <line class="plane" x1={sect.px(a)} x2={sect.px(b)} y1={sect.py(d)} y2={sect.py(d)} />
      {/each}
    {/each}
  {/if}
  <line class="grade" x1="0" x2={W} y1="0" y2="0" />
</svg>
<p class="legend">
  Plan: mesh lines, footprint, rooms (blue). Section: mesh lines at the left–right extent, building void.
  {#if mesh.symmetryX || mesh.symmetryY}Dashed lines are mirror planes; the mesh covers one side only.{/if}
</p>

<style>
  svg { width: 100%; height: auto; display: block; border: 1px solid var(--line); margin-bottom: 0.5rem; background: var(--surface); }
  .mesh { stroke: #999; stroke-width: 0.4; fill: none; }
  .shell { fill: none; stroke: #444; stroke-width: 1.2; }
  .room { fill: rgba(0, 0, 0, 0.1); stroke: #444; stroke-width: 1; }
  .void { fill: rgba(0, 0, 0, 0.1); stroke: #444; stroke-width: 1; }
  .plane { stroke: #555; stroke-width: 1; stroke-dasharray: 5 3; }
  .grade { stroke: #222; stroke-width: 1.5; }
  text { font-size: 12px; text-anchor: middle; dominant-baseline: middle; fill: #222; }
  .legend { color: #6b6b6b; font-size: 0.85em; margin: 0; }
</style>
