import { invoke as tauriInvoke } from '@tauri-apps/api/core';
import { listen as tauriListen } from '@tauri-apps/api/event';

const isTauri = typeof window !== 'undefined' && window.__TAURI_INTERNALS__ !== undefined;

let ws = null;
const listeners = new Map();

function connectWs() {
    if (typeof window === 'undefined') return;
    if (ws && (ws.readyState === WebSocket.OPEN || ws.readyState === WebSocket.CONNECTING)) return;

    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
    ws = new WebSocket(`${protocol}//${window.location.host}/ws`);

    ws.onmessage = (event) => {
        try {
            const msg = JSON.parse(event.data);
            if (msg.event && listeners.has(msg.event)) {
                listeners.get(msg.event).forEach(cb => cb({ payload: msg.payload }));
            }
        } catch (e) {
            console.error('WS parse error', e);
        }
    };

    ws.onclose = () => {
        setTimeout(connectWs, 1000);
    };
}

if (!isTauri) {
    // Initialize WS connection on load for web
    if (typeof window !== 'undefined') {
        connectWs();
    }
}

export async function invoke(cmd, args = {}) {
    if (isTauri) {
        return tauriInvoke(cmd, args);
    } else {
        // Web implementation
        if (cmd === 'start_trace') {
            return fetch('/api/start', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify(args)
            });
        } else if (cmd === 'stop_trace') {
            return fetch('/api/stop', { method: 'POST' });
        }
    }
}

export async function listen(event, callback) {
    if (isTauri) {
        return tauriListen(event, callback);
    } else {
        // Web implementation
        if (!listeners.has(event)) {
            listeners.set(event, new Set());
        }
        listeners.get(event).add(callback);

        // Return unlisten function
        return () => {
            const set = listeners.get(event);
            if (set) {
                set.delete(callback);
            }
        };
    }
}
