# 安卓落地计划（对齐 Win / macOS 已实现功能）

给**初级开发**用：你会 Kotlin / Compose / 一点 JSON，但还不熟本仓库的规矩。  
本文写清：**能改什么、绝不能改什么、每一步怎么证明做对了**。  
读完再写代码。不要凭感觉「先搭个能跑的」。

目标：在 `mobile/` 上，用户能完成 **GPUI 桌面（Windows / macOS）已经能完成的全部事情**。  
不是像素级照搬（左栏、托盘、UAC helper 是桌面壳），而是**同一套用户任务都能做完**。

```
P0 本机 mixed + Clash 闭环
 → P1 VpnService TUN（对齐桌面 TUN）
 → P2 模板 + assemble + 文件/扫码导入
 → P3 首页指标 + 代理测速 + 热重载
 → P4 连接 / 日志 / 设置壳 / 订阅定时
 → P5 Tailscale + 多语言 + 开机恢复 VPN
 → P6 对表收口（矩阵全 ✅ 才算完）
```

**P6 没勾完，不能说「安卓功能齐了」。**

---

## 0. 先建立这几条直觉

1. **本仓库不是 sing-box 的 fork。** 我们只做 GUI + 控制面。数据包走官方 `sing-box`。
2. **桌面产品是 GPUI（`desktop/`），安卓产品是 Compose（`mobile/`）。**
3. **控制面和数据面分开。** 控制面：导入订阅、拼配置、启停进程、调 Clash API。数据面：官方内核。不要自己写代理协议。
4. **磁盘上的订阅原文不许在「点电源」时被改掉。** 启动时写一份 `config.runtime.json`（端口、去掉 TUN、Clash API、Tailscale）。用户下次打开配置，还是原来那份。
5. **安卓上的「系统代理 / TUN」不是桌面那套。** 桌面用 WinINET / helper / setuid；安卓用 `VpnService`。不要 `Runtime.getRuntime().exec` 去开 tun。
6. **先写测试，再写实现。** 没有测试的「感觉对了」不算完成。本仓库已经按这个流程搭过骨架。

---

## 1. 硬约束（违反即返工）

下面每一条都写了「为什么」和「怎么自查」。新人最容易在这里翻车。

### 1.1 目录与产品边界

| 约束 | 为什么 | 自查 |
|---|---|---|
| **只改 `mobile/`** 做安卓功能。不要改 `desktop/src/` | 改错树等于改错产品 | `git status` 只应出现 `mobile/`（除非任务明确写了改 `core/`） |
| **不要用 Compose Multiplatform 做桌面** | 桌面继续 GPUI，JVM 内存大 | 不要加 `desktop` compose 源集当产品 |
| **包名 `app.singplane`**，debug 为 `app.singplane.debug` | 调试包和正式包不能并存 | 不要改 `applicationId` |

### 1.2 数据面（内核）

| 约束 | 为什么 | 自查 |
|---|---|---|
| **只用官方 SagerNet/sing-box** | 产品定义。禁止自研内核、禁止 Mihomo 当数据面 | 下载 URL 必须是 `github.com/SagerNet/sing-box` |
| **禁止把内核源码 vendoring 进仓库** | 体积、协议、法律、维护都炸 | `mobile/` 里不能出现 sing-box 的 `.go` |
| **启停用官方命令** `sing-box run -c <runtime.json>`；校验用 `sing-box check -c` | 与桌面 host 一致 | 不要自己 parse 协议去「验证节点」 |
| **资源文件名必须对上官方 asset** | 下错包解压失败或架构不对 | `sing-box-{ver}-android-{arch}.tar.gz`。真机 `arm64`，模拟器常见 `amd64` |
| **P0 默认 `stripTunOnAssemble = true`** | 没把 TUN fd 交给内核时，配置里若有 tun inbound，内核会秒退 | 启动后读 `filesDir/.../config.runtime.json`，inbounds 里不应有 `"type":"tun"` |
| **P1 之前禁止 `VpnService.Builder.establish()`** | 空 TUN 会把流量黑洞，用户会觉得「联网全挂了」 | `SingPanelVpnService` 里搜 `establish`，P1 前应没有真正调用 |

### 1.3 控制面与架构

| 约束 | 为什么 | 自查 |
|---|---|---|
| **全应用只有一个 `ControlPlane` 实例** | 两个实例会双开内核、状态打架 | `SingPanelApp` / `MainActivity` 注入一次，页面不要 `AndroidControlPlane(...)` |
| **UI（`ui/`）不直接 `ProcessBuilder` / 不直接写文件 / 不直接 OkHttp 下内核** | 逻辑进控制面才能单测；Compose 里写 IO 无法测、易泄漏 | 页面只调 `ControlPlane` 和 `StateFlow` |
| **业务先放 Kotlin。UniFFI / `core/lib` 不挡功能** | Rust 抽库已存在，绑定以后再接。先让用户能用 | 不要在页面里 `external fun` |
| **JSON 字段名必须和桌面一致（camelCase）** | 以后可能互导；测试已按这些键写 | `corePath` 不是 `core_path`；`activeProfileId` 不是 `active_id` |
| **存盘用已有 `writeAtomically`** | 写到一半杀进程会坏文件 | 不要 `File.writeText` 直接覆盖设置/配置 |
| **Clash API 默认 `127.0.0.1`**。设置里若填 `0.0.0.0` / `::`，客户端仍连 `127.0.0.1` | 安卓上对外暴露 Clash API 危险 | 桌面 `clash_base_from_settings` 已这样处理，照抄 |
| **订阅 User-Agent** 与桌面一致：`sing-box/SingPanel clash.meta` | 部分机场按 UA 返回 Clash/sing-box | 不要用默认 OkHttp UA |
| **不要把 authKey、订阅 URL 打进 logcat** | 泄露等于交号 | `Log` / `_logs` 里只打「已刷新 xxx」，不打完整 URL/密钥 |

### 1.4 安卓平台

| 约束 | 为什么 | 自查 |
|---|---|---|
| **全局流量走 `VpnService`，不要实现 WinINET / 系统 HTTP 代理** | 安卓没有桌面那种「系统代理开关」的对等物 | 设置页不要做「系统代理」开关冒充桌面 |
| **TUN 必须 `protect()` 内核连出口的 socket** | 不 protect 会环路：流量进 VPN → 再进 VPN | P1 验收必须写进测试或注释 + 真机验证 |
| **VPN 排除自身（`addDisallowedApplication(packageName)`）** | 应用自己的 GitHub 下载、订阅刷新不能被自己劫持死锁 | Builder 里要排除本包 |
| **前台通知是「托盘」等价物** | 没通知，系统会杀进程 | 运行中必须有 ongoing 通知；点通知回应用；通知可停止 |
| **扫码、选文件用系统能力**（CameraX / ML Kit 或系统扫描；SAF `OpenDocument`） | 不要自己解析磁盘绝对路径，安卓没桌面那种 `C:\...` | 不要做「输入 /sdcard/...」当正式导入 |
| **主题色种子保持 `#047857`（`seedColorValue`）** | 和桌面主色一致 | 不要换成 Material 默认紫 |

### 1.5 工作方式（规范，不是口味）

| 约束 | 为什么 | 自查 |
|---|---|---|
| **TDD：先改/加 `app/src/test/`，再改实现** | 本仓库已有 10+ 个测试文件，是唯一可靠的回归网 | PR / 提交说明里应能指出新增测试名 |
| **一个阶段只做该阶段的用户故事** | 顺手重构首页 + 下载内核 + VPN，会无法验收 | 对照本章节「本阶段可改文件」 |
| **失败要有用户能看懂的中文（或 i18n key）** | 「Exception」不算产品 | `_status.message` / Snackbar 不要堆栈原文 |
| **不要 `GlobalScope`** | 页面销毁后协程还在跑，会重复启动内核 | 用 `viewModelScope` 或 `ControlPlane` 内部作用域 |
| **主线程禁止磁盘 / 网络 / `Process`** | ANR | IO 放 `Dispatchers.IO`（`ProcessCoreProcess` 已示范） |

### 1.6 明确不做（对齐桌面也不做）

- 用 Rust/Kotlin 重写内核或协议栈
- 实现桌面系统代理、托盘、开机启动项、UAC helper、setuid
- 为「看起来像」去抄内核
- 在模拟器上用 GPU 加速跑 Vulkan 实验壳
- 把 `stripTun` 在 P1 完成前设成默认 false

---

## 2. 验证方式（所有阶段通用）

没有对应验证，任务算没做完。

### 2.1 三级验证

| 级别 | 何时必须做 | 命令 / 做法 | 通过标准 |
|---|---|---|---|
| **L1 单元测试** | 每个逻辑改动 | `cd mobile; .\gradlew :app:testDebugUnitTest` | 全绿。新行为有对应 `*Test` |
| **L2 编译 APK** | 每个能点的功能 | `.\gradlew :app:assembleDebug` | 成功；APK 在 `app\build\outputs\apk\debug\` |
| **L3 手测** | 阶段收口 | 模拟器（P0/P2/P3/P4）+ **真机（P1 VPN 必须）** | 按该阶段「手测清单」逐条打勾 |

模拟器现成 AVD：`singpanel_api35`（API 35 / x86_64）。驱动脚本：`mobile/drive.ps1`。

```powershell
# 环境（scoop android-clt + temurin17）
$env:JAVA_HOME = "C:\Users\hpbox\scoop\apps\temurin17-jdk\current"
$env:ANDROID_HOME = "$env:USERPROFILE\scoop\apps\android-clt\current"
$env:ANDROID_SDK_ROOT = $env:ANDROID_HOME
$env:PATH = "$env:JAVA_HOME\bin;$env:ANDROID_HOME\emulator;$env:ANDROID_HOME\platform-tools;$env:PATH"

emulator -avd singpanel_api35 -gpu swiftshader_indirect -no-snapshot -no-audio
# 另开终端
cd mobile
.\gradlew :app:assembleDebug
.\drive.ps1
```

**TUN / 真实翻墙不要用模拟器当最终证据。** 模拟器 VPN 行为与真机差很多。P1 必须真机。

### 2.2 怎么写测试（照着现有文件学）

现成范例，不要另起一套风格：

| 已有测试 | 它在证明什么 | 你加功能时要仿这个 |
|---|---|---|
| `GithubReleasePickerTest` | 从 GitHub JSON 里挑对 asset | 下载：代理 URL 拼接、stable/beta 选择 |
| `ArchiveExtractorTest` | zip/tar 里抽出 `sing-box` | 解压后路径、可执行 |
| `CorePlatformTest` | ABI → `android-arm64` / `amd64` | 不要猜架构字符串 |
| `RuntimePatchTest` | 端口 / strip TUN / Clash API | 启动叠加；**断言原文对象没被原地改** |
| `ContentDetectorTest` | sing-box / Clash / URI | assemble 前分类 |
| `UserinfoParserTest` | `Subscription-Userinfo` | 流量、到期 |
| `ProfileStoreTest` / `SettingsStoreTest` | 存盘字段 | 新设置键必须能读回 |
| `AndroidControlPlaneTest` | 无配置 / 无内核 / 可运行配置会 start | 每个新 `ControlPlane` 方法加一条 |

规则：

- 测试里**禁止**真的访问 GitHub、禁止真的起 VPN。用假 `SubscriptionFetcher`、`RecordingCoreProcess`、`RecordingVpnSession`。
- 需要内核行为时：构造 JSON 字符串断言，不要在单元测试里 `ProcessBuilder("sing-box")`（CI/开发机不一定有内核）。
- `sing-box check` 的封装可以测「空路径跳过 / 找不到文件报错」；真正 check 成功放到 L3。

### 2.3 阶段完成的定义（DoD）

同时满足才许勾阶段：

1. L1 全绿，且本阶段列出的测试都存在。
2. L2 APK 能装。
3. L3 手测清单全部勾选（写在 PR / 提交说明里，或在本文件该阶段下改成 `[x]`）。
4. `git status` 没有改到禁区目录。
5. 失败路径有中文提示（没内核、没配置、订阅不是 JSON、VPN 用户取消）。

---

## 3. 现在已经有什么（不要重做）

只读、复用：

| 模块 | 路径 | 状态 |
|---|---|---|
| 设置 / 配置存盘 | `store/`、`model/AppSettings.kt`、`model/Profile.kt` | 字段还不齐，P2/P4/P5 会加键 |
| 订阅 GET + userinfo | `fetch/` | 有 |
| 内容识别 + runtime patch | `assemble/ContentDetector.kt`、`RuntimePatch.kt` | 有；**还没有模板 merge / convert** |
| 控制面 | `AndroidControlPlane` | 导入 URL/粘贴、选用、启停、Clash 组 |
| 进程内核 | `ProcessCoreProcess` | `run -c`；日志回调在，**还没接到 `logs` StateFlow** |
| 下载零件 | `GithubReleasePicker`、`ArchiveExtractor`、`CorePlatform` | **没有 HTTP 下载、没有设置页按钮** |
| VPN | `SingPanelVpnService` | 只前台通知，**不 establish** |
| 六页骨架 | `ui/pages/` | 连接页文件在，**未进底栏** |
| 模板页 | `TemplatesPage` | 骨架 |

对照桌面读代码（只读）：

- 行为：`desktop/src/pages/*.rs`、`store.rs`、`core_download.rs`、`tailscale.rs`、`net_detect.rs`
- 装配算法：`core/lib/src/assemble.rs`（先移植 Kotlin；不要在安卓上再起一个 HTTP host）
- 交互参考：FlClash

---

## 4. 功能矩阵（做到全 ✅ 才算和桌面对齐）

图例：✅ 已有 · 🟡 半成品 · ❌ 没有 · — 桌面专有，安卓用等价物

### 4.1 首页

| 功能 | 桌面怎么做的 | 安卓现在 | 阶段 | 验收要点 |
|---|---|---|---|---|
| 电源启停官方内核 | host `/v1/start` + `sing-box run` | 🟡 手填路径；进程能起 | P0 | 点一次起、再点停；停后进程不在 |
| 当前配置 / 端口 / Clash API / 内核名 | 首页信息条 | 🟡 状态 + 配置名 | P0 | 展示 mixed 端口、`127.0.0.1:9090`、内核文件名 |
| 运行时长 | 本地计时 | ❌ | P3 | 走着涨，停止清零 |
| 上传 / 下载总量与速率 | Clash `/traffic` 或 connections 差分 | ❌ | P3 | 有流量时数字动 |
| Clash 内存 | Clash `/memory` | ❌ | P3 | 运行中非 0 或显示 `--` |
| 代理模式 规则 / 全局 / 直连 | `PATCH /configs` `mode` | ❌ | P3 | 切换后代理页行为符合 mode |
| 内网 IPv4 | `net.rs` 列网卡 | ❌ | P3 | 能显示 WLAN 地址；没有就 `--` |
| 网络检测 | `net_detect.rs` Cloudflare trace | ❌ | P3 | 显示 IP（可遮罩）+ 国旗/地区；可换国际/国内源 |
| 刷新 | 重拉状态 + 检测 | ❌ | P3 | 按钮存在且不崩 |
| 系统代理 | WinINET / macOS | — VPN | — | **不要做这个开关** |
| TUN | helper / setuid | 🟡 只 prepare | P1 | 真机：开 VPN 后其它 App 走代理 |
| Tailscale 卡片 | 开、登录链、复制 IP | ❌ | P5 | 见 P5 |
| 内核崩溃回停止 | 桌面会反映 status | ❌ | P3 | 杀进程后首页不是「运行中」；日志有退出码 |

### 4.2 代理

| 功能 | 桌面 | 安卓现在 | 阶段 | 验收要点 |
|---|---|---|---|---|
| Clash 组列表 | `GET /proxies` | 🟡 有刷新/选节点 | P0 | 内核起来后能看到 selector |
| Selector 切换 | `PUT /proxies/{name}` | ✅ | — | 点节点后 `now` 变 |
| 延迟测试 | `GET /proxies/{name}/delay?url=gstatic 204` | ❌ | P3 | 显示 ms；超时有标记 |
| 排序 默认/延迟/名称 | 本地排 | ❌ | P3 | 三种都可用 |
| 搜索 | 过滤 tag | ❌ | P3 | 大小写不敏感 |

测速 URL **必须**是 `https://www.gstatic.com/generate_204`（与桌面 `TEST_URL` 一致）。超时 5000ms，并发不要超过 8。

### 4.3 连接

| 功能 | 桌面 | 安卓现在 | 阶段 | 验收要点 |
|---|---|---|---|---|
| 进主导航 | 左栏第 3 | ❌ 文件有未进底栏 | P4 | 底栏有「连接」 |
| `GET /connections` 1s 轮询 | ✅ | ❌ | P4 | 有流量时列表更新 |
| 搜索主机 / 进程 / 节点 | ✅ | ❌ | P4 | |
| 排序 默认 / 速度 / 流量 | ✅ | ❌ | P4 | |
| 关闭单条 | `DELETE /connections/{id}` | ❌ | P4 | 该行消失 |
| 最多约 200 行 | 桌面 `MAX_ROWS` | ❌ | P4 | 不要无上限涨内存 |

### 4.4 配置

| 功能 | 桌面 | 安卓现在 | 阶段 | 验收要点 |
|---|---|---|---|---|
| 列表、选用、删除 | ✅ | ✅ | — | |
| 订阅 URL 导入 | host `/v1/fetch` | ✅ | — | |
| 粘贴 JSON | ✅ | ✅ | — | |
| 本地文件 | 路径框 | 🟡 无 SAF | P2 | 系统文件选择器 |
| 扫码导入 | ✅ | ❌ | P2 | 扫 URL 后走和 URL 导入同一路径 |
| 导入时 assemble | 模板合并 | ❌ 仅 raw JSON | P2 | Clash/URI 在开模板后可变成可运行 |
| 远程刷新 + userinfo | ✅ | ✅ | — | |
| 流量 / 到期展示 | ✅ | 🟡 有字段列表略 | P2 | 列表看得到 |
| 运行中换配置 | 停再起 / reload | ❌ | P3 | 选用另一份会重载 |
| 订阅定时更新 | ≥15 分钟 | ❌ | P4 | WorkManager；应用死了也要能刷（尽力） |

Profile JSON 键（已有，不要改名）：

`id`, `name`, `sourceType`（`local` / `url` / `file`）, `path`, `url`, `content`, `updatedAt`, `upload`, `download`, `total`, `expireMs`, `runnable`, `lastError`, `assembleEnabled`, `templateId`, `sourceBody`, `contentKind`

`sourceType=url` 才定时刷新。刷新失败写 `lastError`，**不要删掉旧 `content`**。

### 4.5 模板

| 功能 | 桌面 | 安卓现在 | 阶段 | 验收要点 |
|---|---|---|---|---|
| 内置两套 | `builtin-mixed-direct` / `builtin-mixed-rule` | ❌ | P2 | id **一字不差** |
| 用户模板增删改 | ✅ | ❌ | P2 | 内置只读 |
| JSON 编辑 | ✅ | ❌ | P2 | 非法 JSON 保存失败并提示 |
| 配置绑定 templateId + assemble | ✅ | ❌ | P2 | 导入开关默认跟设置走 |

内置模板内容从桌面 `desktop/assets/templates/` **原样拷进** `mobile/app/src/main/assets/templates/`，不要手改 inbound 结构。安卓默认会 strip TUN，拷过来即可。

### 4.6 日志

| 功能 | 桌面 | 安卓现在 | 阶段 | 验收要点 |
|---|---|---|---|---|
| 内核 stdout / 文件尾 | 256KB tail | 🟡 只有控制面 start/stop 字 | P0 | 启动后能看到内核自己的日志行 |
| 轮询 / 流式追加 | 1.5s 读文件 | ❌ | P0 | `onLog` → `logs` |
| 搜索 | ✅ | ❌ | P4 | |
| 清空显示 | 只清 UI，不删文件 | ❌ | P4 | 之后只显示新行 |

### 4.7 设置（键名必须一致）

桌面 `default_settings()` 全键。安卓要逐步补齐。**不要发明新名字。**

| 键 | 桌面默认 | 安卓现在 | 阶段 | 安卓说明 |
|---|---|---|---|---|
| `corePath` | AppData cores | 手填 | P0 | 下载后写成 `filesDir/cores/sing-box` |
| `coreChannel` | `beta` | ❌ | P0 | `stable` / `beta` |
| `githubProxy` | `""` | ❌ | P0 | 预设与桌面相同，见下 |
| `mixedPort` | 7890 | ✅ | — | |
| `clashApiPort` | 9090 | ✅ | — | |
| `clashApiHost` | 127.0.0.1 | ✅ | — | |
| `forceAppPortsOnAssemble` | true | ✅ | — | |
| `stripTunOnAssemble` | 桌面 false | ✅ 开关；**P0 默认 true** | P0 | P1 完成后：开 VPN 时启动路径不要 strip |
| `defaultAssembleOnImport` | false | ❌ | P2 | |
| `defaultTemplateId` | `builtin-mixed-direct` | ❌ | P2 | |
| `autoUpdateSubscriptions` | true | ❌ | P4 | |
| `autoUpdateIntervalMinutes` | 60，最小 15 | ❌ | P4 | `<15` 当 15 |
| `theme_mode_v1`（单独存） | `system` | ✅ 已支持 Light/Dark/System | — | `light` / `dark` / `system` |
| `language` | `system` | ✅ 已支持 | P5 | `zh-Hans` / `zh-Hant` / `en` / `system` |
| `seedColorValue` | `0xFF047857` | 模型有，无 UI | — | 先保持默认，可不做调色盘 |
| `disclaimer_accepted_v1` | bool | ❌ | P4 | 首次启动弹窗，文案与桌面 `DISCLAIMER_TEXT` 一致 |
| `tailscale.*` | 整包 | ✅ 已完整移植 | P5 | 键名见 §4.8 |
| `closeToTray` / `trayEnabled` | 桌面 | — | P4 | **不要做开关**。用前台通知 |
| `launchAtStartup` | 桌面 | — | — | 明确不需要开机自启 |
| `systemProxyEnabled` | 桌面 | — | — | **不要做** |
| `tunEnabled` | 桌面首页开关 | 🟡 | P1 | 安卓 =「使用 VPN」 |
| `autoStartCore` | 桌面有键 | — | — | 明确不需要开机自启 |
| `activeProfileId` | ✅ | ✅ | — | |

GitHub 代理预设（与 `desktop/src/core_download.rs` 完全一致）：

| id | 标签 | prefix |
|---|---|---|
| `direct` | 直连 | `""` |
| `ghfast` | ghfast | `https://ghfast.top` |
| `gh-proxy` | gh-proxy | `https://gh-proxy.com` |
| `ghproxy-net` | ghproxy.net | `https://ghproxy.net` |
| `llkk` | gh.llkk.cc | `https://gh.llkk.cc` |

拼接规则：`{proxy}/{original_url}`。空 prefix = 原 URL。用户可手输自定义 prefix。

### 4.8 运行时叠加（启动时，不改订阅原文）

| 功能 | 桌面 | 安卓现在 | 阶段 |
|---|---|---|---|
| 端口 / Clash API / strip TUN | `RuntimePatch` / `for_runtime` | ✅ | — |
| `sing-box check` | host `/v1/check` | ❌ | P2 |
| Tailscale endpoint + DNS | `tailscale.rs` | ✅ `TailscaleOverlay` | P5 |
| `preferred_by` / `accept_search_domain` | **仅内核 ≥1.14** | ✅ `CoreLine.V14` 门控 | P5 |
| endpoint / MagicDNS | **≥1.13**（含 1.13.18） | ✅ `CoreLine.V13` 门控 | P5 |

Tailscale 对象键（嵌在 settings.`tailscale`）：

`enabled`, `tag`（默认 `ts-local`）, `authKey`, `controlUrl`, `hostname`, `stateDirectory`, `acceptRoutes`（默认 true）, `advertiseExitNode`, `exitNodeAllowLanAccess`, `exitNode`, `advertiseRoutes`, `advertiseTags`, `systemInterface`, `sshServer`, `replaceOtherTailscale`（默认 true）, `injectDns`（默认 true）, `acceptDefaultResolvers`, `acceptSearchDomain`（默认 true）, `injectRoutePreferredBy`（默认 true）, `routeDomainSuffix`（默认 `.ts.net`）, `routeIpCidr`

**不要在安卓上做 `systemInterface=true` 当默认**——那是桌面系统网卡。VpnService 已经是隧道。

### 4.9 装配（assemble）——订阅真正能用的关键

桌面流水线（`core/lib` + host）：

1. `detect(body)` → `singbox` / `clash` / `uriList` / `unknown`
2. 若 clash / URI：走 convert sidecar → 得到 sing-box JSON
3. 从源里抽出节点（跳过 `direct`/`block`/`dns`，selector/urltest 看选项）
4. 把节点合并进模板
5. `RuntimePatch`（端口 / Clash / strip TUN）
6. 可选 `sing-box check`

安卓 P2 最低要求：

- sing-box 源 + 模板 merge **必须**在纯 Kotlin 里能单测（对照 `core/lib/src/assemble.rs` 的 `extract`/`merge`）。
- Clash / URI：允许第一刀调用打包的 convert，或先提示「请用已转换的 sing-box JSON」——**但 P2 结束前必须能导入常见 Clash 订阅**，否则不算对齐桌面「导入订阅就能用」。
- 选项键：`include`, `exclude`, `addSourceTag`, `disableDefaultGroups`, `keepSourceGroups`, `keepSourceDns`, `keepSourceRoute`（默认全空/false）。

**convert 不要在安卓上再起一套 loopback HTTP + token。** 那是给桌面 GPUI 用的。能链 so / 直接函数更好；过渡期可以进程，但不要把 token 打到 log。

---

## 5. 阶段任务（按这个做，按这个验）

每阶段格式：**目标 → 可改文件 → 先写哪些测试 → 实现要点 → 手测清单 → 常见错误**。

---

### P0 — 本机代理能用（mixed + Clash）

**目标：** 用户不装 VPN，也能：下载官方内核 → 粘贴/URL 导入可运行 JSON → 电源开 → 代理页有组。  
这是后面所有阶段的地基。

**可改：** `core/`（下载器、接日志）、`model/AppSettings.kt`（`coreChannel`、`githubProxy`、默认 `stripTun`）、`store/`、`ui/pages/SettingsPage.kt`、`ui/pages/HomePage.kt`、`ui/pages/LogsPage.kt`、`AndroidControlPlane.kt`、对应 test。  
**不可改：** `SingPanelVpnService.establish`、模板 assemble、Tailscale、桌面目录。

**先写测试（L1）：**

- [ ] `GithubProxyTest`：空 prefix 原样；`https://ghfast.top` + github url → `https://ghfast.top/https://github.com/...`
- [ ] `CoreDownloader` 用假 HTTP：选 stable/beta、asset 名 `android-arm64` / `android-amd64`
- [ ] `AndroidControlPlaneTest`：`onLog` 进入 `logs`；默认 strip 后 runtime 无 tun inbound
- [ ] `SettingsStoreTest`：读写 `githubProxy`、`coreChannel`；新安装 `stripTunOnAssemble==true`

**实现要点：**

- 下载到 `context.filesDir/cores/`，解压出 `sing-box`，`setExecutable`，写入 `corePath`。
- 旧文件先改名 `.bak`，解压失败再改回去（桌面 `core_download.rs` 已这样做）。
- `ProcessCoreProcess` 的 `onLog` 必须接到 `ControlPlane.logs`（现在回调是空的）。
- 首页展示端口 / Clash / 内核文件名。
- 设置页按钮：「查看版本」「下载匹配内核」。失败显示中文（网络、无对应 asset、解压失败）。

**手测清单（L3，模拟器即可）：**

- [ ] 设置 → 下载（模拟器用 amd64）。`corePath` 变成 `.../files/cores/sing-box`
- [ ] 换 GitHub 代理后再下（若直连失败）
- [ ] 配置页粘贴一份带 mixed + clash_api 的 JSON（可参考 `desktop/examples/http-proxy.json`，端口会被 patch 成设置值）
- [ ] 选用 → 首页电源 → 状态「运行中」
- [ ] 日志页出现内核输出（不是只有 `start xxx`）
- [ ] 代理页刷新能看到组（没有组则检查 Clash API 是否被 patch 进去）
- [ ] 再点电源停止；设置里把内核文件改成坏路径，启动应失败且中文提示
- [ ] `adb shell run-as app.singplane.debug cat files/runtime/config.runtime.json`（或应用 filesDir 等价路径）确认 **没有 tun inbound**

**常见错误：**

- 下了 `linux-amd64` 而不是 `android-amd64`（模拟器也能跑错包，但不要依赖这个）
- 解压后二进制在子目录 `sing-box-1.x.x-android-amd64/sing-box`，没找到就报失败——extractor 必须递归找名为 `sing-box` 的文件
- 忘记 `setExecutable`，表现为立刻退出
- 订阅是 Clash YAML，P0 应明确提示「不是可运行 JSON」，不要假装启动成功

---

### P1 — 系统 VPN（对齐桌面 TUN）

**目标：** 真机打开「VPN」后，其它 App 的流量走官方内核。这是桌面 TUN 的安卓等价。

**可改：** `vpn/*`、`AndroidControlPlane.start/stop`、首页 VPN 开关、`AndroidManifest`（已有则只补权限说明）。  
**不可改：** 下载器大改、模板、桌面 helper。

**先写测试（L1）：**

- [ ] 控制面：VPN 授权取消 → 不 start 内核、状态说明「用户取消」
- [ ] 控制面：VPN 开 → 传给内核的 runtime **保留 tun**（或按官方/libbox 约定带上 fd）；VPN 关且无其它 inbound 策略时仍 strip
- [ ] Builder 配置单测（若抽纯函数）：地址、DNS、`addDisallowedApplication`、路由

**实现要点（按这个顺序，不要跳）：**

1. `VpnService.prepare`（已有 `NeedVpnConsent`）→ 用户同意。
2. `Builder`：IPv4 地址（常用 `172.19.0.1/30` 一类）、DNS、`addRoute`、`addDisallowedApplication(packageName)`、`setSession`、`setMtu`。
3. `establish()` 得到 `ParcelFileDescriptor`。
4. **把 TUN 交给官方内核**。做法必须查当前官方 sing-box 安卓文档：常见是 inbound `tun` + `file_descriptor` / libbox。**不要**自己 `read(tunFd)` 再转发。
5. 对内核的出站 socket `protect(fd)`。
6. 前台通知：运行中、点回 `MainActivity`、停止按钮走 `ACTION_STOP` 且控制面 `stop()`。
7. 未授权或 establish 失败：内核不要处于「半开」。先停进程，关 fd。

**手测清单（L3，必须真机）：**

- [ ] 第一次开：系统 VPN 授权页出现；取消后应用仍可用，首页不是「运行中」
- [ ] 同意后：系统设置里能看到本应用 VPN；通知常驻
- [ ] 浏览器访问会走代理（用你自己的可运行配置；不要在计划里写违法用途）
- [ ] 关 VPN / 点停止：隧道拆掉，通知消失，内核停
- [ ] 开着 VPN 时设置页仍能刷新订阅（排除自身生效）
- [ ] 杀应用：VPN 不应留下「已连接但没进程」的幽灵（`START_STICKY` 要能自洽，或明确不复活）

**常见错误：**

- 先 `establish` 再起内核失败 → 整机断网。必须先能起内核或失败时立刻 `close` tun
- 忘记 `protect` → 延迟爆炸或完全不通
- 在模拟器勾 P1 完成
- 去改桌面树当「参考实现」

---

### P2 — 配置与模板（对齐桌面配置 + 模板）

**目标：** 用户用 Clash 订阅 / URI 列表 / 本地文件 / 扫码，也能得到可运行配置。这是桌面最核心的「能用」。

**可改：** `assemble/`（新增 Assembler，对照 `core/lib/src/assemble.rs`）、`templates` 资源与 store、`ProfilesPage`、`TemplatesPage`、`ControlPlane` 导入路径、设置里 assemble 开关。  
**不可改：** VPN 大改、Tailscale。

**先写测试（L1）：**

- [ ] `AssemblerTest`：一份只有节点 outbound 的 sing-box 源 + `builtin-mixed-direct` 模板 → 结果含 mixed inbound、含那些节点、含 clash_api（若 patch 打开）
- [ ] 抽出时跳过 `direct`/`block`/`dns`；0 节点 → `ok=false` 且中文 error
- [ ] 模板非法 JSON → 失败，不写坏 profile
- [ ] 内置模板只读：`save`/`delete` 报错
- [ ] `ContentDetector` 已有则复用；补 Clash YAML / URI 样例
- [ ] `check` 封装：空 corePath 跳过；文件不存在报错
- [ ] Profile 刷新失败保留旧 `content`

**实现要点：**

- 内置模板 id：`builtin-mixed-direct`、`builtin-mixed-rule`。从桌面 assets **原样复制**。
- 导入流程与桌面一致：`detect` →（需要则 convert）→ assemble → 可选 check → 存 `content`（可运行 JSON）+ `sourceBody`（原文）+ `contentKind` + `templateId`。
- SAF：`ACTION_OPEN_DOCUMENT`，读 utf-8 文本。超大文件要上限（桌面 fetch 16MB，照这个数量级）。
- 扫码：只接受 http(s) 订阅 URL 或节点 URI；扫到别的提示无效。
- 列表展示 `trafficLabel` 和到期日（`expireMs`）。
- 设置：`defaultAssembleOnImport`、`defaultTemplateId`。导入页开关初始值跟设置走。

**手测清单（L3）：**

- [ ] 模板页能看到两个内置；点开能看 JSON；不能删
- [ ] 新建用户模板、编辑、删除
- [ ] 粘贴纯节点 sing-box JSON + 勾选装配 → `runnable=true` → 电源能开
- [ ] URL 导入 Clash 订阅（你自己的测试源）→ 成功或明确「转换失败」原因
- [ ] SAF 选一个 `.json` 导入
- [ ] 扫码导入（可用一张含订阅 URL 的二维码）
- [ ] 非法 JSON 文件：列表里有 `lastError`，不会把应用打崩
- [ ] 有内核时导入后 check 失败：配置标不可运行，电源拒绝启动

**常见错误：**

- 把模板 merge 写成「整份 source 覆盖模板」，节点 inbound 会丢
- 改订阅原文去改端口——必须只改 runtime
- 内置模板改名成中文 id
- 在 UI 里复制一份 assemble 逻辑，控制面另一份，两边不一致

---

### P3 — 首页与代理补齐

**目标：** 首页信息密度和桌面同一档；代理能测速；运行中换配置；内核死了能看出来。

**可改：** `HomePage`、`ProxiesPage`、`clash/` 客户端、`AndroidControlPlane` 状态机。  
**不可改：** 另起一套统计协议。只读 Clash API。

**先写测试（L1）：**

- [ ] 流量差分：两次 `/traffic` 或 connections 快照算出 B/s（纯函数）
- [ ] 代理排序：延迟未知的排后面；名称按 locale
- [ ] 搜索过滤
- [ ] 内核 `isAlive==false` → status.running=false，日志含退出码
- [ ] 热重载：`setActiveProfile` 在 running 时会 stop+start（或等价），recording 里能看到顺序

**实现要点：**

- 上下行：Clash `GET /traffic`（chunked）或轮询 connections。UI 格式对齐桌面（KB/s、MB）。
- 模式：`GET /configs` 读 `mode`，切换 `PATCH` `{ "mode": "rule"|"global"|"direct" }`。
- 网络检测：源列表与 `desktop/src/net_detect.rs` 相同（Cloudflare / 国内 CN trace）。解析 `ip=`、`loc=`。遮罩显示 `*** *** *** ***`。
- 延迟：`GET /proxies/{encoded}/delay?url=https://www.gstatic.com/generate_204&timeout=5000`。名字编码用桌面 `clash_encode_name` 同一规则。
- 内网 IP：`NetworkInterface`；虚拟网卡可标一下，但别崩溃。

**手测清单（L3）：**

- [ ] 运行中首页：时长、速率、总量、内存、模式、内网 IP
- [ ] 切 规则/全局/直连，不崩
- [ ] 网络检测出 IP；点遮罩隐藏；换源
- [ ] 代理页测速出数字；排序；搜索
- [ ] 运行中选用另一配置，流量不断档或短暂重连后恢复
- [ ] `adb shell kill` 内核进程后，首页回到停止并有日志

---

### P4 — 连接、日志、设置壳、订阅定时

**目标：** 桌面除 Tailscale / i18n / 开机以外的壳功能都在。

**可改：** `AppDest`（加入连接）、`ConnectionsPage`、日志搜索清空、设置主题与免责、WorkManager 订阅更新、应用图标。  
**不可改：** 桌面托盘实现。

**先写测试（L1）：**

- [ ] 连接行解析（host、process、chains、up/down、速度差分）
- [ ] 搜索 / 排序纯函数
- [ ] 间隔分钟：`3` → 存 15；`60` 保持 60
- [ ] 日志「清空」只影响显示缓冲
- [ ] 免责：未接受时进不了主界面（或有阻断层）

**实现要点：**

- 底栏顺序建议：首页、代理、**连接**、配置、模板、日志、设置。项多可把连接放代理页入口，但矩阵要求「进主导航」，优先底栏。
- 轮询 1s；页面不可见时停轮询（省电）。
- 主题：`light` / `dark` / `system`，键与桌面 `theme_mode_v1` 对齐（安卓可存在 settings JSON 里，但**值集合一致**）。
- 免责声明原文必须与 `desktop/src/store.rs` 的 `DISCLAIMER_TEXT` 一致。
- 图标：与 `desktop/assets/app_icon.ico` 同一套 mark，不要用 Android 绿机器人。
- 订阅定时：`WorkManager` + 约束「有网」。只刷新 `sourceType==url`。失败写 `lastError`。
- 通知渠道已有则复用；不要再做一个「托盘开关」。

**手测清单（L3）：**

- [ ] 连接页出现在底栏；有流量时有行；搜索；关闭一条
- [ ] 日志搜索、清空后再出新行
- [ ] 设置改主题立即生效
- [ ] 清应用数据后先看到免责，不同意进不去
- [ ] 自动更新打开，把间隔设 15，过一会儿（或临时把间隔在 debug 调短**仅测完改回**）能看到 `updatedAt` 变
- [ ] 启动器图标是产品标，不是默认机器人

---

### P5 — Tailscale + 多语言 + 开机恢复

**目标：** 桌面剩余产品能力。不挡 P0–P4 交付，但 P6 前必须做完。

**可改：** `model` tailscale 嵌套对象、启动 overlay、`HomePage` 卡片、`i18n`、可选 `BOOT_COMPLETED`。  
**不可改：** 桌面 `tailscale.rs` 行为（只许移植，不许发明字段）。

**先写测试（L1）：**

- [x] overlay：`enabled=false` 配置不变
- [x] `enabled=true` 写入 endpoint；**假内核版本 1.13** 不含 `preferred_by`；**1.14** 才含
- [x] 登录 URL / `100.` IP 从日志或 status 解析（对照桌面 `tailscale.rs` 测试用例或注释）
- [x] 语言：`zh-Hant` / `en` / `system` 切 key 不崩
- [x] 开机：按需求明确不需要开机自启动

**实现要点：**

- 字段名 §4.8。启动时 overlay，**不写回 profile**。
- 首页卡片：开关、复制登录 URL、打开浏览器、显示 `100.x`、状态文案。不要「点整张卡片开启」这类多余 hint。
- `systemInterface` 在安卓上忽略或强制 false。
- 文案：简体 / 繁体 / English。先抽 `strings.xml` + 设置项，不要硬编码新中文到各个页面（旧中文可逐步搬）。
- 开机恢复：明确不需要。

**手测清单（L3，真机）：**

- [x] 设置打开 Tailscale，填测试 auth 或走登录链，启动后卡片有状态
- [x] 内核 1.13 配置里没有 1.14 字段（抓 runtime JSON）
- [x] 语言切 English，底栏/设置变成英文；切回简体
- [x] 开机恢复：跳过（按需求明确不需要）

---

### P6 — 对表收口

**目标：** 第 4 章矩阵全部 ✅ 或 —。用户拿安卓机能做完 Win/macOS 上能做的事。

**清单：**

- [ ] 把第 4 章每一行改成 ✅ 或 —（不允许残留 🟡）
- [ ] L1：`.\gradlew :app:testDebugUnitTest` 全绿
- [ ] L2：release 签名方案有文档（debug 可先用）
- [ ] L3 真机一条龙：免责 → 下载内核 → URL/模板导入 → mixed 确认 Clash 组 → 开 VPN → 测速 → 关一条连接 → 看日志 → Tailscale 登录 → 切语言
- [ ] L3 模拟器：下载 amd64、无 VPN 的 mixed 闭环仍可用
- [ ] `mobile/README.md` 把「骨架 / 还不 establish」改成「功能与桌面对齐」，并链到本文矩阵
- [ ] 对照 §1 再扫一遍，没有违规改动

---

## 6. 推荐文件归属（减少打架）

| 目录 | 谁改 | 放什么 |
|---|---|---|
| `ui/pages/X.kt` | 只做 X 页外观 | 不要 IO |
| `core/AndroidControlPlane.kt` | 编排 | 启停、导入、状态 |
| `assemble/` | 纯函数 | detect / patch / merge |
| `clash/` | Clash HTTP | 解析 + 客户端 |
| `fetch/` | 订阅 | |
| `store/` | 磁盘 | |
| `vpn/` | 仅 VpnService | |
| `app/src/test/` | 先于实现 | 不依赖 Android 框架的尽量放这里 |

新增设置键：先改 `AppSettings` + `SettingsStoreTest`，再改 UI。

---

## 7. 桌面能力 → 安卓等价（提醒）

| 桌面 | 安卓等价 | 算对齐？ |
|---|---|---|
| 系统代理 | VpnService 全局 | 是（不要再做系统代理） |
| TUN helper / setuid | VpnService + 系统授权 | 是 |
| 托盘 / 关到托盘 | 前台通知 + 划掉停止 | 是 |
| 开机启动 | 可选 BOOT 恢复 VPN | P5 做了算是 |
| 文件路径框 | SAF | 是 |
| 左栏 | 底栏 | 是 |
| host HTTP + Bearer | 进程内 Kotlin 函数 | 是（不要为本机再监听一个端口） |
| convert sidecar HTTP | 库调用或无端口 sidecar | 是 |

---

## 8. 下一刀

从 **P0** 开始。不要先做 P1 VPN，也不要先做 UniFFI。

P0 第一批提交建议拆成：

1. 测试：github 代理 + 默认 stripTun + logs 接口  
2. 下载并安装官方内核 + 设置页按钮  
3. 接上日志 + 首页信息条  
4. 模拟器手测，按 P0 清单打勾  

做完 P0 再开 P1。
