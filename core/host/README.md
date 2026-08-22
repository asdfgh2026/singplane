# singpanel-host

Desktop HTTP shell around [`../lib`](../lib) (`singpanel-core`).
The GPUI desktop shell starts this process and reads one stdout line.

Do not add assemble/engine logic here — put it in `singpanel-core`.

```
READY port=12345 token=…
```

Then calls loopback HTTP with `Authorization: Bearer <token>`.

| Method | Path | Purpose |
|--------|------|---------|
| GET | `/v1/ping` | Health |
| GET | `/v1/status` | `{ ok, running, viaHelper, pid }` |
| POST | `/v1/start` | `{ corePath, configPath, requireHelper }` |
| POST | `/v1/stop` | Stop core (not the host) |
| POST | `/v1/check` | `{ corePath, content }` — `sing-box check` |
| POST | `/v1/fetch` | `{ url }` — download subscription as-is |
| POST | `/v1/convert` | Clash/URI → outbounds (owns convert sidecar) |
| POST | `/v1/assemble` | detect + optional convert + template merge |
| POST | `/v1/clash` | `{ baseUrl, secret, method, path, query, body }` |

Start prefers the existing **singpanel-helper** service when `ctl ping` works;
otherwise it runs official `sing-box run -c` as the current user.

```bash
cargo test
cargo build --release
# Windows: target/release/singpanel-host.exe
```
