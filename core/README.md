# 控制面 / 辅助层

## host/ — Rust 控制面（启停官方 sing-box）

见 [host/README.md](host/README.md)。

桌面 GPUI 把 start/stop/status 交给 `singpanel-host`（本机 `127.0.0.1` + Bearer）。

```bash
cd core/host
cargo build --release
```

## convert/ — Clash → outbounds 本地服务

见 [convert/README.md](convert/README.md)。

```bash
cd core/convert
go mod tidy
go build -o singpanel-convert.exe .   # Windows
```

桌面会在装配 Clash/URI 订阅时启动该进程（仅 `127.0.0.1`）。

## 主内核

官方 `sing-box` 仍是外部进程；由 host 托管，不改内核。

长期可选：FFI 嵌入 `libbox` / 去掉 convert 常驻进程。
