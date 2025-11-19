<script>
  import { invoke } from '@tauri-apps/api/core';
  import { hopData, hostInput, tracing } from '$lib/stores';
  let starting = false;
  let stopping = false;

  $: host = $hostInput;

  async function start() {
    if (!host) return;
    try {
      console.log('Invoking start_trace with host:', host);
      starting = true;
      // mark tracing and persist host in store
      tracing.set(true);
      hostInput.set(host);
      await invoke('start_trace', { host });
      console.log('invoke start_trace resolved');
    } catch (e) {
      console.error('Failed to start trace:', e);
      tracing.set(false);
    } finally {
      starting = false;
    }
  }

  async function stop() {
    try {
      stopping = true;
      await invoke('stop_trace');
    } catch (e) {
      console.error('Failed to stop trace:', e);
    } finally {
      // clear UI state regardless
      hopData.set({});
      hostInput.set('');
      tracing.set(false);
      starting = false;
      stopping = false;
    }
  }
</script>

<div class="input-bar-container">
  <input
    type="text"
    class="host-input"
    placeholder="Enter host (e.g. 8.8.8.8)"
    bind:value={$hostInput}
    on:keydown={(e) => e.key === 'Enter' && (!$tracing ? start() : stop())}
    disabled={starting || stopping} />
  {#if $tracing}
    <button class="button" on:click={stop} disabled={stopping || starting}>
      {stopping ? 'Stopping…' : (starting ? 'Starting…' : 'Stop')}
    </button>
  {:else}
    <button class="button" on:click={start} disabled={starting || !host}>
      {starting ? 'Starting…' : 'Start'}
    </button>
  {/if}
</div>

<style>
  .input-bar-container {
    display: flex;
    gap: 8px;
    align-items: center;
    flex: 1;
    max-width: 600px;
  }
  .host-input {
    flex: 1;
    min-width: 200px;
    background: var(--input-bg);
    border: 1px solid var(--input-border);
    color: var(--fg);
    border-radius: 6px;
    padding: 6px 12px;
    outline: none;
    font-size: 14px;
  }
  .host-input:focus {
    border-color: var(--accent);
  }
  button {
    white-space: nowrap;
    padding: 6px 12px;
    font-size: 14px;
  }
</style>
