# GPUI / host 测试夹具

后续验收、回归、截图用的固定配置。不要改成「产品默认」；要改语义就另开一份文件。

## 索引

| 夹具 | 路径 | 用来测什么 |
|------|------|------------|
| 本机 HTTP 代理 | [`desktop/examples/http-proxy.json`](../examples/http-proxy.json) | 官方 sing-box 启动；HTTP 入站 `127.0.0.1:7890`；Clash API `127.0.0.1:9090`；首页代理模式 Rule / Global / Direct；日志页有输出。夹具本身不含 Tailscale；开启后 GPUI 会按已装内核注入（1.13 用 `ip_accept_any`，≥1.14 用 `preferred_by`） |

内核仍走官方 `SagerNet/sing-box`，装到 AppData `cores/`（Windows：`%APPDATA%\SingPanel\SingPanel\cores\sing-box.exe`；Linux：`$APPDATA/SingPanel/SingPanel/cores/sing-box`，orb 里 `APPDATA` 常用 `~/.local/share`）。

## `http-proxy.json`

完整可运行的 sing-box 1.13+ 配置（新 DNS `type`/`server`，不是已废弃的 `address`）。

- 入站：`http`，`127.0.0.1:7890`，无认证
- 出站：只有 `direct`（本机直连，不是订阅节点）
- Clash API：`127.0.0.1:9090`，`secret` 空，`default_mode` = `Rule`
- 路由里写了 `clash_mode` = `Direct` / `Global`，这样 `PATCH /configs {"mode":…}` 的 `mode-list` 才是 `Rule, Global, Direct`。只有 `Rule` 时 API 会 204 但模式不变

### 装成当前运行时

把文件拷到 AppData 并写成当前配置（GPUI 首页电源开关读的是 `runtime/config.runtime.json`）：

```bash
# Linux / orb
export APPDATA="${APPDATA:-$HOME/.local/share}"
ROOT="$APPDATA/SingPanel/SingPanel"
mkdir -p "$ROOT/runtime" "$ROOT/profiles"
cp desktop/examples/http-proxy.json "$ROOT/runtime/config.runtime.json"
```

或在 GPUI **配置** 页：导入本地路径 → 选这份 JSON → **设为当前**。

校验 + 直接跑内核（不经过壳）：

```bash
"$ROOT/cores/sing-box" check -c desktop/examples/http-proxy.json
"$ROOT/cores/sing-box" run -c "$ROOT/runtime/config.runtime.json"
```

应看到：

```
inbound/http[http-in]: tcp server started at 127.0.0.1:7890
clash-api: restful api listening at 127.0.0.1:9090
```

```bash
curl -sS http://127.0.0.1:9090/version
curl -sS http://127.0.0.1:9090/configs   # mode / mode-list
curl -x http://127.0.0.1:7890 https://example.com
```

### 已知限制

- 没有 Selector 组时，**代理**页只有空态；模式切换看首页「代理模式」。
- `block` / `dns` outbound 和旧 DNS `address` 字段在 1.13 上会 check 失败或启动被拒。
- Tailscale：首页开关 / 设置页字段会写入 prefs，启动时由 `desktop/src/tailscale.rs` 按已装内核版本注入。最低兼容 **1.13**（DNS 用 `ip_accept_any`）；`preferred_by` / `accept_search_domain` 是 1.14。单元测试：`cargo test --bin singpanel-gpui tailscale`。
