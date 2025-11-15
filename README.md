NekroTrace
===========

High-performance, beautiful network diagnostic tool built with Tauri (Rust backend) and SvelteKit (frontend). It discovers route hops to a target, emits hop data, and continuously pings each hop at 1-second intervals to visualize latency in real time.

Features
- Traceroute to discover hops (TTL, IP, hostname)
- Continuous per-hop ping loop with real-time updates
- Smooth, dark-mode charts using ApexCharts
- Animated hop list with Svelte transitions

Requirements
- Node.js 18+
- Rust toolchain + Cargo
- Tauri CLI (installed via npm)
- Windows or Linux
	- Linux needs root/sudo or CAP_NET_RAW to send ICMP

Quick Start
```bash
# Install JS dependencies
npm install

# Run the app
npm run dev:windows # if you are using windows
npm run dev:linux:x11 # if you are using linux (x11)
npm run dev:linux:wayland # if you are using linux (wayland)

# Or build it get the binary file (or the exe file if you use windows)
npm run tauri:build
```

Linux Capabilities (optional alternative to sudo)
After building the binary, grant raw socket capability so it can send ICMP without sudo:
```bash
# Example path; adjust to your build output
sudo setcap cap_net_raw+ep ./src-tauri/target/release/nekrotrace
```

Project Structure
- `src/`: SvelteKit app (UI)
	- `src/lib/stores.js`: Global stores for hops and chart series
	- `src/routes/+page.svelte`: Listens to Tauri events and updates stores
	- `src/lib/components/`: `InputBar.svelte`, `HopList.svelte`, `Chart.svelte`
- `src-tauri/`: Rust backend (Tauri)
	- `src-tauri/src/network_engine.rs`: Traceroute + continuous ping + event emitters
	- `src-tauri/src/main.rs`: Tauri command wiring and bootstrap
	- `src-tauri/tauri.conf.json`: Tauri configuration

Notes
- This app emits two Tauri events:
	- `hop_list_updated`: complete list of discovered hops (once after traceroute)
	- `new_ping_data`: each ping result in real-time (ip, latency|null, status)
- On Windows, run as Administrator for raw ICMP.
- On Linux, run with `sudo` or use `setcap` as above.

# nekrotrace