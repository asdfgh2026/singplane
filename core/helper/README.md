# singpanel-helper

Elevated **Windows service** that starts/stops the official `sing-box` binary.

## Why

Creating a TUN adapter needs administrator rights. Instead of elevating the GUI or prompting UAC on every start/stop:

1. User installs this service **once** (one UAC).
2. Service runs as **LocalSystem** and listens on a named pipe.
3. SingPanel (normal user) calls `ctl` over the pipe to start/stop the core.

## Build

```bash
# Build for Windows
cd core/helper
cargo build --release --target x86_64-pc-windows-msvc
```

Copy `target/x86_64-pc-windows-msvc/release/singpanel-helper.exe` next to the app, or keep under `core/helper/` for dev discovery.

## Commands

| Command | Who | Purpose |
|---------|-----|---------|
| `install` | Admin (UAC) | Create + start `SingPanelHelper` service, write token |
| `uninstall` | Admin | Stop + delete service |
| `service` | SCM | Service entry (automatic) |
| `run` | Dev | Foreground pipe server |
| `ctl ping` | User | Health check |
| `ctl start --core … --config …` | User | Start sing-box via service |
| `ctl stop` | User | Stop core |
| `ctl status` | User | Running / PID |

## Token

`%ProgramData%\SingPanel\helper.token` — generated on first install only. Re-running `install` when the service already exists starts it and does **not** rotate the token.

ACL is SYSTEM + installing user (not world-readable). The pipe is bound to that user SID when `helper.owner` exists.

`core.start` ignores caller argv: it only runs `sing-box.exe run -c <config>` and both paths must be local files under the install-time allow list (`helper.allow`).

桌面在 helper 可用时走服务启停；没有 helper 时，开 TUN 会提示先安装。
