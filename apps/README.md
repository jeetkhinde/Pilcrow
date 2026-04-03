# Apps

Demo apps moved to [`sandbox/apps`](../sandbox/apps).

Use the sandbox workspace as the canonical consumer:

```bash
cargo install --path tools/cli --force
cd sandbox
cargo run -p pilcrow-backend
cargo run -p web
pilcrow-cli check-arch
```

Runtime host/port and backend URL are configured in [`sandbox/Pilcrow.toml`](../sandbox/Pilcrow.toml).
