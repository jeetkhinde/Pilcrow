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
