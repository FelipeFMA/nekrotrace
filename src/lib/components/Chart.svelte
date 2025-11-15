<script>
  import { onMount } from 'svelte';
  import { chartSeries, themeMode, hopData } from '$lib/stores';
  import ApexCharts from 'apexcharts';

  let chartEl;
  let chart = null;
  let loadErr = null;
  let ready = false;
  let frozen = false;
  let latestSeries = null;
  let frameIndex = 0;
  let maxFrame = 0;
  // Fixed Y-axis domain once established so spikes do not rescale chart
  let fixedYMin = 0;
  let fixedYMax = null; // will be set after first non-null data render

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
      tooltip: {
        enabled: true,
        x: { show: false },
        custom: ({ series, seriesIndex, dataPointIndex, w }) => {
          try {
            const point = w?.config?.series?.[seriesIndex]?.data?.[dataPointIndex] || {};
            const latency = series?.[seriesIndex]?.[dataPointIndex];
            const latencyText = latency === null || latency === undefined ? 'timeout' : `${latency} ms`;
            const name = point.label ?? '';
            const hop = point.x ?? '';
            const fg = cssVar('--fg', '#c0caf5');
            const bg = cssVar('--card', 'rgba(0,0,0,0.7)');
            const muted = cssVar('--muted', '#a9b1d6');
            return `
              <div style="background:${bg}; color:${fg}; padding:8px 10px; border-radius:6px; font-size:12px;">
                <div style="font-weight:600; margin-bottom:2px;">${name}</div>
                <div style="color:${muted};">Hop ${hop} • ${latencyText}</div>
              </div>
            `;
          } catch {
            return '';
          }
        }
      },
      stroke: { curve: 'straight', width: 2 },
      markers: { size: 5 },
      theme: { mode },
      xaxis: {
        type: 'category',
        title: { text: 'Hop', style: { color: fg } },
        labels: {
          show: true,
          style: { colors: fg },
          formatter: (val) => (val == null || val === '' ? '' : String(val))
        }
      },
      yaxis: {
        title: { text: 'Latency (ms)', style: { color: fg } },
        labels: { style: { colors: fg } },
        // Apply fixed domain if established
        ...(fixedYMax !== null
          ? { min: fixedYMin, max: fixedYMax }
          : {})
      },
      legend: { position: 'top', labels: { colors: fg } }
    };
  }

  let unsubSeries;
  let unsubMode;
  let unsubHopData;
  onMount(() => {
    try {
      unsubSeries = chartSeries.subscribe(async (series) => {
        try {
          latestSeries = series;
          if (!chart && chartEl) {
            const mode = $themeMode || 'dark';
            // Establish fixed Y-axis domain from initial data
            const initialVals = (series?.[0]?.data || [])
              .map(p => p?.y)
              .filter(v => typeof v === 'number' && !isNaN(v));
            if (initialVals.length) {
              const maxVal = Math.max(...initialVals);
              const minVal = Math.min(...initialVals);
              fixedYMin = Math.min(0, minVal); // keep at or below 0
              // Add a small headroom (10%) so top points are not clipped by marker
              fixedYMax = maxVal <= 0 ? 10 : Math.ceil(maxVal * 1.1);
            } else {
              // Fallback domain if no data yet
              fixedYMin = 0;
              fixedYMax = 100;
            }
            chart = new ApexCharts(chartEl, { ...baseOptions(mode), series });
            await chart.render();
            ready = true;
          } else if (chart) {
            if (!frozen) {
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

      // Track max available frame based on hop latencies
      unsubHopData = hopData.subscribe(async ($hd) => {
        try {
          const hops = Object.values($hd || {});
          const lengths = hops.map(h => Array.isArray(h?.latencies) ? h.latencies.length : 0);
          const newMax = Math.max(0, ...(lengths.length ? lengths : [0])) - 1;
          const prevMax = maxFrame;
          maxFrame = isFinite(newMax) ? Math.max(0, newMax) : 0;
          if (frozen) {
            if (frameIndex > maxFrame) {
              frameIndex = maxFrame;
              // If chart exists and we are frozen, keep the view consistent
              if (chart) {
                await chart.updateSeries(seriesAtFrame(frameIndex, $hd), false);
              }
            }
          } else if (!ready && latestSeries && chart && prevMax !== maxFrame) {
            // no-op placeholder; live mode already updates via chartSeries
          }
        } catch (e) {
          console.error('HopData update error:', e);
        }
      });
    } catch (e) {
      console.error('Chart init failed:', e);
      loadErr = e;
    }

    return () => {
      unsubSeries?.();
      unsubMode?.();
      unsubHopData?.();
      if (chart) {
        chart.destroy();
        chart = null;
      }
    };
  });

  function seriesAtFrame(idx, $hd = $hopData) {
    try {
      const hops = Object.values($hd || {}).sort((a, b) => (a.hop ?? 0) - (b.hop ?? 0));
      const data = hops.map((h, i) => {
        const latencies = Array.isArray(h.latencies) ? h.latencies : [];
        const hopNumber = h.hop ?? i + 1;
        const label = h.hostname || h.ip || `Hop ${hopNumber}`;
        const val = latencies[idx];
        const y = (val === null || val === undefined) ? null : Number(val);
        return { x: hopNumber, y, label };
      });
      return [{ name: 'Latency per hop', data }];
    } catch {
      return latestSeries || [{ name: 'Latency per hop', data: [] }];
    }
  }
</script>

<div class="card">
  <div style="display:flex; align-items:center; justify-content:space-between; gap: 8px;">
    <div class="title">Latency per Hop</div>
    <div style="display:flex; gap:6px; align-items:center;">
      {#if frozen}
        <button class="button" title="Previous frame" on:click={async () => {
          if (!chart) return;
          frameIndex = Math.max(0, frameIndex - 1);
          await chart.updateSeries(seriesAtFrame(frameIndex), false);
        }}>&larr;</button>
        <button class="button" title="Next frame" on:click={async () => {
          if (!chart) return;
          frameIndex = Math.min(maxFrame, frameIndex + 1);
          await chart.updateSeries(seriesAtFrame(frameIndex), false);
        }}>&rarr;</button>
      {/if}
      <button class="button" on:click={async () => {
        frozen = !frozen;
        if (frozen) {
          // Snap to latest available frame
          frameIndex = maxFrame;
          if (chart) {
            await chart.updateSeries(seriesAtFrame(frameIndex), false);
          }
        } else {
          // Return to live updates
          if (latestSeries && chart) {
            await chart.updateSeries(latestSeries, false);
          }
        }
      }}>{frozen ? 'Unfreeze' : 'Freeze'}</button>
    </div>
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
