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

<div class="card" style="display:flex; gap: 8px; align-items:center;">
  <input
    type="text"
    placeholder="Enter host (e.g. 8.8.8.8 or example.com)"
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
