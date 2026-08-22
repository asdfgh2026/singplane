# SingPanel

[English](README.md) | [简体中文](README.zh.md)

> **Early development.** APIs, UI, and packaging may change without notice. Expect bugs and incomplete features.

[Download](https://github.com/asdfgh2026/singplane/releases/latest) · [All releases](https://github.com/asdfgh2026/singplane/releases)

A native GUI client powered by **[sing-box](https://github.com/SagerNet/sing-box)**.

The desktop app is built with **Rust + [GPUI](https://www.gpui.rs/)** for minimal memory usage. The interface provides Clash-style group selection, latency testing, connection tracking, and rule routing out of the box. Tailscale can be toggled directly from the Home screen without editing subscription files.

Android is natively built using **Jetpack Compose** (`mobile/`).

## Highlights

- **Lightweight & Fast**: Native GPUI rendering on desktop, avoiding Electron or WebView overhead.
- **Ready Out of the Box**: Clash-style grouping and node switching without complex manual config.
- **Integrated Tailscale**: One-click Tailscale toggle on Home, running proxy and private tailnet concurrently.
- **Native sing-box Support**: Full compatibility with sing-box features and routing syntax.

## Stack

- **[GPUI](https://www.gpui.rs/)**: Rust native UI framework by Zed editor. Widgets powered by [GPUI Component](https://github.com/longbridge/gpui-component).
- **[Jetpack Compose](https://developer.android.com/jetpack/compose)**：Android official declarative UI toolkit.

## Install

Download the installer or package for your platform from **[Releases](https://github.com/asdfgh2026/singplane/releases/latest)**:

| Platform | File |
|----------|------|
| Windows | `SingPanel-<ver>-windows-x64.zip` |
| macOS | `SingPanel-<ver>-macos-arm64.dmg` / `.zip` |
| Android | `SingPanel-<ver>-android-*.apk` |

First-time use: Settings → **download core** → import profile → start on Home.

## Build from source

### Desktop (Windows / macOS)

```bash
cd core/host && cargo build
cd desktop && cargo run
```

Windows:

```powershell
cd core\host; cargo build
cd desktop; cargo run
```

macOS packaging:

```bash
cargo build --release --manifest-path core/host/Cargo.toml
cargo build --release --manifest-path desktop/Cargo.toml
./desktop/scripts/package-macos-app.sh
```

### Android

```bash
# Build libbox.aar and assemble an APK
./scripts/build_android.sh               # default v1.13.19
./scripts/build_android.sh v1.13.19      # specify sing-box version
./scripts/build_android.sh --release     # release APK

# Or assemble the APK only
cd mobile
./gradlew :app:assembleDebug
# output: mobile/app/build/outputs/apk/debug/app-debug.apk
```

## Release

Run `./scripts/release.sh` from the command line:

```bash
git checkout main
git pull
./scripts/release.sh          # bump patch: 0.0.1 → 0.0.2
# ./scripts/release.sh minor  # 0.0.2 → 0.1.0
# ./scripts/release.sh major  # 0.1.0 → 1.0.0
```

## FAQ

### 1. General Notes

- On Android, grant VPN, notification, and background permissions (requires **Android 8.0+**).
- First-time setup: download or specify [sing-box](https://github.com/SagerNet/sing-box/releases) in Settings.
- Security & Privacy: Open source under GPL-3.0. All configurations and network data remain strictly on your local device.

### 2. Desktop FAQ

- **Windows Admin Rights:** Enabling TUN virtual network mode requires installing a helper service once. Afterwards, the app runs with standard user privileges.
- **TUN Permissions:** macOS / Linux will prompt for administrator password to grant virtual interface permissions.
- **Port Conflicts:** The client runs as a single instance. If port conflicts occur, ensure no other proxy tools or sing-box instances are running.
- **Tailscale Auth:** If it shows waiting for authorization after toggling on, click to log in via browser to connect.

### 3. macOS Installation Notes

- After downloading `.dmg`, drag **SingPanel** to Applications.
- **First Launch & Security Notice**: Right-click **SingPanel** in Applications → click "Open", then confirm "Open" in the system dialog.
- If macOS displays "App is damaged", run `xattr -d com.apple.quarantine /Applications/SingPanel.app` in Terminal.

### 4. Importing Profiles

- Directly import sing-box JSON profiles, or paste Clash subscription links (automatically converted).
- Select and set as current profile, then return to Home and click Start.

### 5. Android Notes

- Grant system VPN permission when starting the connection.
- If connection drops frequently in background, disable battery optimization for SingPanel in system settings.

### 6. Still stuck

Open an [Issue](https://github.com/asdfgh2026/singplane/issues).

## License

GPL-3.0
