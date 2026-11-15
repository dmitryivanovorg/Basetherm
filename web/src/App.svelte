<script lang="ts">
  import { Cancelled, SolverClient } from './api/solver';
  import type { Progress, RunOutput } from './api/types';
  import AboutDialog from './lib/AboutDialog.svelte';
  import CaseTab from './lib/CaseTab.svelte';
  import { parseCase, serializeCase } from './lib/casefile';
  import { download, slug } from './lib/export';
  import GuideDialog from './lib/GuideDialog.svelte';
  import { blockers, defaults, flatRooms, setFootprint, toProject, totalDepth, uniqueRoomName, newRoom, type FormState } from './lib/form';
  import { fillMissing } from './lib/soilDefaults';
  import { contains, splitPolygon, type Pt } from './lib/geometry';
  import PlanCanvas, { type PlanRoom, type Tool } from './lib/PlanCanvas.svelte';
  import ResultsTab from './lib/ResultsTab.svelte';
  import RoomsTab from './lib/RoomsTab.svelte';
  import FluxView from './lib/FluxView.svelte';
  import SectionView from './lib/SectionView.svelte';

  type Tab = 'rooms' | 'case' | 'results';
  /** The model as it was run, so the results stay consistent while the form is edited. */
  interface Ran {
    key: string;
    footprint: Pt[];
    levels: { name: string; rooms: { id: string; name: string; poly: Pt[] }[] }[];
    rooms: { level: string; name: string; id: string }[];
    lx: number;
    ly: number;
    depth: number;
    wall: number;
    floor: number;
    /** Convergence target of the run, fraction. */
    target: number;
  }

  const client = new SolverClient();
  let form = $state<FormState>(structuredClone(defaults));
  let epw = $state<string | null>(null);
  let epwName = $state('');
  let dialog = $state<'guide' | 'about' | null>(null);
  let fileInput: HTMLInputElement;
  let tab = $state<Tab>('rooms');
  let tool = $state<Tool>('outline');
  let grid = $state(0.1);
  let level = $state(0);
  let selectedId = $state<string | null>(null);
  let notice = $state('');

  let running = $state(false);
  let progress = $state<Progress | null>(null);
  let output = $state<RunOutput | null>(null);
  let ran = $state<Ran | null>(null);
  let error = $state('');
  let meshBlocked = $state(false);
  let resultsView = $state<'plan' | 'sections' | 'flux'>('plan');
  let resultsLevel = $state(0);
  let resultRoom = $state(-1);

  // Width of the side panel in px: drag the divider (or use the arrow keys on it); remembered.
  const MIN_ASIDE = 280;
  const maxAside = () => Math.max(MIN_ASIDE, Math.floor(window.innerWidth * 0.7));
  const clampAside = (w: number) => Math.min(maxAside(), Math.max(MIN_ASIDE, Math.round(w)));
  let asideW = $state(clampAside(Number(localStorage.getItem('basetherm.asideWidth')) || 480));
  let resizing = $state(false);
  $effect(() => localStorage.setItem('basetherm.asideWidth', String(asideW)));
  function dragStart(e: PointerEvent) {
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    resizing = true;
  }
  function dragMove(e: PointerEvent) {
    if (resizing) asideW = clampAside(window.innerWidth - e.clientX);
  }
  function dragKey(e: KeyboardEvent) {
    const step = e.shiftKey ? 80 : 20;
    if (e.key === 'ArrowLeft') asideW = clampAside(asideW + step);
    else if (e.key === 'ArrowRight') asideW = clampAside(asideW - step);
    else if (e.key === 'Home') asideW = MIN_ASIDE;
    else if (e.key === 'End') asideW = maxAside();
    else return;
    e.preventDefault();
  }

  // Soil properties come from the class table (WASM); fill them in whichever tab is open.
  $effect(() => fillMissing(form.soil));
  const project = $derived(toProject(form));
  const problems = $derived(blockers(form));
  const currentLevel = $derived(form.levels[form.slab ? 0 : level]);
  const room = $derived(currentLevel?.rooms.find((r) => r.id === selectedId) ?? null);
  const stale = $derived(!!ran && ran.key !== JSON.stringify(project));

  const label = $derived(
    !progress ? '' :
    progress.stage === 'setup' ? 'Far-field column and periodic initial state…' :
    `${progress.stage === 'warm-up' ? 'Daily-step warm-up' : 'Hourly year'} ${progress.year}: ${(100 * progress.fraction).toFixed(0)}%`,
  );

  // --- editing, with undo for geometry ---
  let history = $state<string[]>([]);
  const snapshot = () => JSON.stringify({ footprint: form.footprint, levels: form.levels, seq: form.seq });
  function commit() {
    history = [...history.slice(-49), snapshot()];
  }
  function undo() {
    const last = history.at(-1);
    if (!last) return;
    Object.assign(form, JSON.parse(last));
    history = history.slice(0, -1);
    level = Math.min(level, form.levels.length - 1);
    if (!currentLevel?.rooms.some((r) => r.id === selectedId)) selectedId = null;
  }
  function onkeydown(e: KeyboardEvent) {
    const t = e.target as HTMLElement | null;
    if (t && ['INPUT', 'SELECT', 'TEXTAREA'].includes(t.tagName)) return;
    if ((e.ctrlKey || e.metaKey) && e.key === 'z') {
      e.preventDefault();
      undo();
    }
  }

  // --- save and load the case set-up ---
  function saveCase() {
    const name = slug(`basetherm ${epwName.replace(/\.epw$/i, '')}`.trim());
    download(`${name}.basetherm.json`, serializeCase($state.snapshot(form) as FormState, epw ? { name: epwName, text: epw } : null), 'application/json');
    notice = '';
  }
  async function openCase(e: Event) {
    const input = e.currentTarget as HTMLInputElement;
    const file = input.files?.[0];
    input.value = '';
    if (!file) return;
    if ((form.footprint.length || output) && !confirm('Opening a case replaces the current set-up and results. Continue?')) return;
    try {
      const loaded = parseCase(await file.text());
      commit();
      Object.assign(form, loaded.form);
      epw = loaded.epw?.text ?? null;
      epwName = loaded.epw?.name ?? '';
      output = null;
      ran = null;
      error = '';
      level = 0;
      resultRoom = -1;
      selectedId = form.levels[0]?.rooms[0]?.id ?? null;
      tool = form.footprint.length ? 'select' : 'outline';
      tab = 'rooms';
      notice = '';
    } catch (err) {
      notice = err instanceof Error ? err.message : String(err);
    }
  }

  function setTool(t: Tool) {
    if (t === 'outline' && form.footprint.length && !confirm('Drawing a new outline removes all rooms on every level. Continue?')) return;
    tool = t;
    notice = '';
  }

  function onoutline(poly: Pt[]) {
    commit();
    setFootprint(form, poly);
    level = 0;
    tool = 'select';
    selectedId = form.levels[0]?.rooms[0]?.id ?? null;
  }

  function onsplit(a: Pt, b: Pt) {
    const l = currentLevel;
    if (!l) return;
    const mid = { x: (a.x + b.x) / 2, y: (a.y + b.y) / 2 };
    const target = [mid, a, b].map((p) => l.rooms.find((r) => contains(r.poly, p))).find(Boolean);
    if (!target) return (notice = 'The cut line must pass through a room.');
    const res = splitPolygon(target.poly, a, b);
    if ('error' in res) return (notice = res.error);
    commit();
    notice = '';
    const added = newRoom(form, res.parts[1], target);
    added.name = uniqueRoomName(l);
    target.poly = res.parts[0];
    l.rooms.splice(l.rooms.indexOf(target) + 1, 0, added);
    selectedId = target.id;
  }

  function onselect(id: string | null) {
    selectedId = id;
    if (id && tab === 'case') tab = 'rooms';
  }

  const tempText = (r: { mode: string; temp: number; monthly: number[] }) =>
    r.mode === 'monthly' ? `${Math.min(...r.monthly)}…${Math.max(...r.monthly)} °C` : `${r.temp} °C`;
  const editRooms = $derived<PlanRoom[]>(
    (currentLevel?.rooms ?? []).map((r) => ({ id: r.id, name: r.name, poly: r.poly, sub: tempText(r) })),
  );

  const resultRooms = $derived.by<PlanRoom[]>(() => {
    if (!ran || !output) return [];
    const lv = ran.levels[resultsLevel];
    const kwh = output.summary.rooms.map((r) => Math.abs(r.totalKwh));
    const max = Math.max(1, ...kwh);
    return (lv?.rooms ?? []).map((r) => {
      const i = ran!.rooms.findIndex((q) => q.id === r.id);
      const v = output!.summary.rooms[i]?.totalKwh ?? 0;
      return {
        id: r.id,
        name: r.name,
        poly: r.poly,
        fill: `rgba(60, 60, 60, ${0.1 + 0.55 * (Math.abs(v) / max)})`,
        sub: `${v.toFixed(0)} kWh`,
      };
    });
  });
  const resultSelectedId = $derived(resultRoom >= 0 ? (ran?.rooms[resultRoom]?.id ?? null) : null);
  function onresultselect(id: string | null) {
    resultRoom = id ? (ran?.rooms.findIndex((r) => r.id === id) ?? -1) : -1;
  }

  async function run() {
    if (!epw || !project) return;
    running = true;
    error = '';
    output = null;
    ran = null;
    progress = null;
    const flat = flatRooms(form);
    const b = project.building;
    const snap: Ran = {
      key: JSON.stringify(project),
      footprint: $state.snapshot(form.footprint),
      levels: (form.slab ? form.levels.slice(0, 1) : form.levels).map((l) => ({
        name: l.name,
        rooms: l.rooms.map((r) => ({ id: r.id, name: r.name, poly: $state.snapshot(r.poly) })),
      })),
      rooms: flat.map((f) => ({ level: f.levelName, name: f.room.name, id: f.room.id })),
      lx: b.lx,
      ly: b.ly,
      depth: totalDepth(form),
      wall: b.wall.reduce((s, l) => s + l.thickness, 0),
      floor: b.floor.reduce((s, l) => s + l.thickness, 0),
      target: form.convergence / 100,
    };
    try {
      output = await client.run($state.snapshot(project) as typeof project, epw, (p) => (progress = p));
      ran = snap;
      resultRoom = -1;
      resultsLevel = 0;
      resultsView = 'sections';
      tab = 'results';
    } catch (e) {
      if (!(e instanceof Cancelled)) error = e instanceof Error ? e.message : String(e);
    } finally {
      running = false;
    }
  }

  const inResults = $derived(tab === 'results' && !!output && !!ran);
  const levelNames = $derived(inResults ? ran!.levels.map((l) => l.name) : (form.slab ? form.levels.slice(0, 1) : form.levels).map((l) => l.name));
  const activeLevel = $derived(inResults ? resultsLevel : form.slab ? 0 : level);
  function pickLevel(i: number) {
    if (inResults) resultsLevel = i;
    else level = i;
  }
  const whyNot = $derived(
    problems[0] ?? (!epw ? 'Choose a climate file in the Case tab.' : meshBlocked ? 'The mesh is invalid or too large (see the Case tab).' : ''),
  );
</script>

<svelte:window {onkeydown} onresize={() => (asideW = clampAside(asideW))} />

<div class="app" class:resizing style:--aside="{asideW}px">
  <header>
    <div class="brand">
      <svg class="logo" viewBox="0 0 32 32" aria-hidden="true">
        <rect width="32" height="32" rx="8" fill="#3a3a3d" fill-opacity="0.14" />
        <!-- Half section through a basement: dashed mirror axis on the left, wall and floor below the ground line, heat flowing out into the soil. -->
        <path d="M3 8h26" stroke="#3a3a3d" stroke-width="1.6" stroke-linecap="round" />
        <path d="M6 8v14" stroke="#3a3a3d" stroke-width="1.4" stroke-dasharray="2 2" opacity="0.7" />
        <path d="M15 8v14H6" stroke="#3a3a3d" stroke-width="2.8" stroke-linecap="round" stroke-linejoin="round" fill="none" />
        <path d="M19 15h8m0 0-2.4-2.4M27 15l-2.4 2.4" stroke="#3a3a3d" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" fill="none" opacity="0.85" />
        <path d="M8 25.5v4m0 0-1.8-1.8M8 29.5l1.8-1.8M13 25.5v4m0 0-1.8-1.8M13 29.5l1.8-1.8" stroke="#3a3a3d" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" fill="none" opacity="0.85" />
      </svg>
      <div>
        <h1>Basetherm <small>v{__APP_VERSION__}</small></h1>
        <span class="sub">Accurate simulation of heat loss from a basement. Runs from your browser. </span>
      </div>
    </div>
    <span class="spacer"></span>
    {#if running}<span class="progress">{label}</span><button class="ghost" onclick={() => client.cancel()}>Cancel</button>{/if}
    {#if !running && whyNot}<span class="why">{whyNot}</span>{/if}
    <nav aria-label="Case and help">
      <button class="ghost" onclick={() => fileInput.click()} disabled={running} title="Open a saved case">Open case</button>
      <button class="ghost" onclick={saveCase} disabled={running} title="Save the case set-up to a file">Save case</button>
      <button class="ghost" onclick={() => (dialog = 'guide')}>Guide</button>
      <button class="ghost" onclick={() => (dialog = 'about')}>About</button>
    </nav>
    <input bind:this={fileInput} type="file" accept=".json,application/json" onchange={openCase} hidden />
    <button class="run" onclick={run} disabled={!project || !epw || running || meshBlocked}>▶ Run</button>
  </header>

  <section class="stage">
    <div class="toolbar">
      {#if inResults}
        <div class="seg" role="group" aria-label="View">
          <button class:on={resultsView === 'plan'} onclick={() => (resultsView = 'plan')}>Plan</button>
          <button class:on={resultsView === 'sections'} onclick={() => (resultsView = 'sections')}>Temperature sections</button>
          <button class:on={resultsView === 'flux'} onclick={() => (resultsView = 'flux')}>Heat flux sections</button>
        </div>
      {:else}
        <div class="seg" role="group" aria-label="Tool">
          <button class:on={tool === 'select'} onclick={() => setTool('select')} disabled={!form.footprint.length}>Select</button>
          <button class:on={tool === 'outline'} onclick={() => setTool('outline')}>Draw outline</button>
          <button class:on={tool === 'split'} onclick={() => setTool('split')} disabled={!form.footprint.length}>Split room</button>
        </div>
        <label>
          Snap
          <select bind:value={grid}>
            {#each [0.05, 0.1, 0.25, 0.5, 1] as g}<option value={g}>{g} m</option>{/each}
          </select>
        </label>
        <button onclick={undo} disabled={!history.length} title="Ctrl+Z">Undo</button>
      {/if}
      {#if levelNames.length > 1 || inResults}
        <div class="seg levels" role="tablist" aria-label="Level">
          {#each levelNames as n, i}
            <button role="tab" aria-selected={i === activeLevel} class:on={i === activeLevel} onclick={() => pickLevel(i)}>{n}</button>
          {/each}
        </div>
      {:else if levelNames.length === 1}
        <span class="lvl">{levelNames[0]}</span>
      {/if}
      {#if notice}<span class="notice">{notice}</span>{/if}
      {#if inResults && stale}<span class="notice">The model changed since this run.</span>{/if}
    </div>
    <div class="canvas">
      {#if inResults && resultsView === 'flux' && output && ran}
        <div class="scroll">
          <FluxView sections={output.sections} lx={ran.lx} ly={ran.ly} depth={ran.depth} wall={ran.wall} floor={ran.floor} />
        </div>
      {:else if inResults && resultsView === 'sections' && output && ran}
        <div class="scroll">
          <SectionView sections={output.sections} lx={ran.lx} ly={ran.ly} depth={ran.depth} wall={ran.wall} floor={ran.floor} />
        </div>
      {:else if inResults && ran}
        <PlanCanvas footprint={ran.footprint} rooms={resultRooms} tool="select" {grid} readonly selectedId={resultSelectedId} onselect={onresultselect} />
      {:else}
        <PlanCanvas footprint={form.footprint} rooms={editRooms} {tool} {grid} {selectedId} {onoutline} {onsplit} {onselect} onerror={(m) => (notice = m)} />
      {/if}
    </div>
  </section>

  <!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
  <div
    class="divider" role="separator" aria-orientation="vertical" aria-label="Resize the side panel"
    aria-valuenow={asideW} aria-valuemin={MIN_ASIDE} aria-valuemax={maxAside()} tabindex="0" title="Drag to resize"
    onpointerdown={dragStart} onpointermove={dragMove} onpointerup={() => (resizing = false)} onpointercancel={() => (resizing = false)}
    onkeydown={dragKey} ondblclick={() => (asideW = clampAside(480))}
  ></div>
  <aside>
    <div class="tabs" role="tablist">
      <button role="tab" aria-selected={tab === 'rooms'} class:on={tab === 'rooms'} onclick={() => (tab = 'rooms')}>Rooms</button>
      <button role="tab" aria-selected={tab === 'case'} class:on={tab === 'case'} onclick={() => (tab = 'case')}>Case</button>
      <button role="tab" aria-selected={tab === 'results'} class:on={tab === 'results'} onclick={() => (tab = 'results')}>Results</button>
    </div>
    <div class="body">
      {#if tab === 'rooms'}
        <RoomsTab bind:form bind:level {room} />
      {:else if tab === 'case'}
        <CaseTab bind:form {client} bind:epw bind:epwName {project} bind:meshBlocked />
      {:else}
        {#if error}<p class="err">{error}</p>{/if}
        {#if running}<p>{label}</p>{/if}
        {#if output && ran}
          <ResultsTab out={output} rooms={ran.rooms} target={ran.target} bind:selected={resultRoom} />
        {:else if !running && !error}
          <p class="hint">Draw the building, set the case, choose a climate file and press Run.</p>
        {/if}
      {/if}
    </div>
  </aside>
  <footer>
    <span>© 2026 <a href="https://dmitryivanov.org" target="_blank" rel="noopener">Dmitry Ivanov</a></span>
    <span class="dot">·</span>
    <a href="https://dmitryivanov.org" target="_blank" rel="noopener">dmitryivanov.org</a>
    <span class="dot">·</span>
    <button class="link" onclick={() => (dialog = 'about')}>Disclaimer</button>
    <span class="spacer"></span>
    <span>Made in Melbourne with <span class="heart" aria-label="love">♥</span></span>
  </footer>
</div>

{#if dialog === 'guide'}<GuideDialog onclose={() => (dialog = null)} />{/if}
{#if dialog === 'about'}<AboutDialog onclose={() => (dialog = null)} />{/if}

<style>
  .app { display: grid; grid-template-columns: minmax(0, 1fr) 6px var(--aside, 30rem); grid-template-rows: auto minmax(0, 1fr) auto; height: 100vh; }
  .app.resizing { user-select: none; cursor: col-resize; }
  .divider { cursor: col-resize; background: var(--line); touch-action: none; }
  .divider:hover, .divider:focus-visible, .resizing .divider { background: #999; outline: none; }
  header { grid-column: 1 / 4; display: flex; align-items: center; gap: 0.9rem; padding: 0.55rem 1.1rem; background: var(--chrome); color: var(--ink); border-bottom: 1px solid var(--line); z-index: 1; }
  .brand { display: flex; align-items: center; gap: 0.7rem; }
  .logo { width: 2.2rem; height: 2.2rem; }
  h1 { font-size: 1.2rem; margin: 0; letter-spacing: 0.01em; }
  h1 small { font-weight: 400; opacity: 0.7; font-size: 0.78rem; }
  .sub { opacity: 0.8; font-size: 0.8rem; }
  .spacer { flex: 1; }
  .why, .progress { font-size: 0.82rem; color: var(--muted); max-width: 28rem; }
  .progress { color: var(--muted); }
  nav { display: flex; gap: 0.3rem; }
  header :global(button.ghost) { background: transparent; color: var(--ink); border-color: #c9c9cd; }
  header :global(button.ghost:hover:not(:disabled)) { background: #e2e2e5; border-color: #a8a8ad; }
  .run { background: var(--brand); color: #fff; border: 1px solid var(--brand); padding: 0.4rem 1.5rem; font-weight: 600; }
  .run:hover:not(:disabled) { background: var(--brand-dark); border-color: var(--brand-dark); }
  .run:disabled { background: #d4d4d8; border-color: transparent; color: #8a8a90; opacity: 1; }
  .stage { display: flex; flex-direction: column; min-width: 0; min-height: 0; }
  .toolbar { display: flex; align-items: center; gap: 0.8rem; padding: 0.45rem 0.8rem; border-bottom: 1px solid var(--line); background: var(--panel); flex-wrap: wrap; font-size: 0.9rem; }
  .seg { display: flex; }
  .seg button { border-radius: 0; margin-left: -1px; }
  .seg button:first-child { border-radius: var(--radius) 0 0 var(--radius); margin-left: 0; }
  .seg button:last-child { border-radius: 0 var(--radius) var(--radius) 0; }
  .seg button.on, .tabs button.on { background: var(--brand); color: #fff; border-color: var(--brand); }
  .lvl { font-weight: 600; }
  .notice { color: #111; font-weight: 600; font-size: 0.85rem; }
  .canvas { flex: 1; min-height: 0; position: relative; background: var(--surface); }
  .scroll { position: absolute; inset: 0; overflow: auto; padding: 0.8rem 1rem; }
  aside { display: flex; min-width: 0; flex-direction: column; min-height: 0; background: var(--surface); }
  .tabs { display: flex; border-bottom: 1px solid var(--line); background: var(--panel); padding: 0.4rem 0.4rem 0; gap: 0.25rem; }
  .tabs button { flex: 1; border: 1px solid transparent; border-bottom: 0; border-radius: var(--radius) var(--radius) 0 0; padding: 0.5rem; background: transparent; font-weight: 600; color: var(--muted); }
  .tabs button:hover:not(.on) { background: #e4e4e7; }
  .tabs button.on { color: #fff; }
  .body { overflow: auto; padding: 0.9rem; flex: 1; }
  .err { color: #111; font-weight: 600; }
  .hint { color: var(--muted); }
  footer { grid-column: 1 / 4; display: flex; align-items: center; gap: 0.5rem; padding: 0.3rem 1.1rem; border-top: 1px solid var(--line); background: var(--panel); font-size: 0.78rem; color: var(--muted); }
  footer a { color: var(--muted); }
  footer a:hover, footer .link:hover { color: var(--brand); }
  footer .link { border: 0; background: none; padding: 0; color: var(--muted); text-decoration: underline; font-size: inherit; }
  .dot { opacity: 0.5; }
  .heart { color: #000; }
</style>
