# SingPanel GPUI 壳

桌面壳，只用 [GPUI Component](https://github.com/longbridge/gpui-component)。  
不要改安卓产品树 `mobile/`。

控制面逻辑在 `core/lib`（`singpanel-core`）。桌面进程仍是 `core/host`（HTTP 薄壳）。安卓 UI 在 [`../mobile/`](../mobile/README.md)（Compose）。

```text
desktop/          ← 这个壳
mobile/           ← Compose 安卓，别往这里塞
core/host/        ← Rust 控制面
```

## 运行

先编 host（若还没有），再开壳。到处都能用 Cargo：

```bash
cd core/host && cargo build
cd desktop && cargo run
```

Windows：

```powershell
cd core\host; cargo build
cd desktop; cargo run
```

窗口里可以看状态、启动/停止官方 sing-box（读 `%APPDATA%\SingPanel\SingPanel` 里已有内核和 `config.runtime.json`）。

Windows 图标在 `assets/app_icon.ico`，由 `build.rs` 打进 exe。

### Cinder（可选，加速开发编译）

[Cinder](https://github.com/CapSoftware/cinder) 是 Cargo 的替代入口：参数、workspace、lockfile、feature 仍以 Cargo 为准。

| 平台 | 行为 |
|------|------|
| **macOS** | 在能证明结果仍有效时走加速路径（`build` / `check` / `run`、部分 `test`）。本机开发建议用这个。 |
| **Linux / Windows** | 命令能跑，但**不做产物加速**，全部原样交给 Cargo。 |

`--release`、它不认识的命令、第一次见到的结构改动，也会回落到 Cargo。

没有预编译包，需 Rust 1.85+ 自行编译后放进 `PATH`：

```bash
git clone https://github.com/CapSoftware/cinder
cd cinder && cargo build --release
# 二进制：target/release/cinder
```

装好后把上面的 `cargo` 换成 `cinder` 即可，例如：

```bash
cd desktop && cinder run
cinder check
cinder stats          # 要统计先设 CINDER_USAGE=1
```

没装 Cinder 时继续用 `cargo`，不要改清单或 lockfile。

## 文件（一个 tab 一个文件）

| 文件 | Tab |
|------|-----|
| `src/pages/home.rs` | 首页 |
| `src/pages/proxies.rs` | 代理 |
| `src/pages/profiles.rs` | 配置 |
| `src/pages/templates.rs` | 模板 |
| `src/pages/logs.rs` | 日志 |
| `src/pages/settings.rs` | 设置 |

导航在 `src/app.rs`（左侧栏 + 右侧内容）。主色是 `#047857`（`themes/singpanel.json`），启动时替换 shadcn 默认灰阶。host 客户端在 `src/host.rs`。各 tab **只改自己的文件**，共用 `Arc<HostClient>`。

共享（tab 任务不要改）：

| 文件 | 职责 |
|------|------|
| `src/host.rs` | start/stop/status/check/fetch/convert/assemble/clash、内核日志路径 |
| `src/store.rs` | 本机设置、配置、模板 |
| `src/net.rs` | 内网 IPv4（跳过 vEthernet / fake-ip / Tailscale） |
| `src/widgets.rs` | `page_frame` / `page_scroll` / `card` / `section_header` |
| `src/core_download.rs` | 官方 GitHub Releases 下内核（设置页按钮） |
| `src/tailscale.rs` | 应用级 Tailscale 注入（endpoint / MagicDNS / 路由），不改订阅文件；官方 ≥1.13，1.14 字段按版本写入 |

## 测试夹具

固定配置给后续启动 / Clash API / 首页模式验收用，见 [docs/test-fixtures.md](docs/test-fixtures.md)。

当前夹具：[`examples/http-proxy.json`](examples/http-proxy.json) — 本机 HTTP 入站 `:7890` + Clash API `:9090`。
