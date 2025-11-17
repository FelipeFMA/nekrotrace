# NekroTrace

**High-performance network diagnostics with a beautiful real-time UI**

## Overview

NekroTrace is a desktop network diagnostic application that combines a Rust-based tracing and probing engine with a modern SvelteKit frontend. The backend performs traceroute discovery and then continuously cycles through the discovered hops, pinging each one sequentially with roughly one-second pauses between sweeps. Latency and status events stream to the frontend via Tauri event channels for real-time visualization.

NekroTrace is a polished, modern GUI front-end for traditional traceroute / tracert utilities — designed to make route discovery and per-hop latency inspection fast, intuitive, and visually clear. It is cross-platform and works on both Windows and Linux.

<img width="1606" height="1525" alt="image" src="https://github.com/user-attachments/assets/88329a34-564d-4235-9cbe-680a2642ff2c" />

## Table of Contents
- [Overview](#overview)
- [Key Features](#key-features)
- [Architecture](#architecture)
- [Event API](#event-api)
- [Data Model](#data-model)
- [Getting Started](#getting-started)
- [Development](#development)
- [Building & Distribution](#building--distribution)
- [Linux Capabilities & Permissions](#linux-capabilities--permissions)
- [Project Layout](#project-layout)
- [Contributing](#contributing)
- [License](#license)

## Key Features
- **Hybrid stack:** performant Rust backend (ICMP/UDP probing) with SvelteKit frontend for a smooth desktop UX via Tauri.
- **Traceroute discovery:** enumerates route hops (TTL, IP, reverse DNS when available).
- **Continuous per-hop probing:** the engine iterates over the hop list, pinging each hop sequentially to provide millisecond-granular latency samples.
- **Real-time streaming:** backend emits structured events (`hop_list_updated`, `new_ping_data`) consumed by the UI for live charts and animated lists.
- **Modern UI:** dark-themed, animated hop list and sparkline charts (ApexCharts) designed for readability and performance.

**Desktop GUI:** NekroTrace provides a visually-oriented UI for traceroute/tracert workflows — showing hop lists, per-hop charts, and live status updates in a single window.

**Cross-Platform:** Officially supported on **Windows** and **Linux** (native binaries for both platforms are available in releases).

## Architecture

- Backend: `src-tauri/src/` — Rust + Tauri. The core network engine lives in `src-tauri/src/network_engine.rs`. It runs traceroute, spawns per-hop ping loops, and emits events using the Tauri event API.
- Frontend: `src/` — SvelteKit app. The UI subscribes to Tauri events, keeps global state in `src/lib/stores.js`, and renders components from `src/lib/components/`.
- IPC: Tauri events are used to push data from Rust -> JS. This keeps the UI responsive and lets the engine run at native speed.

## Event API

The backend emits two primary events over Tauri's event system. The frontend expects these payloads:

- `hop_list_updated`
	- Emitted incrementally as traceroute results stream in, and once more when the discovery phase finishes.
	- Payload: array of hop objects:
		- `hop` (number): Time-To-Live value / hop index.
		- `ip` (string): IP address discovered at that hop.
		- `hostname` (string): reverse DNS result, or the IP string when PTR lookup fails.

- `new_ping_data`
	- Emitted after each ping attempt in the continuous probing loop.
	- Payload:
		- `ip` (string): target hop IP for this ping sample.
		- `latency` (number | null): measured round-trip time in milliseconds, or `null` when the attempt times out or errors.
		- `status` (string): one of `ok`, `timeout`, `error`, or `invalid_ip`.

`src/routes/+page.svelte` subscribes to these events, updates the shared stores in `src/lib/stores.js`, and re-renders the UI components in real time.

## Data Model

- Discovery phase: the engine performs a traceroute to the configured target, building a static list of hops.
- Probing phase: for each discovered hop, the engine runs a background ping loop (ICMP/UDP) and emits `new_ping_data` for every attempt.
- Retention & charting: the frontend keeps a sliding window of recent latencies per hop and renders them as time-series charts.

Pings are issued sequentially per hop. After each full sweep, the loop sleeps for one second, so an individual hop's sampling interval equals the sweep duration plus that pause.

## Getting Started

- Node.js 18 or later
- Rust toolchain (rustc + cargo)
- Tauri CLI (installed via npm: `npm i -g @tauri-apps/cli`) — required for building the desktop bundle
- On Linux: ensure the system `traceroute` and `ping` binaries are installed and runnable by your user (the app shells out to them instead of opening raw sockets itself).


## Releases / Prebuilt Binaries

You do not need to build from source to use NekroTrace. Download the latest prebuilt release for your platform from this repository's [Releases](https://github.com/FelipeFMA/nekrotrace/releases) page and run the provided binary:

- Windows: download the `.exe` from the latest release and run it normally.
- Linux: download the ELF binary for your architecture and run it. If your distribution removes the usual privileges from `ping`/`traceroute`, run NekroTrace with elevated permissions or adjust those utilities accordingly.

Using a release binary is the simplest way to try the application without installing the development toolchain.

## Development

Install JavaScript dependencies:

```bash
npm install
```

Run the app in development mode (select the command applicable to your platform):

```bash
npm run dev:windows        # Windows
npm run dev:linux:x11      # Linux (X11)
npm run dev:linux:wayland  # Linux (Wayland)
```

These scripts start the SvelteKit dev server and the Tauri dev runner so changes in both frontend and backend are live.

## Building & Distribution

Create a production build and package the desktop application using Tauri:

```bash
npm run build
npm run tauri:build
```

Output binaries are placed in the Tauri build output (e.g. `src-tauri/target/release/`).

## Linux Capabilities & Permissions

NekroTrace invokes the host system's `traceroute` and `ping` executables rather than opening raw sockets directly. On mainstream distributions those utilities already carry the necessary privileges (setuid root or `cap_net_raw`), so running NekroTrace as an unprivileged user typically works out of the box.

Only modify capabilities if your environment removes those defaults. Options include:

- Run NekroTrace with `sudo`.
- Restore the expected privileges on the system `ping`/`traceroute` binaries.
- As a last resort, grant `cap_net_raw` to the built NekroTrace binary:

```bash
sudo setcap cap_net_raw+ep ./src-tauri/target/release/nekrotrace
```

The final option is rarely necessary but remains available for hardened environments.

## Project Layout

- `src/` — SvelteKit frontend
	- `src/lib/stores.js` — central Svelte stores for hops and series
	- `src/lib/components/` — `Chart.svelte`, `HopList.svelte`, `InputBar.svelte`
	- `src/routes/` — Svelte pages and Tauri event wiring (`+page.svelte`, `+layout.svelte`)
- `src-tauri/` — Rust backend and Tauri config
	- `src-tauri/src/network_engine.rs` — traceroute orchestration, sequential ping loops, event emission
	- `src-tauri/src/main.rs` — Tauri command registration and bootstrap
	- `src-tauri/tauri.conf.json` — Tauri configuration for bundling

## Contributing

Contributions are welcome. Recommended workflow:

1. Fork the repository and create a feature branch.
2. Implement changes and add tests where appropriate.
3. Open a pull request with a clear description of the change and any relevant benchmarks or screenshots.

Development notes:
- Keep the probing engine efficient and avoid blocking operations on the main thread.
- Respect platform-specific raw socket and permission models.

## Security & Permissions

The application depends on the host `traceroute` and `ping` binaries, so it inherits whatever security posture those tools already have on your system. In a standard setup you can run NekroTrace as a regular user and rely on the system utilities' existing capabilities. If you harden or replace those tools, adjust their permissions or run NekroTrace with elevated privileges so they can continue to operate. Never grant additional capabilities to untrusted binaries.

## Acknowledgements

Built with Tauri, SvelteKit, Rust, and ApexCharts.

## License

This project is distributed under the terms of the repository `LICENSE` file.
