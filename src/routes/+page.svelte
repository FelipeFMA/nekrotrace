<script>
  import { onMount } from 'svelte';
  import { listen } from '@tauri-apps/api/event';
  import { WebviewWindow } from '@tauri-apps/api/webviewWindow';
  import InputBar from '$lib/components/InputBar.svelte';
  import HopList from '$lib/components/HopList.svelte';
  import Chart from '$lib/components/Chart.svelte';
  import { hopData } from '$lib/stores';

  let debug = { listenersReady: false, hopCount: 0, lastPing: null };
  let uiLogs = [];
  function logUI(msg) {
    const line = `[${new Date().toLocaleTimeString()}] ${msg}`;
    uiLogs = [...uiLogs.slice(-99), line];
    console.log('[UI]', msg);
  }

  onMount(async () => {
    logUI('Page mounted: registering Tauri event listeners');
    let unlistenHopList = null;
    let unlistenPing = null;
    try {
      const current = WebviewWindow.getCurrent();
      unlistenHopList = await current.listen(
        'hop_list_updated',
        (event) => {
          logUI(`hop_list_updated received`);
      const hops = event.payload || [];
      debug.hopCount = Array.isArray(hops) ? hops.length : 0;
          logUI(`setting hopData with ${debug.hopCount} hops`);
      hopData.set(
        Object.fromEntries(
          hops.map((h) => [h.ip, { ...h, latencies: [] }])
        )
      );
        }
      );

      unlistenPing = await current.listen(
        'new_ping_data',
        (event) => {
          const data = event.payload;
          if (data && data.latency != null) {
            logUI(`new_ping_data ${data.ip} ${data.status} ${data.latency}ms`);
          }
          if (!data || !data.ip) return;
          debug.lastPing = { ip: data.ip, status: data.status, latency: data.latency };
          hopData.update((state) => {
            const current = state[data.ip] || { ip: data.ip, hop: null, hostname: data.ip, latencies: [] };
            const nextLatencies = [...(current.latencies || []), data.latency == null ? null : Number(data.latency)].slice(-60);
            return { ...state, [data.ip]: { ...current, latencies: nextLatencies } };
          });
        }
      );
      logUI('Listeners registered and ready');
      debug.listenersReady = true;
    } catch (e) {
      logUI(`Tauri event binding failed: ${e?.message || e}`);
    }

    return () => {
      logUI('Page unmount: cleaning up listeners');
      unlistenHopList?.();
      unlistenPing?.();
    };
  });
</script>

<div class="container">
  <h1 class="title">NekroTrace</h1>

  <div style="margin-bottom: 16px;">
    <InputBar />
  </div>

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
