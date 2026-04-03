# Apps

Demo apps moved to [`sandbox/apps`](../sandbox/apps).

Use the sandbox workspace as the canonical consumer:

```bash
cd sandbox
cargo run -p pilcrow-backend
cargo run -p web
cargo run -p pilcrow-cli -- check-arch
```
