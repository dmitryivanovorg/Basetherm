<script lang="ts">
  import type { EpwSummary } from '../api/types';
  import type { SolverClient } from '../api/solver';

  let { client, epw = $bindable(), epwName = $bindable('') }: { client: SolverClient; epw: string | null; epwName?: string } = $props();
  let info = $state<EpwSummary | null>(null);
  let error = $state('');

  // The file is read and parsed in the browser; it is never uploaded.
  async function pick(e: Event) {
    const file = (e.currentTarget as HTMLInputElement).files?.[0];
    if (!file) return;
    error = '';
    info = null;
    epw = null;
    const text = await file.text();
    try {
      info = await client.epwSummary(text);
      parsed = text;
      epwName = file.name;
      epw = text;
    } catch (err) {
      error = err instanceof Error ? err.message : String(err);
    }
  }

  // A climate set from outside (an opened case) is summarised here too.
  let parsed: string | null = null;
  $effect(() => {
    const text = epw;
    if (text === null) {
      parsed = null;
      info = null;
    } else if (text !== parsed) {
      parsed = text;
      client.epwSummary(text).then(
        (i) => (info = i),
        (err) => (error = err instanceof Error ? err.message : String(err)),
      );
    }
  });
</script>

<fieldset>
  <legend>Climate (EPW)</legend>
  <input type="file" accept=".epw" onchange={pick} />
  <a class="find" href="https://www.ladybug.tools/epwmap/" target="_blank" rel="noopener noreferrer">Find an EPW file on ladybug.tools/epwmap</a>
  {#if epw && epwName}<p class="file">{epwName}</p>{/if}
  {#if info}
    <p>{info.city}, {info.country}: mean air temperature {info.meanAirTemperature.toFixed(1)} °C</p>
  {/if}
  {#if error}<p class="err">{error}</p>{/if}
</fieldset>

<style>
  .find { margin-left: 0.6rem; font-size: 0.85rem; white-space: nowrap; }
  .err { color: #111; font-weight: 600; }
  .file { margin: 0.3rem 0 0; color: var(--muted); font-size: 0.85rem; }
</style>
