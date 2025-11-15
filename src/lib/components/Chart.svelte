<script>
  import { onMount } from 'svelte';
  import { chartSeries } from '$lib/stores';
  import ApexCharts from 'apexcharts';

  let chartEl;
  let chart = null;
  let loadErr = null;
  let ready = false;
  let paused = false;
  let pendingSeries = null;

  const options = {
    chart: {
      animations: {
        enabled: true,
        easing: 'linear',
        dynamicAnimation: { speed: 1000 }
      },
      background: 'transparent',
      toolbar: { show: false }
    },
    tooltip: { enabled: true, x: { show: false } },
    stroke: { curve: 'straight', width: 2 },
    markers: { size: 5 },
    theme: { mode: 'dark' },
    xaxis: { labels: { show: false } },
    yaxis: { title: { text: 'Latency (ms)' } },
    legend: { position: 'top', labels: { colors: '#c0caf5' } }
  };

  let unsubscribe;
  onMount(() => {
    try {
      unsubscribe = chartSeries.subscribe(async (series) => {
        try {
          if (!chart && chartEl) {
            chart = new ApexCharts(chartEl, { ...options, series });
            await chart.render();
            ready = true;
          } else if (chart) {
            if (paused) {
              // stash the latest update and skip rendering while paused
              pendingSeries = series;
            } else {
              await chart.updateSeries(series, true);
            }
          }
        } catch (e) {
          console.error('ApexCharts error:', e);
          loadErr = e;
        }
      });
    } catch (e) {
      console.error('Chart init failed:', e);
      loadErr = e;
    }

    return () => {
      unsubscribe?.();
      if (chart) {
        chart.destroy();
        chart = null;
      }
    };
  });
</script>

<div class="card">
  <div style="display:flex; align-items:center; justify-content:space-between; gap: 8px;">
    <div class="title">Latency per Hop</div>
    <button class="button" on:click={() => {
      paused = !paused;
      if (!paused && pendingSeries && chart) {
        chart.updateSeries(pendingSeries, true);
        pendingSeries = null;
      }
    }}>{paused ? 'Resume' : 'Pause'}</button>
  </div>
  {#if loadErr}
    <div style="color: var(--muted)">Chart failed to load.</div>
  {:else}
    <div bind:this={chartEl} style="height: 360px;"></div>
    {#if !ready}
      <div style="color: var(--muted)">Loading chart…</div>
    {/if}
  {/if}
</div>
