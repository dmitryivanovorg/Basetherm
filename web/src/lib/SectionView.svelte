<script lang="ts">
  // Monthly mean temperature on the two centre planes, as coloured cells. One colour scale for both
  // planes and all months, so months can be compared; the range can be set by hand. The freezing line
  // is the 0 °C isotherm. Each plane exports as PNG (with axes and scale) or CSV.
  import type { Sections } from '../api/types';
  import { canvasBlob, csv, download } from './export';
  import { MONTHS } from './form';

  let { sections, lx, ly, depth, wall, floor }: {
    sections: Sections;
    /** Interior size and structure of the run, m. */
    lx: number;
    ly: number;
    /** Depth of the lowest level below grade (0 for slab on grade). */
    depth: number;
    wall: number;
    floor: number;
  } = $props();

  let month = $state(0);
  let near = $state(true);
  let centred = $state(false);
  let custom = $state(false);
  let lo = $state(0);
  let hi = $state(1);
  let isotherms = $state(false);
  let isoInput = $state<number | null | undefined>(null);

  const data = $derived.by(() => {
    let a = Infinity, b = -Infinity;
    for (const arr of [sections.longitudinal, sections.perpendicular])
      for (const v of arr) if (v === v) [a, b] = [Math.min(a, v), Math.max(b, v)];
    return Number.isFinite(a) ? { lo: a, hi: Math.max(b, a + 1) } : { lo: 0, hi: 1 };
  });
  const autoRange = $derived.by(() => {
    if (!centred) return data;
    const m = Math.max(Math.abs(data.lo), Math.abs(data.hi), 1);
    return { lo: -m, hi: m };
  });
  const range = $derived(custom && hi > lo ? { lo, hi } : autoRange);
  const hasFrost = $derived(data.lo < 0);
  // Isotherm interval: the user's value, else a round step giving about 8 lines over the range.
  const isoStep = $derived.by(() => {
    if (typeof isoInput === 'number' && isoInput > 0) return isoInput;
    const raw = (range.hi - range.lo) / 8, p = 10 ** Math.floor(Math.log10(raw));
    return [1, 2, 5, 10].map((q) => q * p).find((s) => s >= raw) ?? p * 10;
  });

  function setCustom(on: boolean) {
    if (on) [lo, hi] = [Math.floor(autoRange.lo), Math.ceil(autoRange.hi)];
    custom = on;
  }

  // Cool-warm diverging ramp: blue, neutral grey, red.
  const STOPS: [number, number, number][] = [[59, 76, 192], [141, 176, 254], [221, 221, 221], [244, 154, 123], [180, 4, 38]];
  function colour(t: number, r: { lo: number; hi: number }): string {
    const u = Math.min(1, Math.max(0, (t - r.lo) / (r.hi - r.lo))) * (STOPS.length - 1);
    const i = Math.min(STOPS.length - 2, Math.floor(u)), f = u - i;
    const [a, b] = [STOPS[i]!, STOPS[i + 1]!];
    return `rgb(${[0, 1, 2].map((c) => Math.round(a[c]! + (b[c]! - a[c]!) * f)).join(',')})`;
  }
  const legend = `linear-gradient(to right, ${STOPS.map((s, i) => `rgb(${s}) ${(100 * i) / (STOPS.length - 1)}%`).join(',')})`;
  const zeroAt = $derived((0 - range.lo) / (range.hi - range.lo));

  const planes = $derived([
    { key: 'longitudinal', title: 'Longitudinal section (along x, through the middle in y)', axis: sections.x, data: sections.longitudinal, len: lx },
    { key: 'perpendicular', title: 'Perpendicular section (along y, through the middle in x)', axis: sections.y, data: sections.perpendicular, len: ly },
  ]);
  type Plane = (typeof planes)[number];
  type View = { x0: number; x1: number };

  const nz = $derived(sections.z.length - 1);
  const zEnd = $derived(near ? Math.min(sections.z.at(-1)!, depth + floor + 8) : sections.z.at(-1)!);
  const viewOf = (p: Plane): View => (near ? { x0: -wall - 6, x1: p.len + wall + 6 } : { x0: p.axis[0]!, x1: p.axis.at(-1)! });

  let hover = $state<{ plane: string; x: number; z: number; t: number; px: number; py: number } | null>(null);
  const canvases: Record<string, HTMLCanvasElement> = {};

  /** Segments of one isotherm: marching squares on the cell-centre values, linear in position. */
  function isotherm(axis: number[], arr: Float64Array, m: number, level = 0): [number, number, number, number][] {
    const n = axis.length - 1, base = m * n * nz;
    const cx = (i: number) => (axis[i]! + axis[i + 1]!) / 2, cz = (k: number) => (sections.z[k]! + sections.z[k + 1]!) / 2;
    const out: [number, number, number, number][] = [];
    for (let i = 0; i + 1 < n; i++)
      for (let k = 0; k + 1 < nz; k++) {
        const v = [arr[base + i * nz + k]!, arr[base + (i + 1) * nz + k]!, arr[base + (i + 1) * nz + k + 1]!, arr[base + i * nz + k + 1]!];
        if (v.some((q) => !(q === q))) continue;
        const pts = [[cx(i), cz(k)], [cx(i + 1), cz(k)], [cx(i + 1), cz(k + 1)], [cx(i), cz(k + 1)]] as const;
        const cut: [number, number][] = [];
        for (let e = 0; e < 4; e++) {
          const [a, b] = [v[e]!, v[(e + 1) % 4]!];
          if (a < level !== b < level) {
            const f = (a - level) / (a - b), [p, q] = [pts[e]!, pts[(e + 1) % 4]!];
            cut.push([p[0] + (q[0] - p[0]) * f, p[1] + (q[1] - p[1]) * f]);
          }
        }
        if (cut.length === 2) out.push([cut[0]![0], cut[0]![1], cut[1]![0], cut[1]![1]]);
        else if (cut.length === 4) out.push([cut[0]![0], cut[0]![1], cut[1]![0], cut[1]![1]], [cut[2]![0], cut[2]![1], cut[3]![0], cut[3]![1]]);
      }
    return out;
  }

  interface Args { axis: number[]; data: Float64Array; len: number; m: number; view: View; zMax: number; r: { lo: number; hi: number }; iso: number | null }

  function draw(canvas: HTMLCanvasElement, a: Args) {
    const W = canvas.width, H = canvas.height, { view, zMax, m, r } = a;
    const ctx = canvas.getContext('2d')!;
    ctx.clearRect(0, 0, W, H);
    const px = (v: number) => ((v - view.x0) / (view.x1 - view.x0)) * W;
    const pz = (v: number) => (v / zMax) * H;
    const n = a.axis.length - 1;
    const base = m * n * nz;
    for (let i = 0; i < n; i++) {
      if (a.axis[i + 1]! < view.x0 || a.axis[i]! > view.x1) continue;
      const xa = Math.floor(px(a.axis[i]!)), xb = Math.ceil(px(a.axis[i + 1]!));
      for (let k = 0; k < nz; k++) {
        const za = sections.z[k]!;
        if (za > zMax) break;
        const t = a.data[base + i * nz + k]!;
        ctx.fillStyle = t === t ? colour(t, r) : '#f4f5f7';
        ctx.fillRect(xa, Math.floor(pz(za)), xb - xa, Math.ceil(pz(sections.z[k + 1]!)) - Math.floor(pz(za)));
      }
    }
    // Isotherms: thin dark lines with their temperature, every `iso` °C (the 0 °C line is drawn below).
    if (a.iso !== null) {
      let tMin = Infinity, tMax = -Infinity;
      for (let i = 0; i < n; i++)
        for (let k = 0; k < nz; k++) {
          const t = a.data[base + i * nz + k]!;
          if (t === t) [tMin, tMax] = [Math.min(tMin, t), Math.max(tMax, t)];
        }
      ctx.font = `${Math.round(W / 70)}px system-ui, sans-serif`;
      ctx.textAlign = 'center';
      ctx.textBaseline = 'middle';
      ctx.lineJoin = 'round';
      for (let q = Math.ceil(tMin / a.iso); q * a.iso <= tMax; q++) {
        const level = Math.round(q * a.iso * 1e6) / 1e6;
        if (level === 0) continue;
        const segs = isotherm(a.axis, a.data, m, level).filter(([x0, , x1]) => x1 >= view.x0 && x0 <= view.x1);
        if (!segs.length) continue;
        ctx.strokeStyle = 'rgba(20,20,20,0.5)';
        ctx.lineWidth = 0.9;
        ctx.setLineDash([5, 3]);
        ctx.beginPath();
        for (const [x0, z0, x1, z1] of segs) { ctx.moveTo(px(x0), pz(z0)); ctx.lineTo(px(x1), pz(z1)); }
        ctx.stroke();
        ctx.setLineDash([]);
        // Label at the right-hand end of the line (inside the plot edge), not over the middle of the image.
        let bx = -Infinity, bz = 0;
        for (const [x0, z0, x1, z1] of segs) {
          if (x0 > bx) [bx, bz] = [x0, z0];
          if (x1 > bx) [bx, bz] = [x1, z1];
        }
        const lx = Math.min(px(bx), W) - 4, lz = Math.min(H - 6, Math.max(6, pz(bz)));
        ctx.textAlign = 'right';
        ctx.strokeStyle = 'rgba(255,255,255,0.85)';
        ctx.lineWidth = 3;
        ctx.strokeText(String(level), lx, lz);
        ctx.fillStyle = '#222';
        ctx.fillText(String(level), lx, lz);
      }
      ctx.textBaseline = 'alphabetic';
    }
    // The freezing line: a dark casing under a white core, visible on every colour.
    const line = isotherm(a.axis, a.data, m);
    ctx.lineCap = 'round';
    for (const [w, c] of [[5, '#0b1f33'], [2.4, '#ffffff']] as const) {
      ctx.strokeStyle = c;
      ctx.lineWidth = w;
      ctx.beginPath();
      for (const [x0, z0, x1, z1] of line) { ctx.moveTo(px(x0), pz(z0)); ctx.lineTo(px(x1), pz(z1)); }
      ctx.stroke();
    }
    // Outline of the building interior (where void and ground cells meet), and grade.
    ctx.strokeStyle = '#111';
    ctx.lineWidth = 2;
    ctx.lineCap = 'butt';
    ctx.beginPath();
    const isVoid = (i: number, k: number) => !(a.data[base + i * nz + k]! === a.data[base + i * nz + k]!);
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
  }

  function canvasAction(node: HTMLCanvasElement, args: Args) {
    draw(node, args);
    return { update: (a: Args) => draw(node, a) };
  }

  function move(e: PointerEvent, p: Plane, view: View) {
    const c = e.currentTarget as HTMLCanvasElement;
    const r = c.getBoundingClientRect();
    const x = view.x0 + ((e.clientX - r.left) / r.width) * (view.x1 - view.x0);
    const z = ((e.clientY - r.top) / r.height) * zEnd;
    const i = p.axis.findIndex((f, k) => f <= x && x < p.axis[k + 1]!);
    const k = sections.z.findIndex((f, q) => f <= z && z < sections.z[q + 1]!);
    const t = i >= 0 && k >= 0 ? p.data[month * (p.axis.length - 1) * nz + i * nz + k]! : NaN;
    hover = { plane: p.key, x, z, t, px: e.clientX - r.left, py: e.clientY - r.top };
  }

  const niceStep = (span: number, count: number) => {
    const raw = span / count, p = 10 ** Math.floor(Math.log10(raw));
    return [1, 2, 5, 10].map((q) => q * p).find((s) => s >= raw) ?? p * 10;
  };

  /** The plane as a PNG with title, axes, colour scale and the freezing-line key. */
  async function exportPng(p: Plane) {
    const src = canvases[p.key];
    if (!src) return;
    const view = viewOf(p), L = 78, R = 24, T = 54, B = 118;
    const out = document.createElement('canvas');
    out.width = src.width + L + R;
    out.height = src.height + T + B;
    const ctx = out.getContext('2d')!;
    ctx.fillStyle = '#fff';
    ctx.fillRect(0, 0, out.width, out.height);
    ctx.drawImage(src, L, T);
    ctx.fillStyle = '#111';
    ctx.font = 'bold 22px system-ui, sans-serif';
    ctx.fillText(`${p.title}, ${MONTHS[month]} (monthly mean)`, L, 34);
    ctx.font = '18px system-ui, sans-serif';
    ctx.strokeStyle = '#555';
    ctx.lineWidth = 1;
    const sx = src.width / (view.x1 - view.x0), sz = src.height / zEnd;
    const xs = niceStep(view.x1 - view.x0, 8), zs = niceStep(zEnd, 6);
    ctx.textAlign = 'center';
    for (let v = Math.ceil(view.x0 / xs) * xs; v <= view.x1 + 1e-9; v += xs) {
      const x = L + (v - view.x0) * sx;
      ctx.beginPath(); ctx.moveTo(x, T + src.height); ctx.lineTo(x, T + src.height + 6); ctx.stroke();
      ctx.fillText(String(Math.round(v * 100) / 100), x, T + src.height + 26);
    }
    ctx.fillText('distance from the interior corner, m', L + src.width / 2, T + src.height + 50);
    ctx.textAlign = 'right';
    for (let v = 0; v <= zEnd + 1e-9; v += zs) {
      const y = T + v * sz;
      ctx.beginPath(); ctx.moveTo(L - 6, y); ctx.lineTo(L, y); ctx.stroke();
      ctx.fillText(String(Math.round(v * 100) / 100), L - 10, y + 6);
    }
    ctx.save();
    ctx.translate(18, T + src.height / 2);
    ctx.rotate(-Math.PI / 2);
    ctx.textAlign = 'center';
    ctx.fillText('depth below grade, m', 0, 0);
    ctx.restore();
    // Colour scale.
    const bx = L, by = T + src.height + 70, bw = Math.min(420, src.width), bh = 16;
    const g = ctx.createLinearGradient(bx, 0, bx + bw, 0);
    STOPS.forEach((s, i) => g.addColorStop(i / (STOPS.length - 1), `rgb(${s})`));
    ctx.fillStyle = g;
    ctx.fillRect(bx, by, bw, bh);
    ctx.strokeRect(bx, by, bw, bh);
    ctx.fillStyle = '#111';
    ctx.textAlign = 'left';
    ctx.fillText(`${range.lo.toFixed(1)} °C`, bx, by + bh + 20);
    ctx.textAlign = 'right';
    ctx.fillText(`${range.hi.toFixed(1)} °C`, bx + bw, by + bh + 20);
    if (zeroAt > 0 && zeroAt < 1) {
      ctx.textAlign = 'center';
      ctx.fillText('0 °C', bx + zeroAt * bw, by + bh + 20);
      ctx.beginPath(); ctx.moveTo(bx + zeroAt * bw, by); ctx.lineTo(bx + zeroAt * bw, by + bh); ctx.stroke();
    }
    // Key of the freezing line.
    const kx = bx + bw + 40, ky = by + bh / 2;
    for (const [w, c] of [[5, '#0b1f33'], [2.4, '#ffffff']] as const) {
      ctx.strokeStyle = c; ctx.lineWidth = w; ctx.lineCap = 'round';
      ctx.beginPath(); ctx.moveTo(kx, ky); ctx.lineTo(kx + 40, ky); ctx.stroke();
    }
    ctx.fillStyle = '#111';
    ctx.textAlign = 'left';
    ctx.fillText('freezing line (0 °C)', kx + 50, ky + 6);
    download(`section-${p.key}-${MONTHS[month]!.toLowerCase()}.png`, await canvasBlob(out));
  }

  function exportCsv(p: Plane) {
    const n = p.axis.length - 1;
    const rows: (string | number)[][] = [['month', 'distance_m', 'depth_m', 'temperature_C']];
    for (let m = 0; m < 12; m++)
      for (let i = 0; i < n; i++)
        for (let k = 0; k < nz; k++)
          rows.push([MONTHS[m]!, (p.axis[i]! + p.axis[i + 1]!) / 2, (sections.z[k]! + sections.z[k + 1]!) / 2, p.data[m * n * nz + i * nz + k]!]);
    download(`section-${p.key}.csv`, csv(rows), 'text/csv');
  }
</script>

<div class="bar">
  <div class="months" role="group" aria-label="Month">
    {#each MONTHS as m, i}<button type="button" class:on={i === month} onclick={() => (month = i)}>{m}</button>{/each}
  </div>
  <label><input type="checkbox" bind:checked={near} /> Near the building</label>
</div>
<div class="range">
  <label><input type="checkbox" bind:checked={isotherms} /> Isotherms</label>
  {#if isotherms}
    <label>every <input type="number" min="0.1" step="0.5" placeholder={String(isoStep)} bind:value={isoInput} /> °C</label>
    <span class="note">Thin labelled lines; the 0 °C line is drawn heavier.</span>
  {/if}
</div>
<div class="range">
  <label><input type="checkbox" checked={custom} onchange={(e) => setCustom(e.currentTarget.checked)} /> Set the temperature range</label>
  {#if custom}
    <label>from <input type="number" step="1" bind:value={lo} /> °C</label>
    <label>to <input type="number" step="1" bind:value={hi} /> °C</label>
    {#if !(hi > lo)}<span class="bad">“to” must exceed “from”</span>{/if}
  {:else}
    <label title="Put the neutral grey of the scale at 0 °C"><input type="checkbox" bind:checked={centred} /> Centre on 0 °C</label>
    <span class="note">Automatic: {range.lo.toFixed(1)} to {range.hi.toFixed(1)} °C (the data span)</span>
  {/if}
</div>
<div class="scale">
  <span>{range.lo.toFixed(1)} °C</span>
  <div class="ramp" style:background={legend}>
    {#if zeroAt > 0 && zeroAt < 1}<i style:left="{100 * zeroAt}%"></i>{/if}
  </div>
  <span>{range.hi.toFixed(1)} °C</span>
  <span class="key"><svg width="34" height="10" aria-hidden="true"><line x1="3" x2="31" y1="5" y2="5" stroke="#0b1f33" stroke-width="5" stroke-linecap="round" /><line x1="3" x2="31" y1="5" y2="5" stroke="#fff" stroke-width="2.4" stroke-linecap="round" /></svg> Freezing line (0 °C)</span>
  <span class="note">Grey cells: building interior.{hasFrost ? '' : ` The ground never reaches 0 °C (lowest monthly mean ${data.lo.toFixed(1)} °C).`}</span>
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
        use:canvasAction={{ axis: p.axis, data: p.data, len: p.len, m: month, view, zMax: zEnd, r: range, iso: isotherms ? isoStep : null }}
        onpointermove={(e) => move(e, p, view)}
        onpointerleave={() => (hover = null)}
      ></canvas>
      {#if hover && hover.plane === p.key && hover.t === hover.t}
        <div class="tip" style:left="{hover.px + 12}px" style:top="{hover.py + 12}px">
          {hover.t.toFixed(1)} °C<br />at {hover.x.toFixed(1)} m, depth {hover.z.toFixed(1)} m
        </div>
      {/if}
    </div>
  </figure>
{/each}

<style>
  .bar { display: flex; justify-content: space-between; align-items: center; flex-wrap: wrap; gap: 0.5rem; margin-bottom: 0.5rem; }
  .months { display: flex; gap: 2px; }
  .months button { padding: 0.15rem 0.45rem; }
  .months button.on { background: #1f4e9c; color: #fff; border-color: #1f4e9c; }
  .range { display: flex; align-items: center; gap: 0.8rem; flex-wrap: wrap; font-size: 0.85rem; margin-bottom: 0.5rem; }
  .range input[type='number'] { width: 4.5rem; }
  .bad { color: #b00020; }
  .scale { display: flex; align-items: center; gap: 0.5rem; font-size: 0.8rem; margin-bottom: 0.6rem; flex-wrap: wrap; }
  .ramp { position: relative; width: 14rem; height: 0.7rem; border: 1px solid #ccc; }
  .ramp i { position: absolute; top: -2px; bottom: -2px; width: 0; border-left: 2px solid #0b1f33; }
  .key { display: inline-flex; align-items: center; gap: 0.3rem; margin-left: 0.6rem; }
  .note { color: #666; }
  figure { margin: 0 0 1rem; }
  figcaption { display: flex; justify-content: space-between; align-items: center; font-weight: 600; font-size: 0.9rem; margin-bottom: 0.2rem; }
  .exp { display: flex; gap: 4px; font-weight: 400; }
  .exp button { padding: 0.05rem 0.5rem; font-size: 0.8rem; }
  .cv { position: relative; }
  canvas { width: 100%; height: auto; display: block; border: 1px solid #ccc; }
  .tip { position: absolute; pointer-events: none; background: #fff; border: 1px solid #999; padding: 2px 6px; font-size: 0.8rem; white-space: nowrap; }
</style>
