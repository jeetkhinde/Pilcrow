# WebSocket Checklist

**Prerequisites:** Before starting, run a local application with WebSocket integration (e.g., `cd examples/chat && cargo run`) and open your browser's developer tools to monitor the Network (WebSocket tab) and Console.

**JS tests:** Manual verification checklist for when you wire up a real handler:

- [ ] `s-live="ws:/ws/path"` → WebSocket opens, `silcrow:live:connect` fires
- [ ] Server sends `{"type":"patch","target":"#el","data":{...}}` → DOM updates
- [ ] Server sends `{"type":"html","target":"#el","markup":"<p>hi</p>"}` → safeSetHTML applied
- [ ] Server sends `{"type":"invalidate","target":"#el"}` → binding cache dropped
- [ ] Server sends `{"type":"navigate","path":"/foo"}` → client navigates
- [ ] Server sends `{"type":"custom","event":"refresh","data":{}}` → `silcrow:ws:refresh` event fires
- [ ] `Silcrow.send("#el", {type:"custom",event:"ping",data:{}})` → message arrives at server
- [ ] Connection drop → `silcrow:live:disconnect` fires, reconnects with backoff
- [ ] Element removed from DOM → MutationObserver closes WebSocket
- [ ] `Silcrow.disconnect("#el")` → WebSocket closes
- [ ] `Silcrow.reconnect("#el")` → WebSocket reopens

## Multiplex Verification

- [ ] Two elements with `s-live="ws:/same/path"` → ONE WebSocket in Network tab
- [ ] Two elements with different WS URLs → TWO WebSockets
- [ ] Remove one element from DOM → socket stays open for remaining subscriber
- [ ] Remove last element from DOM → socket closes
- [ ] `Silcrow.send("/ws/path", data)` sends one frame, not N
- [ ] `Silcrow.disconnect("#one")` keeps socket alive if `#two` subscribed to same URL
- [ ] `Silcrow.disconnect("/ws/path")` pauses hub, all subscribers stop
- [ ] `Silcrow.reconnect("/ws/path")` resumes hub for all subscribers
- [ ] `Silcrow.destroy()` closes all hubs, no leaked sockets