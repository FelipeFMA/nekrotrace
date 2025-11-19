<script>
  import { onMount } from 'svelte';
  import { listen } from '$lib/api';
  import { hopData, tracing, viewMode } from '$lib/stores';
  import GraphView from '$lib/components/GraphView.svelte';
  import MonitoringView from '$lib/components/MonitoringView.svelte';

  let debug = { listenersReady: false, hopCount: 0, lastPing: null };
  let pingUpdateQueue = [];
  let uiLogs = [];
  function logUI(msg) {
    const line = `[${new Date().toLocaleTimeString()}] ${msg}`;
    uiLogs = [...uiLogs.slice(-99), line];
    console.log('[UI]', msg);
  }

  onMount(() => {
    logUI('Page mounted: registering Tauri event listeners');
    let unlistenHopList = null;
    let unlistenPing = null;
    let intervalId = null;

    const setupListeners = async () => {
      try {
        unlistenHopList = await listen(
          'hop_list_updated',
          (event) => {
            logUI(`hop_list_updated received`);
            const hops = event.payload || [];
            debug.hopCount = Array.isArray(hops) ? hops.length : 0;
            logUI(`setting hopData with ${debug.hopCount} hops`);
            if ($tracing) {
              hopData.set(
                Object.fromEntries(
                  hops.map((h) => [h.ip, { ...h, latencies: h.initial_latency != null ? [{ seq: 0, val: h.initial_latency }] : [] }])
                )
              );
            }
          }
        );

        unlistenPing = await listen(
          'new_ping_data',
          (event) => {
            const data = event.payload;
            if (!data || !data.ip) return;
            pingUpdateQueue.push(data);
          }
        );

        // Process queue at 20fps (50ms)
        intervalId = setInterval(() => {
          if (pingUpdateQueue.length === 0) return;
          
          const updates = [...pingUpdateQueue];
          pingUpdateQueue = [];
          
          const last = updates[updates.length - 1];
          debug.lastPing = { ip: last.ip, status: last.status, latency: last.latency };

          if ($tracing) {
            hopData.update((state) => {
              const newState = { ...state };
              for (const d of updates) {
                  const current = newState[d.ip] || { ip: d.ip, hop: null, hostname: d.ip, latencies: [] };
                  const nextLatencies = [...(current.latencies || []), { seq: d.seq, val: d.latency == null ? null : Number(d.latency) }].slice(-60);
                  newState[d.ip] = { ...current, latencies: nextLatencies };
              }
              return newState;
            });
          }
        }, 50);

        logUI('Listeners registered and ready (buffered)');
        debug.listenersReady = true;
      } catch (e) {
        logUI(`Tauri event binding failed: ${e?.message || e}`);
      }
    };

    setupListeners();

    return () => {
      logUI('Page unmount: cleaning up listeners');
      if (unlistenHopList) unlistenHopList();
      if (unlistenPing) unlistenPing();
      if (intervalId) clearInterval(intervalId);
    };
  });
</script>

{#if $viewMode === 'graph'}
  <GraphView {debug} {uiLogs} />
{:else}
  <MonitoringView />
{/if}
