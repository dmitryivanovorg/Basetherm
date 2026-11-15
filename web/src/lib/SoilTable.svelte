<script lang="ts">
  // Soil layers: editable properties (thawed and frozen); a class and saturation from the soil table give the defaults.
  import { soilClient } from '../api/soil';
  import type { SoilPreset } from '../api/types';
  import type { FormState, SoilForm } from './form';
  import { applyClassDefaults } from './soilDefaults';

  let { form = $bindable() }: { form: FormState } = $props();

  let presets = $state<SoilPreset[]>([]);

  $effect(() => {
    soilClient.presets().then((p) => (presets = p)).catch(() => {});
  });

  // Choosing a class or a saturation loads that class's properties as defaults; every value stays editable.
  function setClass(s: SoilForm, id: string) {
    s.preset = id;
    applyClassDefaults(s).catch(() => {});
  }
  function setSaturation(s: SoilForm, percent: number) {
    s.saturation = percent / 100;
    applyClassDefaults(s).catch(() => {});
  }
  const num = (e: Event & { currentTarget: HTMLInputElement }) => e.currentTarget.valueAsNumber;
  const show = (v: number | undefined, scale = 1) => (v === undefined ? '' : Math.round((v / scale) * 1e6) / 1e6);

  function add() {
    const last = form.soil.at(-1)!;
    form.soil.push({ ...last, top: last.top + 1 });
  }
</script>

<fieldset>
  <legend>Soil layers</legend>
  <label class="inline">
    <input type="checkbox" bind:checked={form.simulateFreezing} /> Simulate the freeze/thaw cycle
  </label>
  <table>
    <thead>
      <tr><th>From, m</th><th>Defaults from</th><th>Saturation, %</th><th></th></tr>
    </thead>
    <tbody>
      {#each form.soil as s, i}
        <tr>
          <td><input type="number" step="any" bind:value={s.top} disabled={i === 0} aria-label="Layer top depth" /></td>
          <td>
            <select value={s.preset} onchange={(e) => setClass(s, e.currentTarget.value)} aria-label="Soil class">
              {#each presets as c}<option value={c.id}>{c.name}</option>{/each}
              <option value="custom">Custom</option>
            </select>
          </td>
          <td>
            {#if s.preset !== 'custom'}
              <input
                type="number" min="0" max="100" step="5" aria-label="Degree of saturation"
                value={Math.round(s.saturation * 1000) / 10}
                oninput={(e) => setSaturation(s, num(e))}
              />
            {/if}
          </td>
          <td><button type="button" onclick={() => form.soil.splice(i, 1)} disabled={i === 0}>✕</button></td>
        </tr>
        <tr class="sub">
          <td></td>
          <td colspan="3">
            <details>
              <summary>Properties</summary>
              <label>k thawed <input type="number" step="any" min="0" value={show(s.k)} oninput={(e) => (s.k = num(e))} aria-label="Thawed conductivity" /> W/(m·K)</label>
              <label class:off={!form.simulateFreezing}>k frozen <input type="number" step="any" min="0" value={show(s.kFrozen)} oninput={(e) => (s.kFrozen = num(e))} disabled={!form.simulateFreezing} aria-label="Frozen conductivity" /> W/(m·K)</label>
              <label>C thawed <input type="number" step="any" min="0" value={show(s.cv, 1e6)} oninput={(e) => (s.cv = num(e) * 1e6)} aria-label="Thawed heat capacity" /> MJ/(m³·K)</label>
              <label class:off={!form.simulateFreezing}>C frozen <input type="number" step="any" min="0" value={show(s.cvFrozen, 1e6)} oninput={(e) => (s.cvFrozen = num(e) * 1e6)} disabled={!form.simulateFreezing} aria-label="Frozen heat capacity" /> MJ/(m³·K)</label>
              <label class:off={!form.simulateFreezing}>Latent heat <input type="number" step="any" min="0" value={show(s.latent, 1e6)} oninput={(e) => (s.latent = num(e) * 1e6)} disabled={!form.simulateFreezing} aria-label="Latent heat of freezing" /> MJ/m³</label>
            </details>
          </td>
        </tr>
      {/each}
    </tbody>
  </table>
  <button type="button" onclick={add}>Add layer below</button>
  <details>
    <summary>Comment on the soil properties</summary>
    <p class="hint">
      Each layer runs from its depth below grade to the next; the last goes to the bottom. The soil class and saturation
      only set default values (Johansen–Kersten model, Johansen 1975; Farouki 1981):
      changing either reloads the defaults and overwrites your edits in that layer. Edit the properties directly where you
      have site data, or choose Custom. With the freeze/thaw cycle off, only the thawed values are used; with it on, the
      soil switches to the frozen values below 0 °C and the latent heat is released over the phase-change band.
    </p>
  </details>
</fieldset>

<style>
  fieldset { margin: 0 0 1rem; }
  table { border-collapse: collapse; width: 100%; margin-bottom: 0.4rem; }
  th { font-weight: 500; font-size: 0.8em; text-align: left; }
  td input[type='number'], td select { width: 100%; box-sizing: border-box; }
  .inline { display: block; margin-bottom: 0.5rem; }
  .off { color: #999; }
  .sub td { padding-bottom: 0.4rem; }
  summary { cursor: pointer; font-size: 0.85em; color: #444; }
  .sub label { display: inline-block; margin-right: 0.6rem; font-size: 0.85em; }
  .sub input { width: 5.5rem; }
  .hint { color: #6b6b6b; font-size: 0.85em; margin: 0.3rem 0 0; }
</style>
