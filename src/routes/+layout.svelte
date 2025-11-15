<script>
  import "../app.css";
  import { theme, themeOptions } from '$lib/stores';
</script>

<div class="app-shell">
  <header class="app-header">
    <div class="spacer"></div>
    <label class="theme-select">
      <span>Theme</span>
      <select bind:value={$theme} aria-label="Theme selector">
        {#each themeOptions as opt}
          <option value={opt.id}>{opt.label}</option>
        {/each}
      </select>
    </label>
  </header>
  <main>
    <slot />
  </main>
</div>
