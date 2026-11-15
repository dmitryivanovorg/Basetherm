<script lang="ts">
  import type { Snippet } from 'svelte';

  let { title, onclose, children }: { title: string; onclose: () => void; children: Snippet } = $props();
  let dlg: HTMLDialogElement;
  $effect(() => dlg.showModal());
</script>

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_noninteractive_element_interactions -->
<dialog bind:this={dlg} {onclose} onclick={(e) => e.target === dlg && dlg.close()} aria-label={title}>
  <header>
    <h2>{title}</h2>
    <button class="x" onclick={() => dlg.close()} aria-label="Close">✕</button>
  </header>
  <div class="content">{@render children()}</div>
</dialog>

<style>
  dialog { width: min(52rem, 94vw); max-height: 88vh; padding: 0; border: 0; border-radius: 12px; box-shadow: 0 20px 60px rgba(0, 0, 0, 0.35); color: var(--ink); background: var(--surface); overflow: hidden; }
  dialog[open] { display: flex; flex-direction: column; }
  dialog::backdrop { background: rgba(0, 0, 0, 0.45); backdrop-filter: blur(2px); }
  header { display: flex; align-items: center; justify-content: space-between; padding: 0.9rem 1.4rem; background: var(--chrome); color: var(--ink); border-bottom: 1px solid var(--line); }
  h2 { margin: 0; font-size: 1.2rem; font-weight: 600; }
  .x { background: transparent; border: 0; color: var(--ink); font-size: 1.1rem; padding: 0.2rem 0.5rem; border-radius: 6px; }
  .x:hover { background: rgba(0, 0, 0, 0.08); }
  .content { overflow: auto; padding: 1.2rem 1.6rem 1.6rem; line-height: 1.55; font-size: 0.93rem; }
  .content :global(h3) { margin: 1.4rem 0 0.4rem; font-size: 1.02rem; color: var(--brand); }
  .content :global(h3:first-child) { margin-top: 0; }
  .content :global(p), .content :global(ul), .content :global(ol) { margin: 0.4rem 0; }
  .content :global(li) { margin: 0.2rem 0; }
  .content :global(table) { border-collapse: collapse; width: 100%; margin: 0.5rem 0; font-size: 0.88rem; }
  .content :global(th), .content :global(td) { text-align: left; padding: 0.3rem 0.6rem; border-bottom: 1px solid var(--line); }
  .content :global(th) { background: var(--panel); }
  .content :global(.note) { background: var(--panel); border-left: 3px solid #888; padding: 0.5rem 0.8rem; border-radius: 4px; }
</style>
