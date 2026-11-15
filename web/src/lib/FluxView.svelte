<script lang="ts">
  // Monthly mean heat flux on the two centre planes: cells coloured by flux magnitude, arrows showing
  // its direction (heat flows from warm to cold; length grows with the square root of the magnitude).
  // One colour scale for both planes and all months. Sign convention as elsewhere: a downward
  // arrow is heat flowing from the surface into deeper ground.
  import type { Sections } from '../api/types';
  import { canvasBlob, csv, download } from './export';
  import { MONTHS } from './form';

  let { sections, lx, ly, depth, wall, floor }: {
    sections: Sections;
    lx: number;
    ly: number;
    depth: number;
    wall: number;
    floor: number;
  } = $props();

  let month = $state(0);
  let near = $state(true);
  let arrows = $state(true);

  // Single-hue sequential ramp, light to dark.
  const STOPS: [number, number, number][] = [[255, 245, 240], [252, 187, 161], [251, 106, 74], [203, 24, 29], [103, 0, 13]];
  const colour = (u: number) => {
    const v = Math.min(1, Math.max(0, u)) * (STOPS.length - 1);
    const i = Math.min(STOPS.length - 2, Math.floor(v)), f = v - i;
    const [a, b] = [STOPS[i]!, STOPS[i + 1]!];
    return `rgb(${[0, 1, 2].map((c) => Math.round(a[c]! + (b[c]! - a[c]!) * f)).join(',')})`;
  };
  const legend = `linear-gradient(to right, ${STOPS.map((s, i) => `rgb(${s}) ${(100 * i) / (STOPS.length - 1)}%`).join(',')})`;

  const planes = $derived([
    { key: 'longitudinal', title: 'Longitudinal section (along x, through the middle in y)', axis: sections.x, data: sections.fluxLongitudinal, len: lx, h: 'x' },
    { key: 'perpendicular', title: 'Perpendicular section (along y, through the middle in x)', axis: sections.y, data: sections.fluxPerpendicular, len: ly, h: 'y' },
  ]);
  type Plane = (typeof planes)[number];
  type View = { x0: number; x1: number };

  const nz = $derived(sections.z.length - 1);
  const zEnd = $derived(near ? Math.min(sections.z.at(-1)!, depth + floor + 8) : sections.z.at(-1)!);
  const viewOf = (p: Plane): View => (near ? { x0: -wall - 6, x1: p.len + wall + 6 } : { x0: p.axis[0]!, x1: p.axis.at(-1)! });

  // Colour range: from 0 to the largest magnitude in the visible window of any plane and month, so
  // far-field cells that carry almost no flux do not stretch it, but the building's walls do.
  const qmax = $derived.by(() => {
    let m = 0;
    for (const p of planes) {
      const v = viewOf(p), n = p.axis.length - 1;
      for (let mo = 0; mo < 12; mo++)
        for (let i = 0; i < n; i++) {
          if (p.axis[i + 1]! < v.x0 || p.axis[i]! > v.x1) continue;
          for (let k = 0; k < nz; k++) {
            if (sections.z[k]! > zEnd) break;
            const q = (mo * n * nz + i * nz + k) * 2, a = p.data[q]!, b = p.data[q + 1]!;
            if (a === a) m = Math.max(m, Math.hypot(a, b));
          }
        }
    }
    return Math.max(m, 1e-9);
  });
  let cap = $state<number | null>(null);
  const top = $derived(cap && cap > 0 ? cap : qmax);

  let hover = $state<{ plane: string; x: number; z: number; q: [number, number]; px: number; py: number } | null>(null);
  const canvases: Record<string, HTMLCanvasElement> = {};

  interface Args { axis: number[]; data: Float64Array; m: number; view: View; zMax: number; top: number; arrows: boolean }

  function arrow(ctx: CanvasRenderingContext2D, x: number, y: number, dx: number, dy: number, head: number) {
    const l = Math.hypot(dx, dy), ux = dx / l, uy = dy / l;
    const [x1, y1] = [x + dx, y + dy];
    ctx.beginPath();
    ctx.moveTo(x, y);
    ctx.lineTo(x1, y1);
    ctx.moveTo(x1, y1);
    ctx.lineTo(x1 - head * (ux * 0.87 - uy * 0.5), y1 - head * (uy * 0.87 + ux * 0.5));
    ctx.moveTo(x1, y1);
    ctx.lineTo(x1 - head * (ux * 0.87 + uy * 0.5), y1 - head * (uy * 0.87 - ux * 0.5));
    ctx.stroke();
  }

  function draw(canvas: HTMLCanvasElement, a: Args) {
    const W = canvas.width, H = canvas.height, { view, zMax, m } = a;
    const ctx = canvas.getContext('2d')!;
    ctx.clearRect(0, 0, W, H);
    const px = (v: number) => ((v - view.x0) / (view.x1 - view.x0)) * W;
    const pz = (v: number) => (v / zMax) * H;
    const n = a.axis.length - 1, base = m * n * nz;
    const q = (i: number, k: number): [number, number] => [a.data[(base + i * nz + k) * 2]!, a.data[(base + i * nz + k) * 2 + 1]!];
    for (let i = 0; i < n; i++) {
      if (a.axis[i + 1]! < view.x0 || a.axis[i]! > view.x1) continue;
      const xa = Math.floor(px(a.axis[i]!)), xb = Math.ceil(px(a.axis[i + 1]!));
      for (let k = 0; k < nz; k++) {
        const za = sections.z[k]!;
        if (za > zMax) break;
        const [h, v] = q(i, k);
        ctx.fillStyle = h === h ? colour(Math.hypot(h, v) / a.top) : '#f4f5f7';
        ctx.fillRect(xa, Math.floor(pz(za)), xb - xa, Math.ceil(pz(sections.z[k + 1]!)) - Math.floor(pz(za)));
      }
    }
    // Outline of the building interior and grade.
    ctx.strokeStyle = '#111';
    ctx.lineWidth = 2;
    ctx.beginPath();
    const isVoid = (i: number, k: number) => q(i, k)[0] !== q(i, k)[0];
    for (let i = 0; i < n; i++)
      for (let k = 0; k < nz; k++) {
        if (sections.z[k]! > zMax) break;
        const v = isVoid(i, k);
        if (i + 1 < n && v !== isVoid(i + 1, k)) { ctx.moveTo(px(a.axis[i + 1]!), pz(sections.z[k]!)); ctx.lineTo(px(a.axis[i + 1]!), pz(sections.z[k + 1]!)); }
        if (k + 1 < nz && v !== isVoid(i, k + 1)) { ctx.moveTo(px(a.axis[i]!), pz(sections.z[k + 1]!)); ctx.lineTo(px(a.axis[i + 1]!), pz(sections.z[k + 1]!)); }
      }
    ctx.stroke();
    ctx.beginPath();
    ctx.moveTo(0, 1);
    ctx.lineTo(W, 1);
    ctx.stroke();
    if (!a.arrows) return;
    // Arrows on a regular screen grid: the flux of the cell under each node. Dark on light cells,
    // white on dark ones, with a thin casing so they read on every colour.
    const step = Math.max(34, Math.round(W / 40)), longest = step * 0.85, minLen = step * 0.18;
    for (let sy = step / 2; sy < H; sy += step)
      for (let sx = step / 2; sx < W; sx += step) {
        const xv = view.x0 + (sx / W) * (view.x1 - view.x0), zv = (sy / H) * zMax;
        const i = a.axis.findIndex((f, c) => f <= xv && xv < a.axis[c + 1]!);
        const k = sections.z.findIndex((f, c) => f <= zv && zv < sections.z[c + 1]!);
        if (i < 0 || k < 0 || k >= nz) continue;
        const [h, v] = q(i, k), mag = Math.hypot(h, v);
        if (!(mag === mag) || mag < a.top * 1e-4) continue;
        const len = Math.max(minLen, longest * Math.sqrt(Math.min(1, mag / a.top)));
        const [dx, dy] = [(h / mag) * len, (v / mag) * len];
        for (const [w, c] of [[4.2, 'rgba(255,255,255,0.85)'], [1.8, '#0b1f33']] as const) {
          ctx.strokeStyle = c;
          ctx.lineWidth = w;
          ctx.lineCap = 'round';
          arrow(ctx, sx - dx / 2, sy - dy / 2, dx, dy, Math.max(6, len * 0.32));
        }
      }
  }

  function canvasAction(node: HTMLCanvasElement, args: Args) {
    draw(node, args);
    return { update: (a: Args) => draw(node, a) };
  }

  function move(e: PointerEvent, p: Plane, view: View) {
    const c = e.currentTarget as HTMLCanvasElement, r = c.getBoundingClientRect();
    const x = view.x0 + ((e.clientX - r.left) / r.width) * (view.x1 - view.x0);
    const z = ((e.clientY - r.top) / r.height) * zEnd;
    const i = p.axis.findIndex((f, k) => f <= x && x < p.axis[k + 1]!);
    const k = sections.z.findIndex((f, q) => f <= z && z < sections.z[q + 1]!);
    const o = i >= 0 && k >= 0 && k < nz ? (month * (p.axis.length - 1) * nz + i * nz + k) * 2 : -1;
    hover = { plane: p.key, x, z, q: o >= 0 ? [p.data[o]!, p.data[o + 1]!] : [NaN, NaN], px: e.clientX - r.left, py: e.clientY - r.top };
  }

  async function exportPng(p: Plane) {
    const src = canvases[p.key];
    if (src) download(`heat-flux-${p.key}-${MONTHS[month]!.toLowerCase()}.png`, await canvasBlob(src));
  }

  function exportCsv(p: Plane) {
    const n = p.axis.length - 1;
    const rows: (string | number)[][] = [['month', 'distance_m', 'depth_m', `flux_along_${p.h}_W_m2`, 'flux_down_W_m2']];
    for (let m = 0; m < 12; m++)
      for (let i = 0; i < n; i++)
        for (let k = 0; k < nz; k++) {
          const o = (m * n * nz + i * nz + k) * 2;
          rows.push([MONTHS[m]!, (p.axis[i]! + p.axis[i + 1]!) / 2, (sections.z[k]! + sections.z[k + 1]!) / 2, p.data[o]!, p.data[o + 1]!]);
        }
    download(`heat-flux-${p.key}.csv`, csv(rows), 'text/csv');
  }
</script>

<div class="bar">
  <div class="months" role="group" aria-label="Month">
    {#each MONTHS as m, i}<button type="button" class:on={i === month} onclick={() => (month = i)}>{m}</button>{/each}
  </div>
  <label><input type="checkbox" bind:checked={near} /> Near the building</label>
  <label><input type="checkbox" bind:checked={arrows} /> Arrows</label>
</div>
<div class="scale">
  <span>0</span>
  <div class="ramp" style:background={legend}></div>
  <span>{top.toFixed(1)} W/m²</span>
  <label title="Saturate the colours above this magnitude (W/m²); empty = the largest value">
    cap at <input type="number" min="0" step="1" placeholder={qmax.toFixed(1)} value={cap ?? ''} oninput={(e) => (cap = e.currentTarget.valueAsNumber || null)} /> W/m²
  </label>
  <span class="note">Colour: flux magnitude. Arrows: direction of the heat flow (length ∝ √magnitude); grey cells: building interior. Monthly means of the final hourly year.</span>
</div>

{#each planes as p (p.key)}
  {@const view = viewOf(p)}
  <figure>
    <figcaption>
      <span>{p.title}, {MONTHS[month]}</span>
      <span class="exp"><button type="button" onclick={() => exportPng(p)}>PNG</button><button type="button" onclick={() => exportCsv(p)}>CSV</button></span>
    </figcaption>
    <div class="cv">
      <canvas
        bind:this={canvases[p.key]}
        width={1200}
        height={Math.max(60, Math.round((1200 * zEnd) / (view.x1 - view.x0)))}
        style:aspect-ratio="{view.x1 - view.x0} / {zEnd}"
        use:canvasAction={{ axis: p.axis, data: p.data, m: month, view, zMax: zEnd, top, arrows }}
        onpointermove={(e) => move(e, p, view)}
        onpointerleave={() => (hover = null)}
      ></canvas>
      {#if hover && hover.plane === p.key && hover.q[0] === hover.q[0]}
        <div class="tip" style:left="{hover.px + 12}px" style:top="{hover.py + 12}px">
          {Math.hypot(...hover.q).toFixed(2)} W/m²<br />along {p.h}: {hover.q[0].toFixed(2)}, down: {hover.q[1].toFixed(2)}<br />at {hover.x.toFixed(1)} m, depth {hover.z.toFixed(1)} m
        </div>
      {/if}
    </div>
  </figure>
{/each}

<style>
  .bar { display: flex; align-items: center; flex-wrap: wrap; gap: 0.5rem 1rem; margin-bottom: 0.5rem; }
  .months { display: flex; gap: 2px; }
  .months button { padding: 0.15rem 0.45rem; }
  .months button.on { background: #1f4e9c; color: #fff; border-color: #1f4e9c; }
  .scale { display: flex; align-items: center; gap: 0.5rem; font-size: 0.8rem; margin-bottom: 0.6rem; flex-wrap: wrap; }
  .scale input[type='number'] { width: 5rem; }
  .ramp { width: 14rem; height: 0.7rem; border: 1px solid #ccc; }
  .note { color: #666; }
  figure { margin: 0 0 1rem; }
  figcaption { display: flex; justify-content: space-between; align-items: center; font-weight: 600; font-size: 0.9rem; margin-bottom: 0.2rem; }
  .exp { display: flex; gap: 4px; font-weight: 400; }
  .exp button { padding: 0.05rem 0.5rem; font-size: 0.8rem; }
  .cv { position: relative; }
  canvas { width: 100%; height: auto; display: block; border: 1px solid #ccc; }
  .tip { position: absolute; pointer-events: none; background: #fff; border: 1px solid #999; padding: 2px 6px; font-size: 0.8rem; white-space: nowrap; }
</style>
