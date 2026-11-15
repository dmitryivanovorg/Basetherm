<script lang="ts">
  // Convergence of the run, year by year: the peak hourly heat loss of the whole basement (hourly years
  // only; warm-up years have no hourly flows) and the change from the previous year (peak loss for hourly
  // years, annual flow for warm-up years) against the convergence target. Plain SVG; hover a year for its
  // values. Warm-up years (daily steps) are open markers, hourly years filled.
  import type { YearRecord } from '../api/types';
  import { csv, download } from './export';

  let { history, target }: {
    history: YearRecord[];
    /** Convergence target as a fraction (0.005 = 0.5 %). */
    target: number;
  } = $props();

  const W = 640, H1 = 170, H2 = 170, M = { l: 58, r: 12, t: 10, b: 26 };
  const BLUE = '#1f4e9c', TARGET = '#d9480f';
  const n = $derived(history.length);
  const x = (i: number) => M.l + (n <= 1 ? 0.5 : i / (n - 1)) * (W - M.l - M.r);

  const nice = (lo: number, hi: number) => {
    const span = hi - lo || Math.abs(hi) * 0.1 || 1;
    const p = 10 ** Math.floor(Math.log10(span / 4));
    const step = [1, 2, 5, 10].map((m) => m * p).find((s) => span / s <= 5) ?? p * 10;
    return { lo: Math.floor(lo / step) * step, hi: Math.ceil(hi / step) * step, step };
  };

  // Top panel: peak hourly heat loss, W (hourly years only).
  const peaks = $derived(history.map((y) => y.peak));
  const peakVals = $derived(peaks.filter((p): p is number => p !== null));
  const flow = $derived(peakVals.length ? nice(Math.min(...peakVals), Math.max(...peakVals)) : { lo: -1, hi: 0, step: 1 });
  const y1 = (v: number) => M.t + ((flow.hi - v) / (flow.hi - flow.lo || 1)) * (H1 - M.t - M.b);
  const flowTicks = $derived.by(() => {
    const t: number[] = [];
    for (let v = flow.lo; v <= flow.hi + 1e-9; v += flow.step) t.push(Math.round(v * 1e6) / 1e6);
    return t;
  });

  // Bottom panel: change between years, % on a log axis spanning the data and the target.
  const drifts = $derived(history.map((y) => (y.drift !== null && y.drift > 0 && Number.isFinite(y.drift) ? 100 * y.drift : null)));
  const logRange = $derived.by(() => {
    const vals = [100 * target, ...drifts.filter((d): d is number => d !== null)];
    return { lo: Math.floor(Math.log10(Math.min(...vals)) - 0.3), hi: Math.ceil(Math.log10(Math.max(...vals)) + 0.1) };
  });
  const y2 = (pct: number) => M.t + ((logRange.hi - Math.log10(pct)) / (logRange.hi - logRange.lo || 1)) * (H2 - M.t - M.b);
  const decades = $derived(Array.from({ length: logRange.hi - logRange.lo + 1 }, (_, i) => logRange.lo + i));
  const fmtPct = (e: number) => `${+(10 ** e).toPrecision(2)}%`;

  const line = (pts: [number, number][]) => pts.map(([a, b], i) => `${i ? 'L' : 'M'}${a.toFixed(1)},${b.toFixed(1)}`).join('');
  const flowPath = $derived(line(peaks.flatMap((p, i) => (p === null ? [] : [[x(i), y1(p)] as [number, number]]))));
  const driftPath = $derived(line(drifts.flatMap((d, i) => (d === null ? [] : [[x(i), y2(d)] as [number, number]]))));
  const xTicks = $derived(history.map((_, i) => i).filter((i) => n <= 12 || i % Math.ceil(n / 12) === 0 || i === n - 1));
  const label = (h: YearRecord, i: number) => {
    const same = history.slice(0, i + 1).filter((q) => q.hourly === h.hourly).length;
    return `${h.hourly ? 'Hourly' : 'Warm-up'} year ${same}`;
  };

  let hover = $state<number | null>(null);
  function move(e: PointerEvent) {
    const r = (e.currentTarget as SVGElement).getBoundingClientRect();
    const px = ((e.clientX - r.left) / r.width) * W;
    hover = Math.max(0, Math.min(n - 1, Math.round(((px - M.l) / (W - M.l - M.r)) * (n - 1))));
  }
  const tip = $derived(hover === null ? null : history[hover]!);

  function save() {
    download('convergence.csv', csv([
      ['year', 'stage', 'peak_hourly_heat_loss_W', 'annual_heat_flow_kWh', 'change_from_previous_year_percent'],
      ...history.map((h, i) => [i + 1, h.hourly ? 'hourly' : 'warm-up', h.peak ?? '', h.kwh, h.drift === null ? '' : 100 * h.drift]),
    ]), 'text/csv');
  }
</script>

{#if n > 0}
  <h4>Convergence</h4>
  <div class="legend">
    <span><svg width="22" height="10" aria-hidden="true"><circle cx="11" cy="5" r="4" fill="#fff" stroke={BLUE} stroke-width="1.8" /></svg> Warm-up year (daily steps)</span>
    <span><svg width="22" height="10" aria-hidden="true"><circle cx="11" cy="5" r="4" fill={BLUE} /></svg> Hourly year</span>
    <span><svg width="22" height="10" aria-hidden="true"><line x1="0" x2="22" y1="5" y2="5" stroke={TARGET} stroke-width="1.6" stroke-dasharray="5 3" /></svg> Target {+(100 * target).toPrecision(3)}%</span>
  </div>
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <svg viewBox="0 0 {W} {H1 + H2}" role="img" aria-label="Peak hourly heat loss and change between years, by year" onpointermove={move} onpointerleave={() => (hover = null)}>
    <text class="cap" x={M.l} y="9">Peak hourly heat loss, W (negative = loss; hourly years)</text>
    {#each flowTicks as v}
      <line class="grid" x1={M.l} x2={W - M.r} y1={y1(v)} y2={y1(v)} />
      <text class="tick" x={M.l - 6} y={y1(v) + 3} text-anchor="end">{v}</text>
    {/each}
    <path d={flowPath} fill="none" stroke={BLUE} stroke-width="1.6" />
    {#each history as h, i}
      {#if h.peak !== null}
        <circle cx={x(i)} cy={y1(h.peak)} r="3.6" fill={BLUE} stroke={BLUE} stroke-width="1.8" />
      {/if}
    {/each}

    <g transform="translate(0 {H1})">
      <text class="cap" x={M.l} y="9">Change from the previous year, % (log scale; peak loss, warm-up: annual flow)</text>
      {#each decades as e}
        <line class="grid" x1={M.l} x2={W - M.r} y1={y2(10 ** e)} y2={y2(10 ** e)} />
        <text class="tick" x={M.l - 6} y={y2(10 ** e) + 3} text-anchor="end">{fmtPct(e)}</text>
      {/each}
      <line x1={M.l} x2={W - M.r} y1={y2(100 * target)} y2={y2(100 * target)} stroke={TARGET} stroke-width="1.6" stroke-dasharray="5 3" />
      <path d={driftPath} fill="none" stroke={BLUE} stroke-width="1.6" />
      {#each history as h, i}
        {#if drifts[i] !== null}
          <circle cx={x(i)} cy={y2(drifts[i]!)} r="3.6" fill={h.hourly ? BLUE : '#fff'} stroke={BLUE} stroke-width="1.8" />
        {/if}
      {/each}
      {#each xTicks as i}
        <text class="tick" x={x(i)} y={H2 - 8} text-anchor="middle">{i + 1}</text>
      {/each}
    </g>
    {#if hover !== null}
      <line class="cross" x1={x(hover)} x2={x(hover)} y1={M.t} y2={H1 + H2 - M.b} />
    {/if}
  </svg>
  <p class="x">Year of the run (warm-up years first, then hourly years).</p>
  <p class="tip">
    {#if tip && hover !== null}
      {label(tip, hover)}: {tip.peak !== null ? `peak loss ${tip.peak.toFixed(0)} W` : `annual flow ${tip.kwh.toFixed(0)} kWh (no hourly flows)`}{tip.drift !== null ? `, changed ${(100 * tip.drift).toFixed(3)}% from the year before` : ''}
    {:else}
      Hover the graph for a year's values. <button type="button" onclick={save}>Export (CSV)</button>
    {/if}
  </p>
{/if}

<style>
  h4 { margin: 0.8rem 0 0.2rem; font-size: 0.9rem; }
  svg { width: 100%; height: auto; display: block; }
  .grid { stroke: #e6e8ec; stroke-width: 1; }
  .tick { font-size: 10px; fill: #5b6470; }
  .cap { font-size: 11px; fill: #3b4a5a; }
  .cross { stroke: #8a909a; stroke-width: 1; }
  .legend { display: flex; flex-wrap: wrap; gap: 0.2rem 1rem; font-size: 0.8rem; color: #3b4a5a; }
  .legend span { display: inline-flex; align-items: center; gap: 0.3rem; }
  .x, .tip { font-size: 0.8rem; color: #666; margin: 0.1rem 0; min-height: 1.2em; }
</style>
