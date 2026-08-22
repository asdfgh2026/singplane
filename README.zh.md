# SingPanel

[English](README.md) | [简体中文](README.zh.md)

> **早期开发阶段。** 接口、界面和安装包都可能随时改动，功能和稳定性都不完整，使用时请自行承担风险。

[下载](https://github.com/asdfgh2026/singplane/releases/latest) · [全部版本](https://github.com/asdfgh2026/singplane/releases)

基于 **[sing-box](https://github.com/SagerNet/sing-box)** 打造的原生图形客户端。

桌面端用 **Rust + [GPUI](https://www.gpui.rs/)**，内存占用极低。界面支持 Clash 风格的节点选择、策略组、连接监控与规则配置，开箱即用。首页内置 Tailscale 开关，无需修改订阅，即可实现代理与 tailnet 局域网同时在线。

安卓端基于 **Jetpack Compose** 原生开发（`mobile/`）。

## 特点

- **更轻量省电**：桌面端采用 GPUI 原生绘制，非 Electron / WebView 架构。
- **开箱即用**：支持 Clash 风格分组与节点测速，无需手写复杂规则。
- **Tailscale 联动**：主界面一键启停 Tailscale，代理与私有组网互不干扰。
- **原生支持**：完整支持 sing-box 协议特性与规则配置。

## 技术栈

- **[GPUI](https://www.gpui.rs/)**：Zed 编辑器同款的 Rust 原生 UI 框架，负责桌面端渲染。组件库来自 [GPUI Component](https://github.com/longbridge/gpui-component)。
- **[Jetpack Compose](https://developer.android.com/jetpack/compose)**：Android 官方声明式 UI 工具包。

## 安装

到 **[Releases](https://github.com/asdfgh2026/singplane/releases/latest)** 下载对应平台的安装包：

| 平台 | 文件 |
|------|------|
| Windows | `SingPanel-<ver>-windows-x64.zip` |
| macOS | `SingPanel-<ver>-macos-arm64.dmg` / `.zip` |
| Android | `SingPanel-<ver>-android-*.apk` |

首次使用：设置页 → **下载内核** → 导入配置 → 首页启动。

## 从源码编译

### 桌面（Windows / macOS）

```bash
cd core/host && cargo build
cd desktop && cargo run
```

Windows：

```powershell
cd core\host; cargo build
cd desktop; cargo run
```

macOS 打包：

```bash
cargo build --release --manifest-path core/host/Cargo.toml
cargo build --release --manifest-path desktop/Cargo.toml
./desktop/scripts/package-macos-app.sh
```

### Android

```bash
# 编译 libbox.aar 并打包 APK
./scripts/build_android.sh               # 默认 v1.13.19
./scripts/build_android.sh v1.13.19      # 指定 sing-box 版本
./scripts/build_android.sh --release     # 构建 Release 包

# 或仅编译 APK
cd mobile
./gradlew :app:assembleDebug
# 产物: mobile/app/build/outputs/apk/debug/app-debug.apk
```

## 发布

命令行发布用 `./scripts/release.sh`：

```bash
git checkout main
git pull
./scripts/release.sh          # 默认升级 patch: 0.0.1 → 0.0.2
# ./scripts/release.sh minor  # 0.0.2 → 0.1.0
# ./scripts/release.sh major  # 0.1.0 → 1.0.0
```

## 常见问题

### 1. 安装与使用须知

- 安卓端请授予 VPN、通知与后台运行权限，系统要求 **Android 8.0** 及以上。
- 首次使用请先在设置页下载或指定 [sing-box](https://github.com/SagerNet/sing-box/releases)。
- 安全与隐私：项目基于 GPL-3.0 开源，所有配置与数据均保存在本地，不收集任何隐私信息。

### 2. 桌面端常见问题

- **Windows 管理员权限：** 开启 TUN 虚拟网卡需要安装一次系统辅助服务。安装后客户端以普通权限运行，无需每次以管理员身份启动。
- **TUN 模式授权：** macOS / Linux 会提示输入管理员密码以授权内核虚拟网卡权限。
- **端口冲突：** 客户端默认单实例运行。如提示端口占用，请检查是否有其他代理软件或 sing-box 进程正在运行。
- **Tailscale 登录：** 开启开关后如提示等待授权，点击并在浏览器完成登录后即可自动连通。

### 3. macOS 安装提示

- 下载后打开 `.dmg`，将 **SingPanel** 拖入「应用程序」。
- **首次运行提示未签名或拦截**：在「应用程序」中**右键** SingPanel → 点击「打开」，并在弹窗中再次确认「打开」即可。
- 若提示「已损坏」：在终端执行 `xattr -d com.apple.quarantine /Applications/SingPanel.app` 后重新打开。

### 4. 导入配置

- 支持直接导入 sing-box JSON 配置，或粘贴 Clash 订阅链接（客户端将自动转换）。
- 导入配置后，在配置列表选择并设为当前配置，返回首页点击启动即可。

### 5. 安卓端提示

- 开启连接时请允许系统弹出的 VPN 连接请求。
- 如后台频繁断连，请在系统设置中为 SingPanel 关闭电池优化。

### 6. 待持续完善补充

问题仍在，请到仓库提交 [Issue](https://github.com/asdfgh2026/singplane/issues)。

## 许可证

GPL-3.0
