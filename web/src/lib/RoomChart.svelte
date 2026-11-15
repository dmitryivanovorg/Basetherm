<script lang="ts">
  // Daily mean heat flow through the floor and walls of one room (or the whole basement) over the
  // reporting year. Loss is negative. Plain SVG; hover shows the day's values.
  import { csv, download, slug, svgToPng } from './export';
  import { MONTHS } from './form';

  let { hourly, rooms, room, title }: {
    hourly: Float64Array;
    rooms: number;
    /** Index of the room, or -1 for the whole basement. */
    room: number;
    title: string;
  } = $props();

  const W = 640, H = 250, M = { l: 58, r: 10, t: 10, b: 26 };
  const MONTH_DAYS = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
  const starts = MONTH_DAYS.reduce<number[]>((a, d) => [...a, a[a.length - 1]! + d], [0]);

  const series = $derived.by(() => {
    const stride = rooms * 2;
    const floor: number[] = [], wall: number[] = [];
    for (let d = 0; d < 365; d++) {
      let f = 0, w = 0;
      for (let h = d * 24; h < d * 24 + 24; h++)
        for (let r = 0; r < rooms; r++) {
          if (room >= 0 && r !== room) continue;
          f += hourly[h * stride + 2 * r] ?? 0;
          w += hourly[h * stride + 2 * r + 1] ?? 0;
        }
      floor.push(f / 24);
      wall.push(w / 24);
    }
    return [
      { name: 'Total', color: '#1f4e9c', width: 2.2, v: floor.map((f, i) => f + wall[i]!) },
      { name: 'Floor', color: '#2a9d8f', width: 1.4, v: floor },
      { name: 'Walls', color: '#d9822b', width: 1.4, v: wall },
    ];
  });

  const nice = (lo: number, hi: number) => {
    const span = hi - lo || 1;
    const p = 10 ** Math.floor(Math.log10(span / 4));
    const step = [1, 2, 5, 10].map((m) => m * p).find((s) => span / s <= 5) ?? p * 10;
    return { lo: Math.floor(lo / step) * step, hi: Math.ceil(hi / step) * step, step };
  };
  const ax = $derived.by(() => {
    const all = series.flatMap((s) => s.v);
    return nice(Math.min(0, ...all), Math.max(0, ...all));
  });
  const x = (d: number) => M.l + (d / 364) * (W - M.l - M.r);
  const y = (v: number) => M.t + ((ax.hi - v) / (ax.hi - ax.lo || 1)) * (H - M.t - M.b);
  const ticks = $derived.by(() => {
    const t: number[] = [];
    for (let v = ax.lo; v <= ax.hi + 1e-9; v += ax.step) t.push(Math.round(v * 1e6) / 1e6);
    return t;
  });
  const path = (v: number[]) => v.map((q, d) => `${d ? 'L' : 'M'}${x(d).toFixed(1)},${y(q).toFixed(1)}`).join('');

  let hover = $state<number | null>(null);
  function move(e: PointerEvent) {
    const r = (e.currentTarget as SVGElement).getBoundingClientRect();
    const px = ((e.clientX - r.left) / r.width) * W;
    hover = Math.max(0, Math.min(364, Math.round(((px - M.l) / (W - M.l - M.r)) * 364)));
  }
  const dateOf = (d: number) => {
    const m = starts.filter((s) => s <= d).length - 1;
    return `${MONTHS[m]} ${d - starts[m]! + 1}`;
  };
  const fmt = (v: number) => `${v.toFixed(0)} W`;

  let svgEl: SVGSVGElement;
  const esc = (t: string) => t.replace(/&/g, '&amp;').replace(/</g, '&lt;');

  /** A standalone SVG of the graph with its title and legend (no hover marks). */
  function standalone() {
    const top = 58;
    const inner = svgEl.cloneNode(true) as SVGSVGElement;
    inner.querySelectorAll('.hov').forEach((n) => n.remove());
    const key = series
      .map((s, i) => `<line x1="${M.l + i * 90}" x2="${M.l + i * 90 + 18}" y1="42" y2="42" stroke="${s.color}" stroke-width="3"/><text x="${M.l + i * 90 + 24}" y="46" font-size="12" fill="#333">${s.name}</text>`)
      .join('');
    const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="${W}" height="${H + top}" viewBox="0 0 ${W} ${H + top}" font-family="system-ui, sans-serif"><rect width="${W}" height="${H + top}" fill="#fff"/><text x="10" y="22" font-size="15" font-weight="700" fill="#111">${esc(title)}: daily mean heat flow, W (loss negative)</text>${key}<g transform="translate(0,${top})">${inner.innerHTML}</g></svg>`;
    return { svg, w: W, h: H + top };
  }
  const name = $derived(`heat-flow-${slug(title)}`);
  const saveSvg = () => download(`${name}.svg`, standalone().svg, 'image/svg+xml');
  async function savePng() {
    const { svg, w, h } = standalone();
    download(`${name}.png`, await svgToPng(svg, w, h, 2));
  }
  function saveCsv() {
    const rows: (string | number)[][] = [['day', 'date', ...series.map((s) => `${s.name}_W`)]];
    for (let d = 0; d < 365; d++) rows.push([d + 1, dateOf(d), ...series.map((s) => Math.round(s.v[d]! * 1000) / 1000)]);
    download(`${name}.csv`, csv(rows), 'text/csv');
  }
</script>

<figure>
  <figcaption>
    <span>{title}: daily mean heat flow, W (loss negative)</span>
    <span class="exp">
      <button type="button" onclick={saveSvg}>SVG</button><button type="button" onclick={savePng}>PNG</button><button type="button" onclick={saveCsv}>CSV</button>
    </span>
  </figcaption>
  <div class="legend">
    {#each series as s}<span><i style:background={s.color}></i>{s.name}</span>{/each}
  </div>
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <svg bind:this={svgEl} viewBox="0 0 {W} {H}" role="img" aria-label="Daily mean heat flow of {title} over the year" onpointermove={move} onpointerleave={() => (hover = null)}>
    {#each ticks as t}
      <line x1={M.l} x2={W - M.r} y1={y(t)} y2={y(t)} stroke={t === 0 ? '#8a909a' : '#e6e8ec'} />
      <text x={M.l - 6} y={y(t) + 4} text-anchor="end" font-size="11" fill="#5b6470">{t}</text>
    {/each}
    {#each MONTHS as m, i}
      <text x={x((starts[i]! + starts[i + 1]!) / 2)} y={H - 8} text-anchor="middle" font-size="11" fill="#5b6470">{m}</text>
    {/each}
    {#each series as s}<path d={path(s.v)} fill="none" stroke={s.color} stroke-width={s.width} stroke-linejoin="round" />{/each}
    {#if hover !== null}
      <g class="hov">
      <line x1={x(hover)} x2={x(hover)} y1={M.t} y2={H - M.b} stroke="#5b6470" stroke-dasharray="3 3" />
      {#each series as s}<circle cx={x(hover)} cy={y(s.v[hover]!)} r="3.5" fill={s.color} stroke="#fff" stroke-width="2" />{/each}
      </g>
    {/if}
  </svg>
  <div class="readout">
    {#if hover !== null}
      {dateOf(hover)}: {series.map((s) => `${s.name} ${fmt(s.v[hover!]!)}`).join(' · ')}
    {:else}&nbsp;{/if}
  </div>
</figure>

<style>
  figure { margin: 0 0 1rem; }
  figcaption { display: flex; justify-content: space-between; align-items: center; gap: 0.5rem; font-weight: 600; font-size: 0.9rem; margin-bottom: 0.2rem; }
  .exp { display: flex; gap: 4px; font-weight: 400; }
  .exp button { padding: 0.05rem 0.5rem; font-size: 0.8rem; }
  svg { width: 100%; height: auto; display: block; }
  .legend { display: flex; gap: 1rem; font-size: 0.8rem; color: #3b4a5a; }
  .legend i { display: inline-block; width: 14px; height: 3px; margin-right: 5px; vertical-align: middle; }
  .readout { font-size: 0.8rem; color: #3b4a5a; min-height: 1.2em; }
</style>
