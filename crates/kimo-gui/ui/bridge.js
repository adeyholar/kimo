// Event bridge: ONE ingestion path (onEvent) behind a thin transport
// abstraction. Bindings, in priority order:
//   1. Tauri IPC     inside the shell: events arrive from the loopback
//                    TCP/JSONL listener in src/main.rs (`engine-event`).
//   2. localhost SSE plain-browser development (tools/sse_feed.py).
//                    Forced with ?feed=sse, custom port with ?sse=PORT.
//   3. simulated     ?feed=sim, or nothing reachable: the demo replay from
//                    ../demo (dev checkout only; it is not part of the shipped
//                    ui/ bundle). Simulated data always labels itself.
// The app makes no network calls beyond loopback.

export function connect(onEvent, onStatus) {
  if (window.__TAURI__) {
    window.__TAURI__.event.listen('engine-event', e => onEvent(e.payload));
    window.__TAURI__.event.listen('spectator-status', e => onStatus(String(e.payload) === 'live' ? 'live' : 'down', String(e.payload)));
    onStatus('down', 'listening');
    return { kind: 'tauri' };
  }
  const params = new URLSearchParams(location.search);
  const forced = params.get('feed');
  const sim = () => { onStatus('sim', 'simulated'); startSimulated(onEvent, params, onStatus); return { kind: 'sim' }; };
  if (forced === 'sim') return sim();

  const port = params.get('sse') || '7422';
  const es = new EventSource(`http://127.0.0.1:${port}/events`);
  let opened = false;
  es.onopen = () => { opened = true; onStatus('live', `sse :${port}`); };
  es.onmessage = e => { try { onEvent(JSON.parse(e.data)); } catch {} };
  es.onerror = () => {
    if (!opened) {
      es.close();
      if (forced === 'sse') onStatus('down', `sse :${port} refused`);
      else sim();
    } else onStatus('down', 'sse closed');
  };
  return { kind: 'sse', port };
}

async function startSimulated(onEvent, params, onStatus) {
  try {
    const mod = await import('../demo/replay.js');
    mod.play(onEvent, params);
  } catch (err) {
    onStatus('down', 'no feed');
  }
}
