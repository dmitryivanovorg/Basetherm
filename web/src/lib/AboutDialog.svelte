<script lang="ts">
  import Modal from './Modal.svelte';
  let { onclose }: { onclose: () => void } = $props();
</script>

<Modal title="About Basetherm" {onclose}>
  <h3>Basetherm v{__APP_VERSION__}</h3>
  <p>
    Basetherm is a free and open-source tool for simulating heat transfer between a building basement into the ground and outdoor environment. Its main purpose is to accurately calculate heat loss (gains) of the underground spaces and to size the mechanical systems. The tool based on the author's PhD thesis and on the original Basetherm (version 1), a Windows program that is available in Russian at
    <a href="https://dmitryivanov.org/basetherm" target="_blank" rel="noopener">dmitryivanov.org/basetherm</a>.
    Version 2 is a rewrite as a web application, with a an updated three-dimensional enthalpy solver that runs entirely in your
    browser.
  </p>

  <h3>Disclaimer</h3>
  <p class="note">
    This software is provided "as is", without warranty of any kind, express or implied. The author does not
    accept responsibility or liability for any use of this tool or for any results, decisions, designs, losses or damages
    arising from it or from reliance on it. The results are model estimates that depend on your inputs and on the
    limitations described in the Guide. You are responsible for checking them and for any use you make of them.
  </p>

  <h3>License and source code</h3>
  <p>
    Basetherm is free software distributed under the
    <a href="https://www.gnu.org/licenses/gpl-3.0.html" target="_blank" rel="noopener">GNU General Public License v3.0</a>
    (GNU GPLv3). You may redistribute and modify it under the terms of that license. Source code:
    <a href="https://github.com/dmitryivanovorg/Basetherm" target="_blank" rel="noopener">github.com/dmitryivanovorg/Basetherm</a>.
  </p>

  <h3>Copyright</h3>
  <p>
    © 2026 Dmitry Ivanov.
    <a href="https://dmitryivanov.org" target="_blank" rel="noopener">dmitryivanov.org</a>
  </p>

  <p class="made">Made in Melbourne with <span class="heart" aria-label="love">♥</span></p>
</Modal>

<style>
  .made { margin-top: 1.6rem; text-align: center; color: var(--muted); }
  .heart { color: #000; }
  a { color: var(--brand); }
</style>
