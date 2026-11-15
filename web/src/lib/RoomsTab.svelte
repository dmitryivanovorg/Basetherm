<script lang="ts">
  // Levels (height, add, remove) and the properties of the selected room (name, indoor temperature).
  import { addLevel, copyLayout, MONTHS, resetLevel, roomArea, type FormState, type RoomForm } from './form';
  import NumberField from './NumberField.svelte';

  let { form = $bindable(), level = $bindable(), room }: {
    form: FormState;
    level: number;
    room: RoomForm | null;
  } = $props();

  const depths = $derived(
    form.levels.reduce<number[]>((acc, l) => [...acc, (acc.at(-1) ?? 0) + l.height], [0]),
  );
  const current = $derived(form.levels[level]);

  function remove(i: number) {
    form.levels.splice(i, 1);
    level = Math.min(level, form.levels.length - 1);
  }

  function mode(r: RoomForm, m: 'constant' | 'monthly') {
    if (m === 'monthly' && r.mode === 'constant') r.monthly = Array<number>(12).fill(r.temp);
    if (m === 'constant' && r.mode === 'monthly') r.temp = r.monthly.reduce((s, t) => s + t, 0) / 12;
    r.mode = m;
  }
</script>

<section>
  <h3>Levels</h3>
  {#if form.footprint.length === 0}
    <p class="hint">Draw the outline of the building on the plan first.</p>
  {:else if form.slab}
    <p class="hint">Slab on grade: a single floor at ground level. Change this in the Case tab.</p>
  {:else}
    <table>
      <thead><tr><th></th><th>Level</th><th>Height, m</th><th>Depth, m</th><th></th></tr></thead>
      <tbody>
        {#each form.levels as l, i}
          <tr class:active={i === level}>
            <td><input type="radio" name="level" checked={i === level} onchange={() => (level = i)} aria-label="Edit {l.name}" /></td>
            <td><input aria-label="Level name" bind:value={l.name} /></td>
            <td><input aria-label="Height" type="number" min="0.1" step="0.1" bind:value={l.height} /></td>
            <td class="num">{depths[i]!.toFixed(2)}–{depths[i + 1]!.toFixed(2)}</td>
            <td><button type="button" onclick={() => remove(i)} disabled={form.levels.length < 2}>✕</button></td>
          </tr>
        {/each}
      </tbody>
    </table>
    <button type="button" onclick={() => { addLevel(form); level = form.levels.length - 1; }}>Add level below</button>
    <p class="hint">A new level has the same shape as the others, as one room. Split it into rooms on the plan.</p>
  {/if}
  {#if current && form.footprint.length}
    <div class="row">
      <button type="button" onclick={() => resetLevel(form, level)}>Reset {current.name} to one room</button>
      {#if form.levels.length > 1 && !form.slab}
        <select
          aria-label="Copy room layout from"
          onchange={(e) => { copyLayout(form, Number(e.currentTarget.value), level); e.currentTarget.value = ''; }}
        >
          <option value="">Copy layout from…</option>
          {#each form.levels as l, i}{#if i !== level}<option value={i}>{l.name}</option>{/if}{/each}
        </select>
      {/if}
    </div>
  {/if}
</section>

<section>
  <h3>Room</h3>
  {#if !room}
    <p class="hint">Select a room on the plan (Select tool) to set its indoor temperature.</p>
  {:else}
    <div class="grid">
      <label for="room-name">Name</label>
      <input id="room-name" bind:value={room.name} />
      <span class="unit">{roomArea(room).toFixed(1)} m²</span>
    </div>
    <div class="modes" role="radiogroup" aria-label="Indoor temperature">
      <label><input type="radio" checked={room.mode === 'constant'} onchange={() => mode(room, 'constant')} /> Constant</label>
      <label><input type="radio" checked={room.mode === 'monthly'} onchange={() => mode(room, 'monthly')} /> Monthly</label>
    </div>
    {#if room.mode === 'constant'}
      <div class="grid"><NumberField label="Indoor temperature" bind:value={room.temp} unit="°C" step="0.5" /></div>
    {:else}
      <div class="months">
        {#each MONTHS as m, i}
          <label><span>{m}</span><input type="number" step="0.5" bind:value={room.monthly[i]} /></label>
        {/each}
      </div>
      <p class="row">
        <button type="button" onclick={() => (room.monthly = Array<number>(12).fill(room.monthly[0] ?? 20))}>Set all to January</button>
      </p>
    {/if}
  {/if}
</section>

<style>
  section { margin-bottom: 1.2rem; }
  h3 { margin: 0 0 0.4rem; font-size: 0.95rem; }
  .hint { color: #6b6b6b; font-size: 0.85em; margin: 0.3rem 0; }
  table { border-collapse: collapse; width: 100%; margin-bottom: 0.4rem; }
  th { font-weight: 500; font-size: 0.8em; text-align: left; }
  td input:not([type='radio']) { width: 100%; box-sizing: border-box; }
  td.num { font-size: 0.8em; color: #6b6b6b; white-space: nowrap; }
  tr.active { background: #e4e4e7; }
  .grid { display: grid; grid-template-columns: 1fr 8rem 3.5rem; gap: 0.4rem 0.6rem; align-items: center; margin-bottom: 0.4rem; }
  .unit { color: #6b6b6b; font-size: 0.85em; }
  .modes { display: flex; gap: 1rem; margin: 0.4rem 0; }
  .months { display: grid; grid-template-columns: repeat(4, 1fr); gap: 0.3rem; }
  .months label { display: flex; flex-direction: column; font-size: 0.8em; }
  .months input { width: 100%; box-sizing: border-box; }
  .row { display: flex; gap: 0.5rem; flex-wrap: wrap; margin: 0.4rem 0; }
</style>
