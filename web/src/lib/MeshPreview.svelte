<script lang="ts">
  // Live mesh size and layout for the current form, from the solver's own mesh builder (no solve).
  import { MeshClient, Superseded } from '../api/mesh';
  import type { MeshInfo, Project } from '../api/types';
  import MeshDrawing from './MeshDrawing.svelte';

  let { project, blocked = $bindable(false) }: { project: Project; blocked?: boolean } = $props();

  const client = new MeshClient();
  let mesh = $state<MeshInfo | null>(null);
  let error = $state('');
  let level = $state(0);

  // Debounced: the project changes on every keystroke.
  $effect(() => {
    const p = project;
    const timer = setTimeout(async () => {
      try {
        mesh = await client.info($state.snapshot(p) as Project);
        error = '';
      } catch (e) {
        if (e instanceof Superseded) return;
        mesh = null;
        error = e instanceof Error ? e.message : String(e);
      }
    }, 250);
    return () => clearTimeout(timer);
  });

  $effect(() => {
    blocked = !!error || !!mesh?.overCap;
  });

  const fmt = (n: number) => n.toLocaleString('en-US').replace(/,/g, ' ');
  const cm = (m: number) => `${(100 * m).toFixed(m < 0.1 ? 1 : 0)} cm`;
  const shown = $derived(Math.min(level, Math.max(0, project.building.levels.length - 1)));
</script>

<fieldset>
  <legend>Mesh preview</legend>
  {#if error}
    <p class="err">{error}</p>
  {:else if !mesh}
    <p>Meshing…</p>
  {/if}
  {#if mesh}
    <p class:warn={mesh.warn}>
      <strong>{fmt(mesh.cells)} cells</strong> ({mesh.dims.join(' × ')})
      {#if mesh.symmetryX || mesh.symmetryY}
        after mirror symmetry in {[mesh.symmetryX && 'x', mesh.symmetryY && 'y'].filter(Boolean).join(' and ')}
      {/if}.
      Finest cells x {cm(mesh.hMin[0])}, y {cm(mesh.hMin[1])}, z {cm(mesh.hMin[2])}; largest
      {cm(mesh.hMax[0])}, {cm(mesh.hMax[1])}, {cm(mesh.hMax[2])}.
      {#if mesh.overCap}Over the 4 million cell cap: use a coarser preset or a smaller domain.
      {:else if mesh.warn}Over 2 million cells: the run will be slow and memory hungry.{/if}
    </p>
    {#each mesh.notes as n}<p class="note">{n}</p>{/each}
    {#if project.building.levels.length > 1}
      <label>
        Level in plan view
        <select bind:value={level}>
          {#each project.building.levels as l, i}<option value={i}>{l.name}</option>{/each}
        </select>
      </label>
    {/if}
    <MeshDrawing {mesh} building={project.building} level={shown} />
  {/if}
</fieldset>

<style>
  fieldset { margin-bottom: 1rem; }
  .err { color: #111; font-weight: 600; }
  .warn { color: #444; font-weight: 600; }
  .note { color: #6b6b6b; font-size: 0.85em; margin: 0.2rem 0; }
</style>
