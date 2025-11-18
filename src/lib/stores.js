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
export const themeOptions = [
  {
    id: 'catppuccin-mocha-dark',
    label: 'Catppuccin Mocha (Dark)',
    mode: 'dark'
  },
  {
    id: 'catppuccin-latte-light',
    label: 'Catppuccin Latte (Light)',
    mode: 'light'
  },
  {
    id: 'gruvbox-dark',
    label: 'Gruvbox (Dark)',
    mode: 'dark'
  },
  {
    id: 'gruvbox-light',
    label: 'Gruvbox (Light)',
    mode: 'light'
  }
];

const DEFAULT_THEME = 'catppuccin-mocha-dark';

function applyTheme(id) {
  try {
    if (typeof document !== 'undefined') {
      document.documentElement.setAttribute('data-theme', id);
    }
  } catch {}
}

const initialTheme = (() => {
  try {
    const saved = localStorage.getItem('nekrotrace.theme');
    return saved || DEFAULT_THEME;
  } catch {
    return DEFAULT_THEME;
  }
})();

export const theme = writable(initialTheme);
export const themeMode = derived(theme, ($t) => {
  const opt = themeOptions.find((o) => o.id === $t);
  return opt?.mode || 'dark';
});

// persist + apply on change
if (typeof window !== 'undefined') {
  applyTheme(initialTheme);
  theme.subscribe((val) => {
    try {
      localStorage.setItem('nekrotrace.theme', val);
    } catch {}
    applyTheme(val);
  });
}
