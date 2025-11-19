<script>
  import "../app.css";
  import { viewMode } from '$lib/stores';
  import InputBar from '$lib/components/InputBar.svelte';
</script>

<div class="app-shell">
  <header class="app-header">
    <div class="view-tabs">
      <button class:active={$viewMode === 'graph'} on:click={() => $viewMode = 'graph'}>Graph</button>
      <button class:active={$viewMode === 'dashboard'} on:click={() => $viewMode = 'dashboard'}>Dashboard</button>
    </div>
    <div class="spacer"></div>
    <InputBar />
  </header>
  <main>
    <slot />
  </main>
</div>

<style>
  .view-tabs {
    display: flex;
    gap: 4px;
    background: var(--input-bg);
    padding: 4px;
    border-radius: 8px;
    border: 1px solid var(--input-border);
  }
  .view-tabs button {
    background: transparent;
    border: none;
    color: var(--muted);
    padding: 6px 12px;
    border-radius: 6px;
    cursor: pointer;
    font-weight: 600;
    font-size: 13px;
    transition: all 0.2s;
  }
  .view-tabs button.active {
    background: var(--bg);
    color: var(--accent);
    box-shadow: 0 1px 3px rgba(0,0,0,0.1);
  }
  .view-tabs button:hover:not(.active) {
    color: var(--fg);
  }
  .spacer {
    flex: 1;
  }
</style>
