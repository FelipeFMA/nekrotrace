import { writable, derived } from 'svelte/store';

// hopData: { [ip]: { ip, hop, hostname, latencies: number[] | (number|null)[] } }
export const hopData = writable({});

export const hopList = derived(hopData, ($hopData) =>
  Object.values($hopData).sort((a, b) => (a.hop ?? 0) - (b.hop ?? 0))
);

export const chartSeries = derived(hopData, ($hopData) =>
  Object.values($hopData).map((h) => ({
    name: h.hostname || h.ip,
    data: (h.latencies || []).map((v) => (v === null ? null : Number(v)))
  }))
);
