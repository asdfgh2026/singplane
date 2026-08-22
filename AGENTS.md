# SingPanel — Architecture & Contributor Notes

This repo is **SingPanel**: a modern cross-platform GUI for the **official SagerNet sing-box**.
It is not a fork of sing-box. UI interaction follows FlClash.

Do not mix desktop and Android trees (binaries, folders, or launch commands).

## Versions & Project Paths

| Version | Role | Tree | Launch / Build | Notes |
|---------|------|------|----------------|-------|
| **GPUI / Rust (Desktop)** | Windows / macOS desktop shell. Lightweight native UI. | `desktop/` | `cd desktop && cargo run`<br>Binary: `desktop/target/debug/singpanel-gpui` | Do not edit `mobile/` for desktop work. |
| **Compose (Android)** | Native Android UI (Jetpack Compose) + embedded official `libbox.aar`. | `mobile/` | `./scripts/build_android.sh`<br>Or in `mobile/`: `./gradlew :app:assembleDebug` | Do not edit `desktop/` for Android work. |
| **singpanel-core** | Shared Rust control plane (assemble / fetch / convert / check / engine). | `core/lib/` | `cd core/lib && cargo test` | Pure control plane logic. |
| **singpanel-host** | Desktop HTTP shell over singpanel-core. | `core/host/` | Spawned by desktop GPUI (`READY port=... token=...` loopback HTTP + Bearer). | Loopback control interface. |
| **singpanel-helper** | Windows SYSTEM helper (TUN / named pipe). | `core/helper/` | Windows background service. | Strictly enforces path ACL & token authentication. |
| **sing-box** | Official data plane. | Desktop: `cores/sing-box`<br>Android: embedded `libbox.aar` | Desktop uses official executable; Android uses `VpnService` + official JNI. | Never vendor or rewrite the kernel. |

---

## Architecture Overview

```
┌───────────────────────────┐     ┌───────────────────────────┐
│ GPUI Desktop              │     │ Compose Android           │
│ desktop/                  │     │ mobile/                   │
│ singpanel-gpui            │     │ pages + SingPanelVpnService│
└─────────────┬─────────────┘     └─────────────┬─────────────┘
              │ loopback HTTP                   │ embedded JNI
              ▼                                 ▼
        singpanel-host                    LibboxCoreProcess
              │                                 │
              ▼                                 │
        singpanel-core (`core/lib`)             │
              │                                 │
              ├───────────┬───────────┐         ▼
              ▼           ▼           ▼    official sing-box
        sing-box(exe)  convert     helper  (VpnService + tun fd)
```

- **Control Plane**: Rust (`core/lib`, `core/host`) for desktop; Kotlin (`mobile/app/.../core`) for Android.
- **Data Plane**: Official sing-box binary / `libbox.aar` only.
- **Desktop UI**: Rust GPUI Component.
- **Android UI**: Android Jetpack Compose.

---

## Workspace Layout

### Desktop (`desktop/src/`)
- `app.rs` — Main application shell & left navigation rail
- `host.rs` — singpanel-host HTTP loopback client
- `store.rs` — Preferences, profiles, and templates persistence
- `net.rs` / `net_detect.rs` — Local network & exit IP detection (Cloudflare trace)
- `tailscale.rs` — Runtime Tailscale overlay (≥1.13; 1.14+ fields gated)
- `pages/` — `home.rs`, `proxies.rs`, `connections.rs`, `profiles.rs`, `templates.rs`, `logs.rs`, `settings.rs`

### Android (`mobile/app/src/main/kotlin/app/singplane/`)
- `ui/AppShell.kt` — Bottom navigation bar & routing
- `ui/pages/` — `HomePage.kt`, `ProxiesPage.kt`, `ConnectionsPage.kt`, `ProfilesPage.kt`, `TemplatesPage.kt`, `LogsPage.kt`, `SettingsPage.kt`
- `core/ControlPlane.kt` / `core/AndroidControlPlane.kt` — Android control plane implementation
- `core/LibboxCoreProcess.kt` — Embedded `libbox.aar` JNI adapter & platform network interface manager
- `vpn/SingPanelVpnService.kt` — Android `VpnService` implementation & TUN lifecycle
- `assemble/` — Profile assembling, runtime patching, and Tailscale overlay
- `store/` — Profile, template, and settings JSON stores

---

## Build & Test Commands

### 1. Shared Rust Control Library & Desktop
```bash
# Test core library and helper
cd core/lib && cargo test
cd core/helper && cargo test

# Build host and run desktop GPUI
cd core/host && cargo build
cd desktop && cargo run
```

### 2. Android (Jetpack Compose)
```bash
# One-click automated build (compiles official sing-box libbox.aar and builds APK)
./scripts/build_android.sh                  # Default stable sing-box (v1.13.19)
./scripts/build_android.sh v1.13.19         # Build specific sing-box tag
./scripts/build_android.sh v1.13.19 --release
./scripts/build_android.sh --skip-core      # Skip core compilation, assemble APK only

# Or run tests and build manually in mobile/
cd mobile
./gradlew :app:testDebugUnitTest
./gradlew :app:assembleDebug
```

---

## Test-Driven Development (TDD)

All new behavior and bug fixes across `core/`, `desktop/`, and `mobile/` **must** adhere to red → green → refactor:

1. **Red**: Write a unit test specifying the expected behavior before implementing.
   - Rust: `cargo test` in `core/lib`, `core/host`, or `desktop`.
   - Android: `./gradlew :app:testDebugUnitTest` in `mobile/`.
2. **Green**: Write the minimal implementation to make the test pass.
3. **Refactor**: Clean up and optimize while ensuring all tests remain green.

What requires unit tests:
- Config parsing, serialization, and runtime assemble invariants
- Tailscale overlay rules and version gating (`CoreLine`)
- Subscription fetchers, user-info headers, and error handling
- Network interface selection, proxy routing, and state machines

---

## Core Constraints

1. **Official Kernel Only**: Never vendor, modify, or fork the official sing-box data plane.
2. **Path Isolation**: Desktop changes stay in `desktop/` and `core/`; Android changes stay in `mobile/`.
3. **Security First**: Preserve helper / host security boundaries (Bearer token, path ACL, atomic file writes, LAN ranking).
4. **Clean Schemas**: Maintain cross-platform JSON storage compatibility across desktop and Android.
