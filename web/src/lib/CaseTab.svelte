<script lang="ts">
  // The model configuration: climate, soil layers, construction and insulation, domain, run settings.
  import type { SolverClient } from '../api/solver';
  import type { Project } from '../api/types';
  import ClimatePicker from './ClimatePicker.svelte';
  import type { FormState } from './form';
  import { totalDepth } from './form';
  import MeshPreview from './MeshPreview.svelte';
  import SoilTable from './SoilTable.svelte';
  import NumberField from './NumberField.svelte';

  let { form = $bindable(), client, epw = $bindable(), epwName = $bindable(''), project, meshBlocked = $bindable(false) }: {
    form: FormState;
    client: SolverClient;
    epw: string | null;
    epwName?: string;
    project: Project | null;
    meshBlocked?: boolean;
  } = $props();

  const depth = $derived(totalDepth(form));
</script>

<ClimatePicker {client} bind:epw bind:epwName />

<SoilTable bind:form />

<fieldset>
  <legend>Construction</legend>
  <label class="inline"><input type="checkbox" bind:checked={form.slab} /> Slab on grade (no basement walls, floor at grade)</label>
  <div class="grid">
    {#if !form.slab}
      <NumberField label="Wall thickness" bind:value={form.wall.mm} unit="mm" />
    {/if}
    <NumberField label="Floor thickness" bind:value={form.floor.mm} unit="mm" />
  </div>
  <details>
    <summary>Material properties and film coefficients</summary>
    <div class="grid">
      {#if !form.slab}
        <NumberField label="Wall conductivity" bind:value={form.wall.k} unit="W/(m·K)" />
        <NumberField label="Wall heat capacity" bind:value={form.wall.cv} unit="J/(m³·K)" />
        <NumberField label="Wall film coefficient" bind:value={form.hWall} unit="W/(m²·K)" />
      {/if}
      <NumberField label="Floor conductivity" bind:value={form.floor.k} unit="W/(m·K)" />
      <NumberField label="Floor heat capacity" bind:value={form.floor.cv} unit="J/(m³·K)" />
      <NumberField label="Floor film coefficient" bind:value={form.hFloor} unit="W/(m²·K)" />
    </div>
  </details>
</fieldset>

<fieldset>
  <legend>Insulation</legend>
  {#if !form.slab}
    <h4>Walls</h4>
    <label class="inline"><input type="checkbox" bind:checked={form.wallInsulation.enabled} /> Insulate the walls</label>
    {#if form.wallInsulation.enabled}
      <div class="grid">
        <label for="wi-pos">Position</label>
        <select id="wi-pos" bind:value={form.wallInsulation.position}>
          <option value="outside">Outside (ground side)</option>
          <option value="inside">Inside</option>
        </select><span></span>
        <NumberField label="Thickness" bind:value={form.wallInsulation.mm} unit="mm" />
        <NumberField label="Conductivity" bind:value={form.wallInsulation.k} unit="W/(m·K)" />
        <NumberField label="Heat capacity" bind:value={form.wallInsulation.cv} unit="J/(m³·K)" />
      </div>
      <label class="inline"><input type="checkbox" bind:checked={form.wallInsulation.full} /> Over the full wall depth ({depth.toFixed(2)} m)</label>
      {#if !form.wallInsulation.full}
        <div class="grid"><NumberField label="Insulated depth below grade" bind:value={form.wallInsulation.depth} unit="m" step="0.1" /></div>
        {#if form.wallInsulation.position === 'inside'}
          <p class="hint">Below that depth the wall is bare.</p>
        {/if}
      {/if}
    {/if}
  {/if}

  <h4>Floor</h4>
  <label class="inline"><input type="checkbox" bind:checked={form.floorInsulation.enabled} /> Insulate the floor</label>
  {#if form.floorInsulation.enabled}
    <div class="grid">
      <label for="fi-pos">Position</label>
      <select id="fi-pos" bind:value={form.floorInsulation.position}>
        <option value="below">Under the slab (ground side)</option>
        <option value="above">On top of the slab (inside)</option>
      </select><span></span>
      <NumberField label="Thickness" bind:value={form.floorInsulation.mm} unit="mm" />
      <NumberField label="Conductivity" bind:value={form.floorInsulation.k} unit="W/(m·K)" />
      <NumberField label="Heat capacity" bind:value={form.floorInsulation.cv} unit="J/(m³·K)" />
      <label for="fi-cov">Covers</label>
      <select id="fi-cov" bind:value={form.floorInsulation.coverage}>
        <option value="all">The whole floor</option>
        <option value="perimeter">A strip along the walls</option>
      </select><span></span>
      {#if form.floorInsulation.coverage !== 'all'}
        <NumberField label="Strip width" bind:value={form.floorInsulation.width} unit="m" step="0.1" />
      {/if}
    </div>
    {#if form.slab && form.floorInsulation.position === 'above' && form.floorInsulation.coverage !== 'all'}
      <p class="hint">On a slab on grade, where the insulation is missing the slab is modelled flush with the insulation (no step).</p>
    {/if}
  {/if}

  <h4>Horizontal (around the building)</h4>
  <label class="inline"><input type="checkbox" bind:checked={form.groundInsulation.enabled} /> Insulation strip in the ground</label>
  {#if form.groundInsulation.enabled}
    <div class="grid">
      <NumberField label="Width from the wall" bind:value={form.groundInsulation.width} unit="m" step="0.1" />
      <NumberField label="Depth of its top (0 = at surface)" bind:value={form.groundInsulation.depth} unit="m" step="0.1" />
      <NumberField label="Thickness" bind:value={form.groundInsulation.mm} unit="mm" />
      <NumberField label="Conductivity" bind:value={form.groundInsulation.k} unit="W/(m·K)" />
      <NumberField label="Heat capacity" bind:value={form.groundInsulation.cv} unit="J/(m³·K)" />
    </div>
  {/if}
</fieldset>

<fieldset>
  <legend>Ground surface and domain</legend>
  <div class="grid">
    <NumberField label="Albedo" bind:value={form.albedo} step="0.01" />
    <NumberField label="Emissivity" bind:value={form.emissivity} step="0.01" />
    <NumberField label="Margin beyond walls" bind:value={form.margin} unit="m" />
    <NumberField label="Soil depth below floor" bind:value={form.depthBelowFloor} unit="m" />
  </div>
</fieldset>

<fieldset>
  <legend>Run</legend>
  <div class="grid">
    <label for="preset">Mesh preset</label>
    <select id="preset" bind:value={form.preset}>
      <option value="draft">Draft</option>
      <option value="standard">Standard</option>
      <option value="fine">Fine</option>
    </select>
    <span></span>
    <NumberField label="Convergence target" bind:value={form.convergence} unit="%" step="0.1" />
    <NumberField label="Max hourly years" bind:value={form.hourlyYears} step="1" />
    {#if form.warmUp}
      <NumberField label="Max warm-up years" bind:value={form.coarseYears} step="1" />
    {/if}
  </div>
  <p class="hint">
    The run stops when the peak hourly heat loss of the whole basement changes by less than the target between two
    hourly years. The daily warm-up years are judged on the annual heat flow.
  </p>
  <label class="inline"><input type="checkbox" bind:checked={form.periodicInit} /> Start from the periodic initial state</label>
  {#if !form.periodicInit}
    <p class="hint">
      Every column starts at the undisturbed ground profile instead, so the warm-up and the hourly years need more
      years to settle the heat profile under the building.
    </p>
  {/if}
  <label class="inline"><input type="checkbox" bind:checked={form.warmUp} /> Warm up with daily steps first</label>
  {#if !form.warmUp}
    <p class="hint">
      Without the warm-up the hourly years start straight from the periodic initial state and need more years to
      settle: raise “Max hourly years” (for example to 5 or more).
    </p>
  {/if}
  <label class="inline">
    <input type="checkbox" bind:checked={form.conditionEdges} /> Condition the domain edges to the undisturbed ground
  </label>
  {#if !form.conditionEdges}
    <p class="hint">
      The sides and bottom of the domain are adiabatic instead. Either case, make the margin and the soil depth large enough that
      the underground thermal profile is not affected.
    </p>
  {/if}
</fieldset>

{#if project}
  <MeshPreview {project} bind:blocked={meshBlocked} />
{/if}

<style>
  fieldset { margin: 0 0 1rem; }
  h4 { margin: 0.6rem 0 0.3rem; font-size: 0.85rem; }
  .grid { display: grid; grid-template-columns: 1fr 6.5rem 4.5rem; gap: 0.4rem 0.5rem; align-items: center; margin: 0.3rem 0; }
  .inline { display: block; margin: 0.3rem 0; }
  summary { cursor: pointer; font-size: 0.85rem; color: #444; margin-top: 0.4rem; }
  .hint { color: #6b6b6b; font-size: 0.85em; margin: 0.3rem 0 0; }
</style>
