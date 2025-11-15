<script>
  import { onMount } from 'svelte';
  import { chartSeries, themeMode } from '$lib/stores';
  import ApexCharts from 'apexcharts';

  let chartEl;
  let chart = null;
  let loadErr = null;
  let ready = false;
  let paused = false;
  let pendingSeries = null;

  function cssVar(name, fallback) {
    try {
      const v = getComputedStyle(document.documentElement).getPropertyValue(name).trim();
      return v || fallback;
    } catch {
      return fallback;
    }
  }

  function baseOptions(mode) {
    const fg = cssVar('--fg', '#c0caf5');
    return {
      chart: {
        animations: {
          enabled: false,
          easing: 'linear',
          dynamicAnimation: { speed: 0 }
        },
        background: 'transparent',
        toolbar: { show: false }
      },
      tooltip: { enabled: true, x: { show: false } },
      stroke: { curve: 'straight', width: 2 },
      markers: { size: 5 },
      theme: { mode },
      xaxis: { labels: { show: false } },
      yaxis: {
        title: { text: 'Latency (ms)', style: { color: fg } },
        labels: { style: { colors: fg } }
      },
      legend: { position: 'top', labels: { colors: fg } }
    };
  }

  let unsubSeries;
  let unsubMode;
  onMount(() => {
    try {
      unsubSeries = chartSeries.subscribe(async (series) => {
        try {
          if (!chart && chartEl) {
            const mode = $themeMode || 'dark';
            chart = new ApexCharts(chartEl, { ...baseOptions(mode), series });
            await chart.render();
            ready = true;
          } else if (chart) {
            if (paused) {
              pendingSeries = series;
            } else {
              await chart.updateSeries(series, false);
            }
          }
        } catch (e) {
          console.error('ApexCharts error:', e);
          loadErr = e;
        }
      });

      unsubMode = themeMode.subscribe(async (mode) => {
        try {
          if (chart) {
            await chart.updateOptions(baseOptions(mode), false, true);
          }
        } catch (e) {
          console.error('ApexCharts theme update error:', e);
        }
      });
    } catch (e) {
      console.error('Chart init failed:', e);
      loadErr = e;
    }

    return () => {
      unsubSeries?.();
      unsubMode?.();
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
        chart.updateSeries(pendingSeries, false);
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
