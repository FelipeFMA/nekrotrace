<script>
  import { invoke } from '@tauri-apps/api/core';
  let host = '';
  let starting = false;

  async function start() {
    if (!host) return;
    try {
      console.log('Invoking start_trace with host:', host);
      starting = true;
      await invoke('start_trace', { host });
      console.log('invoke start_trace resolved');
    } catch (e) {
      console.error('Failed to start trace:', e);
    } finally {
      starting = false;
    }
  }
</script>

<div class="card" style="display:flex; gap: 8px; align-items:center;">
  <input
    type="text"
    placeholder="Enter host (e.g. 8.8.8.8 or example.com)"
    bind:value={host}
    on:keydown={(e) => e.key === 'Enter' && start()} />
  <button class="button" on:click={start} disabled={starting}>
    {starting ? 'Starting…' : 'Start'}
  </button>
  
</div>
