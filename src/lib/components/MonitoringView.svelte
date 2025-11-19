<script>
  import { hopList, hostInput } from '$lib/stores';

  $: hops = $hopList;
  $: target = $hostInput;
  
  // Logic to determine status and latency
  // Assuming the last hop is the target or close to it.
  // If the last hop has a valid latency, we consider it "Up".
  $: lastHop = hops.length > 0 ? hops[hops.length - 1] : null;
  $: currentLatencyVal = (lastHop && lastHop.latencies && lastHop.latencies.length > 0) 
      ? lastHop.latencies[lastHop.latencies.length - 1].val 
      : null;
  
  $: isUp = currentLatencyVal !== null;
  $: latencyDisplay = currentLatencyVal !== null ? currentLatencyVal.toFixed(1) : '--';
  $: totalHops = hops.length;

  function getRowClass(hop) {
    const lat = (hop.latencies && hop.latencies.length > 0) ? hop.latencies[hop.latencies.length - 1].val : null;
    if (lat === null) return 'row-timeout'; // Or maybe just dark
    if (lat > 100) return 'row-high';
    if (lat > 40) return 'row-medium';
    return 'row-good';
  }
</script>

{#if !target}
  <div class="empty-state">
    <div class="empty-msg">Start a trace to view dashboard</div>
  </div>
{:else}
<div class="monitoring-layout">
  <aside class="sidebar">
    <div class="status-card" class:up={isUp} class:down={!isUp}>
      <div class="status-text">{isUp ? 'Up' : 'Down'}</div>
      <div class="status-bar"></div>
    </div>

    <div class="metric-card latency-card">
      <div class="metric-value">{latencyDisplay} <span class="unit">ms</span></div>
      <div class="sparkline-placeholder">
        <!-- Simple CSS bars to simulate sparkline -->
        <div class="bar" style="height: 30%"></div>
        <div class="bar" style="height: 50%"></div>
        <div class="bar" style="height: 20%"></div>
        <div class="bar" style="height: 80%"></div>
        <div class="bar" style="height: 40%"></div>
        <div class="bar" style="height: 60%"></div>
      </div>
    </div>

    <div class="metric-card hops-card">
      <div class="metric-label">Total de Saltos</div>
      <div class="metric-value-large">{totalHops}</div>
    </div>
  </aside>

  <main class="main-view">
    <header class="view-header">
      Tracert - {target}
    </header>
    
    <div class="table-wrapper">
      <div class="table-header">
        <div class="th col-hop">Salto</div>
        <div class="th col-host">Host</div>
        <div class="th col-last">Ultimo</div>
      </div>
      <div class="table-body">
        {#each hops as hop}
          {@const lat = (hop.latencies && hop.latencies.length > 0) ? hop.latencies[hop.latencies.length - 1].val : null}
          <div class="table-row {getRowClass(hop)}">
            <div class="td col-hop">{hop.hop}</div>
            <div class="td col-host">{hop.hostname || hop.ip}</div>
            <div class="td col-last">{lat !== null ? lat.toFixed(2) + ' ms' : '*'}</div>
          </div>
        {/each}
      </div>
    </div>
  </main>
</div>
{/if}

<style>
  .monitoring-layout {
    display: flex;
    height: calc(100vh - 60px); /* Adjust based on header height */
    background-color: #111;
    color: white;
    font-family: sans-serif;
    overflow: hidden;
  }

  .empty-state {
    display: flex;
    align-items: center;
    justify-content: center;
    height: calc(100vh - 60px);
    background-color: #111;
    color: var(--muted);
    font-family: sans-serif;
  }
  .empty-msg {
    font-size: 18px;
  }

  .sidebar {
    width: 200px;
    background-color: #1e1e1e;
    display: flex;
    flex-direction: column;
    padding: 10px;
    gap: 10px;
    border-right: 1px solid #333;
  }

  .status-card {
    background-color: #333;
    padding: 20px;
    border-radius: 4px;
    text-align: center;
    display: flex;
    flex-direction: column;
    justify-content: center;
    min-height: 100px;
  }
  .status-card.up { background-color: #2e7d32; }
  .status-card.down { background-color: #c62828; }
  
  .status-text {
    font-size: 32px;
    font-weight: bold;
  }
  .status-bar {
    height: 8px;
    background: rgba(255,255,255,0.3);
    margin-top: 10px;
    border-radius: 4px;
  }

  .metric-card {
    padding: 15px;
    border-radius: 4px;
    min-height: 100px;
    display: flex;
    flex-direction: column;
    justify-content: center;
  }
  .latency-card { background-color: #e65100; } /* Orange */
  .hops-card { background-color: #e65100; }

  .metric-value {
    font-size: 28px;
    font-weight: bold;
  }
  .unit { font-size: 16px; font-weight: normal; }
  
  .metric-label { font-size: 14px; margin-bottom: 5px; }
  .metric-value-large { font-size: 48px; font-weight: bold; text-align: center; }

  .sparkline-placeholder {
    display: flex;
    align-items: flex-end;
    gap: 4px;
    height: 40px;
    margin-top: 10px;
  }
  .bar {
    width: 6px;
    background: rgba(255,255,255,0.5);
    border-radius: 2px;
  }

  .main-view {
    flex: 1;
    display: flex;
    flex-direction: column;
    background-color: #121212;
  }

  .view-header {
    padding: 15px 20px;
    font-size: 18px;
    font-weight: bold;
    border-bottom: 1px solid #333;
  }

  .table-wrapper {
    flex: 1;
    overflow-y: auto;
    padding: 20px;
  }

  .table-header {
    display: flex;
    padding: 10px;
    font-weight: bold;
    color: #aaa;
    border-bottom: 1px solid #333;
  }

  .table-row {
    display: flex;
    padding: 12px 10px;
    margin-bottom: 2px;
    border-radius: 2px;
    font-weight: 500;
  }

  .col-hop { width: 60px; text-align: center; }
  .col-host { flex: 1; padding-left: 20px; }
  .col-last { width: 100px; text-align: right; }

  .row-good { background-color: #1b5e20; color: #fff; } /* Green */
  .row-medium { background-color: #2e7d32; color: #fff; } /* Lighter Green */
  .row-high { background-color: #e65100; color: #fff; } /* Orange */
  .row-timeout { background-color: #333; color: #888; }

</style>
