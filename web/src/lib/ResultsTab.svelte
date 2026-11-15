<script lang="ts">
  // Peak heat loss and gain per room and for the whole basement; click a row (or a room on the plan)
  // for its annual heat flow graph. Loss is negative, gain positive.
  import type { Peak, RunOutput } from '../api/types';
  import { csv, download } from './export';
  import ConvergenceChart from './ConvergenceChart.svelte';
  import RoomChart from './RoomChart.svelte';

  let { out, rooms, target, selected = $bindable(-1) }: {
    out: RunOutput;
    /** Convergence target of the run, as a fraction. */
    target: number;
    /** Level and name of each room, in the order of the results. */
    rooms: { level: string; name: string }[];
    /** Index into `rooms`; -1 = the whole basement. */
    selected: number;
  } = $props();

  const s = $derived(out.summary);
  const watts = (p: Peak | null) => (p ? `${p.watts.toFixed(0)} W` : '–');
  const when = (p: Peak | null) => (p ? p.label.replace(/, hour.*/, '') : '');
  function saveTable() {
    const row = (name: string, loss: Peak | null, gain: Peak | null, kwh: number) => [
      name, loss?.watts ?? '', loss?.label ?? '', gain?.watts ?? '', gain?.label ?? '', kwh,
    ];
    download('peak-loads.csv', csv([
      ['room', 'peak_loss_W', 'peak_loss_when', 'peak_gain_W', 'peak_gain_when', 'annual_kWh'],
      ...s.rooms.map((r, i) => row(`${rooms[i]?.level ? `${rooms[i]!.level}: ` : ''}${r.name}`, r.peaks.loss, r.peaks.gain, r.totalKwh)),
      row('Basement total', s.basementPeaks.loss, s.basementPeaks.gain, s.totalKwh),
    ]), 'text/csv');
  }
  const title = $derived(selected < 0 ? 'Basement total' : `${rooms[selected]?.level}: ${rooms[selected]?.name}`);
</script>

<p class="status">
  {s.symmetry}. {s.converged ? 'Converged' : 'NOT converged'}: the peak hourly heat loss changed
  {s.drift !== null ? `${(100 * s.drift).toFixed(2)}%` : 'by an unknown amount (one hourly year only)'} in the last hourly year
  (target {+(100 * target).toPrecision(3)}%), after {s.coarseYears} warm-up + {s.hourlyYears} hourly years.
</p>
<table>
  <thead>
    <tr><th>Room</th><th>Peak loss</th><th>Peak gain</th><th>Annual, kWh</th></tr>
  </thead>
  <tbody>
    {#each s.rooms as r, i}
      <tr class:sel={i === selected} onclick={() => (selected = i)}>
        <td>{rooms[i]?.level ? `${rooms[i]!.level}: ` : ''}{r.name}</td>
        <td title={when(r.peaks.loss)}>{watts(r.peaks.loss)}</td>
        <td title={when(r.peaks.gain)}>{watts(r.peaks.gain)}</td>
        <td>{r.totalKwh.toFixed(0)}</td>
      </tr>
    {/each}
    <tr class="total" class:sel={selected < 0} onclick={() => (selected = -1)}>
      <td>Basement total</td>
      <td title={when(s.basementPeaks.loss)}>{watts(s.basementPeaks.loss)}</td>
      <td title={when(s.basementPeaks.gain)}>{watts(s.basementPeaks.gain)}</td>
      <td>{s.totalKwh.toFixed(0)}</td>
    </tr>
  </tbody>
</table>
<p class="hint">Peak loss is the most negative hourly flow and peak gain the most positive; hover a value for its date. <button type="button" onclick={saveTable}>Export table (CSV)</button></p>

<RoomChart hourly={out.hourly} rooms={s.rooms.length} room={selected} {title} />

<ConvergenceChart history={s.history} {target} />

<style>
  .status { font-size: 0.85rem; color: #444; margin-top: 0; }
  .hint { font-size: 0.8rem; color: #6b6b6b; }
  table { border-collapse: collapse; width: 100%; margin-bottom: 0.4rem; font-size: 0.9rem; }
  th, td { padding: 0.25rem 0.5rem; text-align: right; border-bottom: 1px solid #ddd; }
  th:first-child, td:first-child { text-align: left; }
  tbody tr { cursor: pointer; }
  tbody tr:hover { background: #ececee; }
  tr.sel { background: #dcdcdf; }
  .total { font-weight: 600; }
</style>
