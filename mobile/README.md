# SingPanel Android（Compose）

安卓**产品**壳。页面用 Jetpack Compose；控制面稍后用 UniFFI 链 `core/host` 同一套逻辑。  
数据面仍是**官方 sing-box**，经 `VpnService` 拿 TUN fd——现在还没接。

落地计划见 [PLAN.md](PLAN.md)（按初级开发写：硬约束、每阶段先写哪些测试、怎么手测。做到 P6 才算和 Win/macOS 功能对齐）。

| 不要用 | 原因 |
|---|---|
| `desktop/` | 桌面壳；不要当安卓交付 |
| Compose Multiplatform 桌面 | 桌面继续 GPUI 省内存 |

## 状态（纯安卓，Kotlin 控制面）

TDD：`app/src/test/` 先写断言，再实现。跑 `.\gradlew :app:testDebugUnitTest`。

已有：

- 配置文件存盘、设置、订阅 GET + `Subscription-Userinfo`
- 启动时 runtime patch（端口 / Clash API / 可选去掉 TUN）
- `AndroidControlPlane`：导入 URL、选用、启停；VPN 授权走系统对话框
- `VpnService` 只起前台服务，**还不** `establish()`（TUN 等内核接 fd）
- 启动会拉官方 `sing-box run -c`（设置里填内核路径）
- 代理页读 Clash API；配置页可粘贴 JSON

Rust 抽库是另一条线，不要往这个 UI 里塞 UniFFI。

## 构建
 
JDK 17、Android SDK、Go 1.22+ 与 gomobile。

```bash
# 方式一：根目录一键脚本（自动克隆官方 sing-box、编译 libbox.aar 并构建 APK）
./scripts/build_android.sh               # 默认 v1.13.19
./scripts/build_android.sh v1.13.19      # 指定版本
./scripts/build_android.sh --skip-core   # 跳过内核编译仅打包 APK

# 方式二：手动在 mobile 目录构建
cd mobile
./gradlew :app:assembleDebug
# APK: app/build/outputs/apk/debug/app-debug.apk
# 包名: app.singplane.debug
```

## 模拟器

AVD 名：`singpanel_api35`（API 35 / google_apis / x86_64 / Pixel 7）。

```powershell
$env:ANDROID_HOME = "$env:USERPROFILE\scoop\apps\android-clt\current"
$env:ANDROID_SDK_ROOT = $env:ANDROID_HOME
$env:PATH = "$env:ANDROID_HOME\emulator;$env:ANDROID_HOME\platform-tools;$env:PATH"

emulator -avd singpanel_api35 -gpu swiftshader_indirect -no-snapshot -no-audio
# 另开终端：
cd mobile
.\gradlew :app:assembleDebug
.\drive.ps1
```

上次实机驱动：启动成功，电源切到「运行中」，底栏点过 代理 / 配置 / 模板。adb 在模拟器上会偶发 offline，重开 `adb start-server` 即可。

包名 `app.singplane`。调试包为 `app.singplane.debug`，两者不能同时装。
