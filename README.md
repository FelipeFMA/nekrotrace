# NekroTrace

**High-performance network diagnostics with a beautiful real-time UI**

## Overview

NekroTrace is a desktop network diagnostic application that combines a Rust-based tracing and probing engine with a modern SvelteKit frontend. The backend performs traceroute discovery and then continuously pings each discovered hop at a fixed 1-second interval, streaming per-hop latency and status events to the frontend via Tauri event channels for real-time visualization.

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
- [Linux Capabilities](#linux-capabilities)
- [Project Layout](#project-layout)
- [Contributing](#contributing)
- [License](#license)

## Key Features
- **Hybrid stack:** performant Rust backend (ICMP/UDP probing) with SvelteKit frontend for a smooth desktop UX via Tauri.
- **Traceroute discovery:** enumerates route hops (TTL, IP, reverse DNS when available, and AS metadata if present).
- **Continuous per-hop probing:** each hop is polled at short intervals to provide millisecond-granular latency series.
- **Real-time streaming:** backend emits structured events (`hop_list_updated`, `new_ping_data`) consumed by the UI for live charts and animated lists.
- **Modern UI:** dark-themed, animated hop list and sparkline charts (ApexCharts) designed for readability and performance.

**Desktop GUI:** NekroTrace provides a visually-oriented UI for traceroute/tracert workflows — showing hop lists, per-hop charts, and live status updates in a single window.

**Cross-Platform:** Officially supported on **Windows** and **Linux** (native binaries for both platforms are available in releases).

## Architecture

- Backend: `src-tauri/src/` — Rust + Tauri. The core network engine lives in `src-tauri/src/network_engine.rs`. It runs traceroute, spawns per-hop ping loops, and emits events using the Tauri event API.
- Frontend: `src/` — SvelteKit app. The UI subscribes to Tauri events, keeps global state in `src/lib/stores.js`, and renders components from `src/lib/components/`.
- IPC: Tauri events are used to push data from Rust -> JS. This keeps the UI responsive and lets the engine run at native speed.

## Event API

The backend emits two primary events over Tauri's event system. The frontend expects these payload shapes:

- `hop_list_updated`
	- Emitted once after a traceroute completes (or when the hop list changes).
	- Payload: array of hop objects:
		- `ttl` (number): Time-To-Live value / hop index.
		- `ip` (string | null): IP address discovered at that hop, or null if unresolved.
		- `hostname` (string | null): Reverse DNS name when available.
		- `protocol` (string): probe protocol used (e.g. `icmp`, `udp`).
		- `metadata` (object | null): optional data such as AS, geo, etc.

- `new_ping_data`
	- Emitted continuously for each ping probe result.
	- Payload:
		- `ip` (string): target hop IP for this ping sample.
		- `latency_ms` (number | null): measured round-trip time in milliseconds, or `null` if timed out.
		- `status` (string): e.g. `ok`, `timeout`, `error`.
		- `timestamp` (ISO 8601 string): sample timestamp.

These events are consumed by `src/routes/+page.svelte` which updates Svelte stores in `src/lib/stores.js` and updates the UI components.

## Data Model

- Discovery phase: the engine performs a traceroute to the configured target, building a static list of hops.
- Probing phase: for each discovered hop, the engine runs a background ping loop (ICMP/UDP) and emits `new_ping_data` for every attempt.
- Retention & charting: the frontend keeps a sliding window of recent latencies per hop and renders them as time-series charts.

## Getting Started

- Node.js 18 or later
- Rust toolchain (rustc + cargo)
- Tauri CLI (installed via npm: `npm i -g @tauri-apps/cli`) — required for building the desktop bundle
- On Linux: either run with elevated privileges (sudo) or grant `CAP_NET_RAW` to the built binary to allow raw ICMP without root.


## Releases / Prebuilt Binaries

You do not need to build from source to use NekroTrace. Download the latest prebuilt release for your platform from this repository's [Releases](https://github.com/FelipeFMA/nekrotrace/releases) page and run the provided binary:

- Windows: download the `.exe` from the latest release and run it normally.
- Linux: download the ELF binary for your architecture and run it (grant `CAP_NET_RAW` if you prefer not to use `sudo`).

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

## Linux Capabilities

To run the built binary without `sudo`, grant the raw socket capability so the binary can send ICMP packets:

```bash
# Example path — adjust to your build output
sudo setcap cap_net_raw+ep ./src-tauri/target/release/nekrotrace
```

If you choose not to set capabilities, run the app with elevated privileges when probing network hops:

```bash
sudo ./src-tauri/target/release/nekrotrace
```

## Project Layout

- `src/` — SvelteKit frontend
	- `src/lib/stores.js` — central Svelte stores for hops and series
	- `src/lib/components/` — `Chart.svelte`, `HopList.svelte`, `InputBar.svelte`
	- `src/routes/` — Svelte pages and Tauri event wiring (`+page.svelte`, `+layout.svelte`)
- `src-tauri/` — Rust backend and Tauri config
	- `src-tauri/src/network_engine.rs` — traceroute, ping loops, event emission
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

The application requires network probing privileges to send/receive ICMP or raw UDP responses. Granting `CAP_NET_RAW` to the binary is a more secure alternative to running the entire app as root. Do not run untrusted binaries with elevated privileges.

## Acknowledgements

Built with Tauri, SvelteKit, Rust, and ApexCharts.

## License

This project is distributed under the terms of the repository `LICENSE` file.
