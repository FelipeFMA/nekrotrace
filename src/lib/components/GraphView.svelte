<script>
  import HopList from './HopList.svelte';
  import Chart from './Chart.svelte';

  export let debug = {};
  export let uiLogs = [];
</script>

<div class="container">
  <h1 class="title">NekroTrace</h1>

  <div class="row">
    <Chart />
    <HopList />
  </div>

  <div class="card" style="margin-top: 12px;">
    <div class="title">Debug</div>
    <div style="font-size: 12px; color: var(--muted)">
      listeners: {debug.listenersReady ? 'ready' : 'not ready'} | hops: {debug.hopCount} | last ping: {debug.lastPing ? `${debug.lastPing.ip} ${debug.lastPing.status} ${debug.lastPing.latency ?? ''}` : '—'}
    </div>
    <pre style="max-height: 140px; overflow:auto; font-size: 11px; white-space: pre-wrap; color: var(--muted)">{uiLogs.join('\n')}</pre>
  </div>
</div>
