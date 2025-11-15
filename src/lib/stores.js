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

  const data = hops.map((h) => {
    const latencies = Array.isArray(h.latencies) ? h.latencies : [];
    // pick the most recent non-null value
    let latest = null;
    for (let i = latencies.length - 1; i >= 0; i--) {
      const v = latencies[i];
      if (v !== null && v !== undefined) {
        latest = Number(v);
        break;
      }
    }
    const label = h.hostname || h.ip || `Hop ${h.hop ?? ''}`;
    return { x: label, y: latest };
  });

  return [
    {
      name: 'Latency per hop',
      data
    }
  ];
});
