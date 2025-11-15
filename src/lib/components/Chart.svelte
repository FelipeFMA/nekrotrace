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
  // Stairs mode (step line rendering, values unchanged)
  let stairsMode = false;

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
        // Disable mouse drag/scroll interactions (zoom/selection/pan)
        zoom: { enabled: false },
        selection: { enabled: false },
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
      stroke: { curve: stairsMode ? 'stepline' : 'straight', width: 2 },
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

  function transformToStairs(series) {
    try {
      if (!stairsMode || !Array.isArray(series)) return series;
      // Monotonic non-decreasing stairs capped at final hop's RTT.
      return series.map((s, idx) => {
        const data = Array.isArray(s?.data) ? s.data : [];
        // Determine target final RTT from last non-null point
        let finalCap = null;
        for (let i = data.length - 1; i >= 0; i--) {
          const v = data[i]?.y;
          if (typeof v === 'number' && !isNaN(v)) {
            finalCap = v;
            break;
          }
        }
        if (!(typeof finalCap === 'number' && isFinite(finalCap))) {
          // If we can't determine final RTT, keep data unchanged
          return {
            name: s?.name ? `${s.name} (stairs)` : `Series ${idx + 1} (stairs)`,
            data
          };
        }
        let prev = 0;
        const mono = data.map((pt) => {
          const raw = pt?.y;
          const val = typeof raw === 'number' && !isNaN(raw) ? raw : prev; // hold during nulls
          const capped = Math.min(val, finalCap);
          const y = Math.max(prev, capped);
          prev = y;
          return { x: pt?.x, y, label: pt?.label };
        });
        // Ensure last equals finalCap explicitly (in case of all nulls except last)
        if (mono.length) {
          const last = mono[mono.length - 1];
          mono[mono.length - 1] = { ...last, y: finalCap };
        }
        return {
          name: s?.name ? `${s.name} (stairs)` : `Series ${idx + 1} (stairs)`,
          data: mono
        };
      });
    } catch {
      return series;
    }
  }

  function computeYDomain(series) {
    try {
      const vals = (series?.[0]?.data || [])
        .map((p) => p?.y)
        .filter((v) => typeof v === 'number' && !isNaN(v));
      if (!vals.length) return { min: 0, max: 100 };
      const maxVal = Math.max(...vals);
      const minVal = Math.min(...vals);
      const min = Math.min(0, minVal);
      const max = maxVal <= 0 ? 10 : Math.ceil(maxVal * 1.1);
      return { min, max };
    } catch {
      return { min: 0, max: 100 };
    }
  }

  let unsubSeries;
  let unsubMode;
  let unsubHopData;
  onMount(() => {
    try {
      unsubSeries = chartSeries.subscribe(async (series) => {
        try {
          latestSeries = series;
          const toRender = transformToStairs(series);
          if (!chart && chartEl) {
            const mode = $themeMode || 'dark';
            // Establish fixed Y-axis domain from initial data (respecting mode)
            const dom = computeYDomain(toRender);
            fixedYMin = dom.min;
            fixedYMax = dom.max;
            chart = new ApexCharts(chartEl, { ...baseOptions(mode), series: toRender });
            await chart.render();
            ready = true;
          } else if (chart) {
            if (!frozen) {
              await chart.updateSeries(toRender, false);
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
      const base = [{ name: 'Latency per hop', data }];
      return transformToStairs(base);
    } catch {
      const fallback = latestSeries || [{ name: 'Latency per hop', data: [] }];
      return transformToStairs(fallback);
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
            await chart.updateSeries(transformToStairs(latestSeries), false);
          }
        }
      }}>{frozen ? 'Unfreeze' : 'Freeze'}</button>
    </div>
  </div>
  {#if loadErr}
    <div style="color: var(--muted)">Chart failed to load.</div>
  {:else}
    <div class="chart-wrap">
      <div bind:this={chartEl} style="height: 360px;"></div>
      <label class="toggle" title="Stairs mode">
        <input
          type="checkbox"
          bind:checked={stairsMode}
          on:change={async () => {
            const mode = $themeMode || 'dark';
            const currentSeries = frozen
              ? seriesAtFrame(frameIndex)
              : transformToStairs(latestSeries || []);
            const dom = computeYDomain(currentSeries);
            fixedYMin = dom.min;
            fixedYMax = dom.max;
            if (chart) {
              const opts = { ...baseOptions(mode), series: currentSeries };
              await chart.updateOptions(opts, false, true);
            }
          }}
        />
        <span class="slider" aria-hidden="true"></span>
        <span class="label">Stairs</span>
      </label>
    </div>
    {#if !ready}
      <div style="color: var(--muted)">Loading chart…</div>
    {/if}
  {/if}
</div>

<style>
  .chart-wrap {
    position: relative;
  }
  .toggle {
    position: absolute;
    left: 8px;
    bottom: 8px;
    display: inline-flex;
    align-items: center;
    gap: 8px;
    user-select: none;
    background: var(--card, rgba(0,0,0,0.35));
    border: 1px solid var(--border, rgba(255,255,255,0.08));
    padding: 6px 8px;
    border-radius: 8px;
    backdrop-filter: blur(4px);
  }
  .toggle .label {
    color: var(--fg, #c0caf5);
    font-size: 12px;
  }
  .toggle input {
    position: absolute;
    opacity: 0;
    width: 0;
    height: 0;
  }
  .slider {
    position: relative;
    width: 36px;
    height: 20px;
    background: var(--muted, #a9b1d6);
    border-radius: 999px;
    transition: background 120ms ease;
    box-shadow: inset 0 0 0 1px rgba(0,0,0,0.2);
  }
  .slider::after {
    content: '';
    position: absolute;
    top: 2px;
    left: 2px;
    width: 16px;
    height: 16px;
    background: var(--fg, #c0caf5);
    border-radius: 50%;
    transition: transform 120ms ease;
  }
  .toggle input:checked + .slider {
    background: var(--accent, #7aa2f7);
  }
  .toggle input:checked + .slider::after {
    transform: translateX(16px);
  }
  .toggle:focus-within .slider {
    outline: 2px solid var(--accent, #7aa2f7);
    outline-offset: 2px;
  }
</style>
