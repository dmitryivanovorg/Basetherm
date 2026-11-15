<script lang="ts">
  // Plan view with pan/zoom. Tools: select a room, draw the outline with straight lines, split a room
  // with a cut line. y is up. Snaps to the grid and to existing corners. Lines are always horizontal or vertical.
  import { bounds, clean, contains, labelPoint, outlineProblem, area, type Pt } from './geometry';

  export interface PlanRoom {
    id: string;
    name: string;
    poly: Pt[];
    fill?: string;
    /** Second label line. */
    sub?: string;
  }
  export type Tool = 'select' | 'outline' | 'split';

  let { footprint, rooms, tool, grid, selectedId = null, readonly = false, onoutline, onsplit, onselect, onerror }: {
    footprint: Pt[];
    rooms: PlanRoom[];
    tool: Tool;
    /** Snap spacing, m. */
    grid: number;
    selectedId?: string | null;
    readonly?: boolean;
    onoutline?: (p: Pt[]) => void;
    onsplit?: (a: Pt, b: Pt) => void;
    onselect?: (id: string | null) => void;
    onerror?: (msg: string) => void;
  } = $props();

  let w = $state(600), h = $state(400);
  let view = $state({ cx: 6, cy: 4, k: 40 }); // world centre and pixels per metre
  let cursor = $state<Pt | null>(null);
  let drawing = $state<Pt[]>([]);
  let splitFrom = $state<Pt | null>(null);

  const sx = (x: number) => w / 2 + (x - view.cx) * view.k;
  const sy = (y: number) => h / 2 - (y - view.cy) * view.k;
  const wx = (px: number) => view.cx + (px - w / 2) / view.k;
  const wy = (py: number) => view.cy - (py - h / 2) / view.k;
  const path = (p: Pt[]) => p.map((q, i) => `${i ? 'L' : 'M'}${sx(q.x).toFixed(1)} ${sy(q.y).toFixed(1)}`).join('') + 'Z';

  function fit() {
    const b = footprint.length ? bounds(footprint) : { x0: 0, x1: 14, y0: 0, y1: 9 };
    const bw = Math.max(b.x1 - b.x0, 1), bh = Math.max(b.y1 - b.y0, 1);
    view = { cx: (b.x0 + b.x1) / 2, cy: (b.y0 + b.y1) / 2, k: Math.min(w / (bw * 1.3), h / (bh * 1.3)) };
  }

  // Fit when the canvas gets its size and when a shape appears or goes away.
  let fittedFor = $state<string | null>(null);
  $effect(() => {
    const key = `${footprint.length > 0}`;
    if (w > 50 && h > 50 && fittedFor !== key) {
      fittedFor = key;
      fit();
    }
  });

  // Leaving a tool drops its half-finished work.
  $effect(() => {
    void tool;
    drawing = [];
    splitFrom = null;
  });

  const corners = $derived([...footprint, ...rooms.flatMap((r) => r.poly), ...drawing]);

  const aligned = (a: Pt, b: Pt) => Math.abs(a.x - b.x) < 1e-6 || Math.abs(a.y - b.y) < 1e-6;

  /** The snapped point; after `anchor` it lies on the horizontal or vertical line through it. */
  function snap(px: number, py: number, anchor: Pt | null): Pt {
    const raw = { x: wx(px), y: wy(py) };
    let near: Pt | null = null, best = 10;
    for (const c of corners) {
      if (anchor && !aligned(anchor, c)) continue;
      const d = Math.hypot(sx(c.x) - px, sy(c.y) - py);
      if (d < best) [best, near] = [d, c];
    }
    if (near) return near;
    const g = (v: number) => Math.round(v / grid) * grid;
    let p = { x: g(raw.x), y: g(raw.y) };
    if (anchor) {
      p = Math.abs(raw.x - anchor.x) >= Math.abs(raw.y - anchor.y) ? { x: p.x, y: anchor.y } : { x: anchor.x, y: p.y };
    }
    return { x: Math.round(p.x * 1e6) / 1e6, y: Math.round(p.y * 1e6) / 1e6 };
  }

  const anchor = $derived(tool === 'outline' ? (drawing.at(-1) ?? null) : tool === 'split' ? splitFrom : null);

  let down: { x: number; y: number; button: number; panned: boolean } | null = null;

  function local(e: PointerEvent) {
    const r = (e.currentTarget as SVGElement).getBoundingClientRect();
    return { px: e.clientX - r.left, py: e.clientY - r.top };
  }

  function onpointerdown(e: PointerEvent) {
    const { px, py } = local(e);
    down = { x: px, y: py, button: e.button, panned: false };
    (e.currentTarget as SVGElement).setPointerCapture(e.pointerId);
  }

  function onpointermove(e: PointerEvent) {
    const { px, py } = local(e);
    cursor = snap(px, py, anchor);
    if (!down) return;
    if (!down.panned && (down.button !== 0 || Math.hypot(px - down.x, py - down.y) > 4)) down.panned = true;
    if (down.panned) {
      view.cx -= (px - down.x) / view.k;
      view.cy += (py - down.y) / view.k;
      down.x = px;
      down.y = py;
    }
  }

  function onpointerup(e: PointerEvent) {
    const d = down;
    down = null;
    if (!d || d.panned || d.button !== 0) return;
    const { px, py } = local(e);
    click(px, py);
  }

  function onwheel(e: WheelEvent) {
    e.preventDefault();
    const { px, py } = local(e as unknown as PointerEvent);
    const [x, y] = [wx(px), wy(py)];
    const k = Math.min(400, Math.max(2, view.k * Math.exp(-e.deltaY * 0.0015)));
    view = { k, cx: x - (px - w / 2) / k, cy: y + (py - h / 2) / k };
  }

  function click(px: number, py: number) {
    if (readonly || tool === 'select') {
      const p = { x: wx(px), y: wy(py) };
      onselect?.([...rooms].reverse().find((r) => contains(r.poly, p))?.id ?? null);
    } else if (tool === 'outline') {
      const first = drawing[0];
      if (first && drawing.length >= 3 && Math.hypot(sx(first.x) - px, sy(first.y) - py) < 10) return finish();
      const p = snap(px, py, drawing.at(-1) ?? null);
      const last = drawing.at(-1);
      if (!last || last.x !== p.x || last.y !== p.y) drawing = [...drawing, p];
    } else {
      const p = snap(px, py, splitFrom);
      if (!splitFrom) splitFrom = p;
      else if (p.x !== splitFrom.x || p.y !== splitFrom.y) {
        onsplit?.(splitFrom, p);
        splitFrom = null;
      }
    }
  }

  function finish() {
    const poly = clean(drawing);
    const [a, b] = [poly.at(-1), poly[0]];
    if (a && b && !aligned(a, b)) return onerror?.('The closing line must be horizontal or vertical: place a corner in line with the first one.');
    if (poly.some((q, i) => !aligned(q, poly[(i + 1) % poly.length]!))) return onerror?.('All lines must be horizontal or vertical.');
    const problem = outlineProblem(poly);
    if (problem) return onerror?.(problem);
    drawing = [];
    onoutline?.(poly);
  }

  function onkeydown(e: KeyboardEvent) {
    const t = e.target as HTMLElement | null;
    if (t && ['INPUT', 'SELECT', 'TEXTAREA'].includes(t.tagName)) return;
    if (e.key === 'Escape') {
      if (drawing.length) drawing = [];
      else splitFrom = null;
    } else if (e.key === 'Enter' && tool === 'outline' && drawing.length >= 3) finish();
    else if (e.key === 'Backspace' && tool === 'outline') drawing = drawing.slice(0, -1);
  }

  const niceStep = $derived.by(() => {
    const raw = 50 / view.k;
    const p = 10 ** Math.floor(Math.log10(raw));
    return [1, 2, 5, 10].map((m) => m * p).find((s) => s >= raw) ?? raw;
  });
  const gridLines = $derived.by(() => {
    const lines: { v: number; vertical: boolean }[] = [];
    const [x0, x1, y0, y1] = [wx(0), wx(w), wy(h), wy(0)];
    for (let v = Math.ceil(x0 / niceStep) * niceStep; v <= x1; v += niceStep) lines.push({ v, vertical: true });
    for (let v = Math.ceil(y0 / niceStep) * niceStep; v <= y1; v += niceStep) lines.push({ v, vertical: false });
    return lines;
  });
  const len = (a: Pt, b: Pt) => Math.hypot(b.x - a.x, b.y - a.y);
  const edges = (p: Pt[], close: boolean) =>
    p.flatMap((a, i) => (i + 1 < p.length || close ? [[a, p[(i + 1) % p.length]!] as const] : []));
  const scaleLen = $derived(niceStep * (niceStep * view.k < 40 ? 5 : 1));
  const cutLine = $derived.by(() => {
    if (!splitFrom || !cursor) return null;
    const dx = cursor.x - splitFrom.x, dy = cursor.y - splitFrom.y, l = Math.hypot(dx, dy);
    if (l < 1e-9) return null;
    const far = 1e3 / l;
    return { a: { x: splitFrom.x - dx * far, y: splitFrom.y - dy * far }, b: { x: splitFrom.x + dx * far, y: splitFrom.y + dy * far } };
  });
  const hint = $derived(
    readonly || tool === 'select'
      ? rooms.length ? 'Click a room. Drag to pan, wheel to zoom.' : ''
      : tool === 'outline'
        ? drawing.length === 0 ? 'Click to place the first corner.'
          : drawing.length < 3 ? 'Click the next corner.'
          : 'Click the next corner; click the first corner or press Enter to close. Backspace undoes a corner.'
        : splitFrom ? 'Click the second point of the cut line. Esc cancels.' : 'Click two points: the line through them cuts the room.',
  );
</script>

<svelte:window {onkeydown} />

<div class="wrap" bind:clientWidth={w} bind:clientHeight={h}>
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <svg
    width={w}
    height={h}
    class:draw={!readonly && tool !== 'select'}
    {onpointerdown}
    {onpointermove}
    {onpointerup}
    onpointerleave={() => (cursor = null)}
    onwheel={onwheel}
    oncontextmenu={(e) => e.preventDefault()}
    ondblclick={() => tool === 'outline' && drawing.length >= 3 && finish()}
  >
    {#each gridLines as l}
      {#if l.vertical}
        <line class="grid" x1={sx(l.v)} x2={sx(l.v)} y1="0" y2={h} />
        <text class="tick" x={sx(l.v) + 2} y={h - 4}>{+l.v.toFixed(3)}</text>
      {:else}
        <line class="grid" x1="0" x2={w} y1={sy(l.v)} y2={sy(l.v)} />
        <text class="tick" x="3" y={sy(l.v) - 2}>{+l.v.toFixed(3)}</text>
      {/if}
    {/each}

    {#if footprint.length}
      <path class="shell" d={path(footprint)} />
    {/if}
    {#each rooms as r (r.id)}
      {@const c = labelPoint(r.poly)}
      <path class="room" class:sel={r.id === selectedId} d={path(r.poly)} style:fill={r.fill} />
      <text class="name" x={sx(c.x)} y={sy(c.y) - (r.sub ? 6 : 0)}>{r.name}</text>
      {#if r.sub}<text class="sub" x={sx(c.x)} y={sy(c.y) + 9}>{r.sub}</text>{/if}
    {/each}
    {#if !readonly}
      {#each edges(footprint, true) as [a, b]}
        <text class="dim" x={sx((a.x + b.x) / 2)} y={sy((a.y + b.y) / 2) - 5}>{len(a, b).toFixed(2)} m</text>
      {/each}
    {/if}

    {#if tool === 'outline' && !readonly}
      {#if drawing.length}
        <path class="draft" d={path(drawing).slice(0, -1)} />
        {#if cursor}<line class="draft" x1={sx(drawing.at(-1)!.x)} y1={sy(drawing.at(-1)!.y)} x2={sx(cursor.x)} y2={sy(cursor.y)} />{/if}
        {#each edges(drawing, false) as [a, b]}
          <text class="dim" x={sx((a.x + b.x) / 2)} y={sy((a.y + b.y) / 2) - 5}>{len(a, b).toFixed(2)} m</text>
        {/each}
        {#if cursor}
          {@const a = drawing.at(-1)!}
          <text class="dim" x={sx((a.x + cursor.x) / 2)} y={sy((a.y + cursor.y) / 2) - 5}>{len(a, cursor).toFixed(2)} m</text>
        {/if}
        {#each drawing as p, i}<circle class="vertex" r={i === 0 ? 6 : 3.5} cx={sx(p.x)} cy={sy(p.y)} />{/each}
      {/if}
    {/if}
    {#if tool === 'split' && !readonly}
      {#if cutLine}<line class="cut" x1={sx(cutLine.a.x)} y1={sy(cutLine.a.y)} x2={sx(cutLine.b.x)} y2={sy(cutLine.b.y)} />{/if}
      {#if splitFrom}<circle class="vertex" r="4" cx={sx(splitFrom.x)} cy={sy(splitFrom.y)} />{/if}
    {/if}
    {#if cursor && !readonly && tool !== 'select'}
      <circle class="snap" r="5" cx={sx(cursor.x)} cy={sy(cursor.y)} />
    {/if}

    <g class="scale">
      <line x1="12" x2={12 + scaleLen * view.k} y1={h - 24} y2={h - 24} />
      <text x="12" y={h - 28}>{+scaleLen.toFixed(3)} m</text>
    </g>
  </svg>
  <div class="status">
    {#if cursor}x {cursor.x.toFixed(2)} m, y {cursor.y.toFixed(2)} m{/if}
  </div>
  {#if hint}<div class="hint">{hint}</div>{/if}
  <button class="fit" type="button" onclick={fit}>Fit</button>
  {#if footprint.length}<div class="area">Footprint {area(footprint).toFixed(1)} m²</div>{/if}
</div>

<style>
  .wrap { position: relative; width: 100%; height: 100%; min-height: 20rem; background: var(--surface); overflow: hidden; }
  svg { display: block; touch-action: none; cursor: default; user-select: none; }
  svg.draw { cursor: crosshair; }
  .grid { stroke: #e8e8e8; stroke-width: 1; }
  .tick { font-size: 10px; fill: #8a8a8a; }
  .shell { fill: none; stroke: #222; stroke-width: 3; stroke-linejoin: round; }
  .room { fill: rgba(0, 0, 0, 0.08); stroke: #666; stroke-width: 1.5; stroke-linejoin: round; }
  .room.sel { stroke: #111; stroke-width: 3; }
  .name, .sub, .dim, .scale text { text-anchor: middle; pointer-events: none; paint-order: stroke; stroke: #fff; stroke-width: 3px; }
  .name { font-size: 13px; font-weight: 600; fill: #222; dominant-baseline: middle; }
  .sub { font-size: 11px; fill: #444; dominant-baseline: middle; }
  .dim { font-size: 11px; fill: #666; }
  .draft { fill: none; stroke: #111; stroke-width: 2; stroke-dasharray: 6 4; }
  .cut { stroke: #111; stroke-width: 2; stroke-dasharray: 6 4; }
  .vertex { fill: #fff; stroke: #111; stroke-width: 2; }
  .snap { fill: none; stroke: #111; stroke-width: 1.5; pointer-events: none; }
  .scale line { stroke: #222; stroke-width: 2; }
  .scale text { text-anchor: start; font-size: 11px; fill: #222; }
  .status, .hint, .area { position: absolute; font-size: 12px; color: #444; background: rgba(255, 255, 255, 0.85); padding: 2px 6px; border-radius: 3px; pointer-events: none; }
  .status { right: 8px; bottom: 8px; }
  .hint { left: 8px; top: 8px; }
  .area { right: 8px; top: 8px; }
  .fit { position: absolute; right: 8px; top: 34px; }
</style>
