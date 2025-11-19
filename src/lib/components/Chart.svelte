<script>
  import { onMount } from 'svelte';
  import { chartSeries, hopData } from '$lib/stores';
  import ApexCharts from 'apexcharts';

  let chartEl;
  let chart = null;
  let loadErr = null;
  let ready = false;
  let frozen = false;
  let latestSeries = null;
  let frameIndex = 0;
  let maxFrame = 0;
  // Y-axis domain tracking for manual panning
  let baseYMin = 0;
  let baseYMax = null; // set after first non-null data render
  let viewYMin = 0;
  let viewYMax = null;
  let panBoundMin = 0;
  let panBoundMax = 0;
  let userHasManualPan = false;
  let pendingYFrame = null;
  let activePanId = null;
  let panStartY = 0;
  let panStartView = null;
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

  function baseOptions() {
    const fg = cssVar('--fg', '#c0caf5');
    return {
      chart: {
        // Keep zoom/selection disabled but allow drag-based panning
        zoom: { enabled: false },
        selection: { enabled: false },
        pan: {
          enabled: true,
          type: 'xy'
        },
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
      theme: { mode: 'dark' },
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
        ...(viewYMax !== null
          ? { min: viewYMin, max: viewYMax }
          : {})
      },
      legend: { position: 'top', labels: { colors: fg } }
    };
  }

  function clampSeries(series) {
    try {
      return (series || []).map((s, idx) => {
        const data = Array.isArray(s?.data) ? s.data : [];
        const safe = data.map((pt) => {
          const raw = pt?.y;
          const y = typeof raw === 'number' && !isNaN(raw) ? Math.max(0, raw) : raw;
          return { ...pt, y };
        });
        return {
          name: s?.name ?? `Series ${idx + 1}`,
          data: safe
        };
      });
    } catch {
      return series || [];
    }
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
          const safeRaw = typeof raw === 'number' && !isNaN(raw) ? Math.max(0, raw) : prev;
          const val = typeof safeRaw === 'number' ? safeRaw : prev; // hold during nulls
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
      const min = 0;
      const max = maxVal <= 0 ? 10 : Math.ceil(maxVal * 1.1);
      return { min, max };
    } catch {
      return { min: 0, max: 100 };
    }
  }

  const PAN_PAD_RATIO = 0.65;
  const MIN_WINDOW_RATIO = 0.05;
  const ZOOM_SENSITIVITY = 0.0015;

  function assignYDomain(dom, { preserveView = false } = {}) {
    const sanitizedMin = Math.max(0, dom.min ?? 0);
    const sanitizedMax = Math.max(sanitizedMin + 1, dom.max ?? sanitizedMin + 1);
    const span = Math.max(1, sanitizedMax - sanitizedMin);
    const pad = span * PAN_PAD_RATIO;
    baseYMin = sanitizedMin;
    baseYMax = sanitizedMax;
    panBoundMin = Math.max(0, sanitizedMin - pad);
    panBoundMax = sanitizedMax + pad;

    const windowSize = (viewYMax ?? dom.max) - (viewYMin ?? dom.min) || span;
    if (!preserveView || viewYMax === null) {
      viewYMin = dom.min;
      viewYMax = dom.max;
      userHasManualPan = false;
    } else {
      const maxLimit = Math.max(panBoundMin, panBoundMax - windowSize);
      const clampedMin = clamp(viewYMin, panBoundMin, maxLimit);
      viewYMin = clampedMin;
      viewYMax = clampedMin + windowSize;
    }
  }

  function clamp(val, min, max) {
    if (val < min) return min;
    if (val > max) return max;
    return val;
  }

  function scheduleYRange(min, max, { markManual = false } = {}) {
    viewYMin = min;
    viewYMax = max;
    if (markManual) {
      userHasManualPan = true;
    }
    if (!chart) return;
    const run = () => {
      pendingYFrame = null;
      chart.updateOptions({ yaxis: { min, max } }, false, false).catch((e) => {
        console.error('Y-axis update failed:', e);
      });
    };
    if (typeof requestAnimationFrame === 'function') {
      if (pendingYFrame) cancelAnimationFrame(pendingYFrame);
      pendingYFrame = requestAnimationFrame(run);
    } else {
      run();
    }
  }

  function resetYView() {
    if (baseYMax === null) return;
    userHasManualPan = false;
    scheduleYRange(baseYMin, baseYMax);
  }

  function chartWindowSize() {
    return (viewYMax ?? baseYMax ?? 100) - (viewYMin ?? baseYMin ?? 0) || 1;
  }

  function baseSpan() {
    return (baseYMax ?? 100) - (baseYMin ?? 0) || 1;
  }

  function minWindowSize() {
    return Math.max(0.5, baseSpan() * MIN_WINDOW_RATIO);
  }

  function maxWindowSize() {
    const fullSpan = Math.max(1, panBoundMax - panBoundMin);
    return Math.max(fullSpan, baseSpan());
  }

  function chartPixelHeight() {
    return chartEl?.offsetHeight || 360;
  }

  function beginPointerPan(event) {
    if (!chart || baseYMax === null) return;
    if (typeof event.button === 'number' && event.button !== 0) return;
    activePanId = event.pointerId ?? 'mouse';
    panStartY = event.clientY;
    panStartView = { min: viewYMin ?? baseYMin, max: viewYMax ?? baseYMax };
    chartEl?.setPointerCapture?.(event.pointerId);
    event.preventDefault();
  }

  function updatePointerPan(event) {
    if (activePanId === null) return;
    if (event.pointerId !== undefined && event.pointerId !== activePanId) return;
    event.preventDefault();
    const deltaPx = event.clientY - panStartY;
    const heightPx = chartPixelHeight();
    if (!heightPx) return;
    const windowSize = (panStartView?.max ?? 0) - (panStartView?.min ?? 0) || chartWindowSize();
    const deltaUnits = (deltaPx / heightPx) * windowSize;
    let nextMin = (panStartView?.min ?? 0) + deltaUnits;
    const minLimit = panBoundMin;
    const maxLimit = Math.max(panBoundMin, panBoundMax - windowSize);
    nextMin = clamp(nextMin, minLimit, maxLimit);
    const nextMax = nextMin + windowSize;
    userHasManualPan = true;
    scheduleYRange(nextMin, nextMax);
  }

  function endPointerPan(event) {
    if (activePanId === null) return;
    if (event.pointerId !== undefined && event.pointerId !== activePanId) return;
    chartEl?.releasePointerCapture?.(event.pointerId);
    activePanId = null;
    panStartView = null;
  }

  function handleWheelZoom(event) {
    if (!chart || baseYMax === null) return;
    if (!event.deltaY) return;
    event.preventDefault();
    const clampedDelta = clamp(event.deltaY, -400, 400);
    const windowSize = chartWindowSize();
    const zoomFactor = Math.exp(clampedDelta * ZOOM_SENSITIVITY);
    let nextWindow = windowSize * zoomFactor;
    nextWindow = clamp(nextWindow, minWindowSize(), maxWindowSize());
    const heightPx = chartPixelHeight();
    const pointerRatio = heightPx ? clamp((event.offsetY ?? heightPx / 2) / heightPx, 0, 1) : 0.5;
    const anchor = (viewYMin ?? baseYMin) + windowSize * pointerRatio;
    let nextMin = anchor - nextWindow * pointerRatio;
    const minLimit = panBoundMin;
    const maxLimit = Math.max(panBoundMin, panBoundMax - nextWindow);
    nextMin = clamp(nextMin, minLimit, maxLimit);
    const nextMax = nextMin + nextWindow;
    scheduleYRange(nextMin, nextMax, { markManual: true });
  }


  let unsubSeries;
  let unsubHopData;
  onMount(() => {
    try {
      unsubSeries = chartSeries.subscribe(async (series) => {
        try {
          latestSeries = clampSeries(series);
          const toRender = transformToStairs(latestSeries);
          const dom = computeYDomain(toRender);
          assignYDomain(dom, { preserveView: userHasManualPan });
          if (!chart && chartEl) {
            chart = new ApexCharts(chartEl, { ...baseOptions(), series: toRender });
            await chart.render();
            ready = true;
          } else if (chart) {
            if (!frozen) {
              await chart.updateSeries(toRender, false);
            }
          }
          if (chart) {
            scheduleYRange(viewYMin, viewYMax);
          }
        } catch (e) {
          console.error('ApexCharts error:', e);
          loadErr = e;
        }
      });

      // Track max available frame based on hop latencies
      unsubHopData = hopData.subscribe(async ($hd) => {
        try {
          const hops = Object.values($hd || {});
          const maxSeqs = hops.map(h => {
             if (!Array.isArray(h?.latencies) || h.latencies.length === 0) return 0;
             const last = h.latencies[h.latencies.length - 1];
             return (typeof last === 'object' && last !== null) ? last.seq : 0;
          });
          const newMax = Math.max(0, ...maxSeqs);
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
      unsubHopData?.();
      if (chart) {
        chart.destroy();
        chart = null;
      }
    };
  });

  function seriesAtFrame(targetSeq, $hd = $hopData) {
    try {
      const hops = Object.values($hd || {}).sort((a, b) => (a.hop ?? 0) - (b.hop ?? 0));
      const data = hops.map((h, i) => {
        const latencies = Array.isArray(h.latencies) ? h.latencies : [];
        const hopNumber = h.hop ?? i + 1;
        const label = h.hostname || h.ip || `Hop ${hopNumber}`;
        
        // Find the latest non-null value at or before targetSeq
        // This mimics the live view behavior which ignores timeouts/nulls
        let val = null;
        for (let k = latencies.length - 1; k >= 0; k--) {
            const l = latencies[k];
            if (typeof l === 'object' && l !== null && l.seq <= targetSeq) {
                if (l.val !== null && l.val !== undefined) {
                    val = l.val;
                    break;
                }
            }
        }
        
        const y = (val === null || val === undefined) ? null : Math.max(0, Number(val));
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
      <button class="button" title="Reset pan" on:click={resetYView}>Reset view</button>
    </div>
  </div>
  {#if loadErr}
    <div style="color: var(--muted)">Chart failed to load.</div>
  {:else}
    <div class="chart-wrap">
      <div
        bind:this={chartEl}
        class="chart-surface"
        on:pointerdown={beginPointerPan}
        on:pointermove={updatePointerPan}
        on:pointerup={endPointerPan}
        on:pointerleave={endPointerPan}
        on:pointercancel={endPointerPan}
        on:wheel={handleWheelZoom}
      ></div>
      <label class="toggle" title="Stairs mode">
        <input
          type="checkbox"
          bind:checked={stairsMode}
          on:change={async () => {
            const currentSeries = frozen
              ? seriesAtFrame(frameIndex)
              : transformToStairs(latestSeries || []);
            const dom = computeYDomain(currentSeries);
            assignYDomain(dom, { preserveView: userHasManualPan });
            if (chart) {
              const opts = { ...baseOptions(), series: currentSeries };
              await chart.updateOptions(opts, false, true);
              scheduleYRange(viewYMin, viewYMax);
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
  .chart-surface {
    height: 360px;
    cursor: grab;
  }
  .chart-surface:active {
    cursor: grabbing;
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
