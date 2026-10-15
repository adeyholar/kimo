// Event bridge: ONE ingestion path (onEvent) behind a thin transport
// abstraction. Bindings, in priority order:
//   1. Tauri IPC   - inside the shell: events arrive from the loopback
//                    TCP/JSONL listener in src/main.rs.
//   2. localhost SSE - plain-browser development: a zero-dependency local
//                    stream (tools/sse_feed.py). Forced with ?feed=sse,
//                    custom port with ?sse=PORT.
//   3. simulated   - no shell and no SSE reachable: the built-in mockup-A
//                    replay. Every non-Tauri binding labels itself in the
//                    status pill; simulated data is never silent.
// The SSE binding is a development transport on 127.0.0.1 only - the Tauri
// shell path never uses it, and the app makes no other network calls.

export function connect(onEvent, onStatus, startSimulated) {
  if (window.__TAURI__) {
    window.__TAURI__.event.listen('engine-event', e => onEvent(e.payload));
    window.__TAURI__.event.listen('spectator-status', e => onStatus(e.payload));
    return { kind: 'tauri' };
  }
  const params = new URLSearchParams(location.search);
  const forced = params.get('feed');
  if (forced === 'sim') { startSimulated(); return { kind: 'sim' }; }

  const port = params.get('sse') || '7422';
  const es = new EventSource(`http://127.0.0.1:${port}/events`);
  let opened = false;
  es.onopen = () => { opened = true; onStatus('live', `sse loopback :${port}`); };
  es.onmessage = e => { try { onEvent(JSON.parse(e.data)); } catch {} };
  es.onerror = () => {
    if (!opened) {
      es.close();
      if (forced === 'sse') { onStatus('down', `sse loopback :${port} refused`); }
      else { startSimulated(); onStatus('sim', null); }
    } else onStatus('down', 'sse stream closed');
  };
  return { kind: 'sse', port };
}
