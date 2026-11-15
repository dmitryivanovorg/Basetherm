<script lang="ts">
  let { label, value = $bindable(), unit = '', step = 'any' }: {
    label: string;
    value: number;
    unit?: string;
    step?: string;
  } = $props();
</script>

<label>
  <span>{label}</span>
  <input type="number" {step} bind:value />
  <span class="unit">{unit}</span>
</label>

<style>
  label { display: contents; }
  input { width: 100%; box-sizing: border-box; }
  .unit { color: #6b6b6b; font-size: 0.85em; }
</style>
