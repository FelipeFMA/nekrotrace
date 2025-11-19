import { writable, derived } from 'svelte/store';

// hopData: { [ip]: { ip, hop, hostname, latencies: number[] | (number|null)[] } }
export const hopData = writable({});

// current host input value
export const hostInput = writable('');

// whether a trace session is active
export const tracing = writable(false);

export const hopList = derived(hopData, ($hopData) =>
  Object.values($hopData).sort((a, b) => (a.hop ?? 0) - (b.hop ?? 0))
);

// Single line where each point is a hop (latest latency per hop)
export const chartSeries = derived(hopData, ($hopData) => {
  const hops = Object.values($hopData).sort(
    (a, b) => (a.hop ?? 0) - (b.hop ?? 0)
  );

  const data = hops.map((h, i) => {
    const latencies = Array.isArray(h.latencies) ? h.latencies : [];
    // pick the most recent non-null value
    let latest = null;
    for (let i = latencies.length - 1; i >= 0; i--) {
      const item = latencies[i];
      const v = (typeof item === 'object' && item !== null) ? item.val : item;
      if (v !== null && v !== undefined) {
        latest = Number(v);
        break;
      }
    }
    // Use hop number for the X value so the bottom axis
    // can display the same numbers as the Discovered Hops list
    const hopNumber = h.hop ?? i + 1;
    const label = h.hostname || h.ip || `Hop ${hopNumber}`;
    return { x: hopNumber, y: latest, label };
  });

  return [
    {
      name: 'Latency per hop',
      data
    }
  ];
});

// Theme support
// Removed theme options as we are using a single monitoring theme

export const viewMode = writable('graph'); // 'graph' | 'dashboard'
