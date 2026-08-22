//! Internationalization (i18n) support for SingPanel GPUI shell.
//! Supports English (en), 简体中文 (zh-Hans), and 繁體中文 (zh-Hant).

use std::sync::atomic::{AtomicU8, Ordering};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Language {
    ZhHans,
    ZhHant,
    En,
}

impl Language {
    pub const ALL: [Language; 3] = [Language::ZhHans, Language::ZhHant, Language::En];

    pub fn code(&self) -> &'static str {
        match self {
            Language::ZhHans => "zh-Hans",
            Language::ZhHant => "zh-Hant",
            Language::En => "en",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Language::ZhHans => "简体中文",
            Language::ZhHant => "繁體中文",
            Language::En => "English",
        }
    }

    pub fn from_code(s: &str) -> Self {
        match s.to_ascii_lowercase().as_str() {
            "zh-hant" | "zh-tw" | "zh-hk" | "zh-mo" | "traditional" => Language::ZhHant,
            "en" | "en-us" | "en-gb" | "english" => Language::En,
            _ => Language::ZhHans,
        }
    }
}

static CURRENT_LANG: AtomicU8 = AtomicU8::new(0); // 0: ZhHans, 1: ZhHant, 2: En

pub fn current_lang() -> Language {
    match CURRENT_LANG.load(Ordering::Relaxed) {
        1 => Language::ZhHant,
        2 => Language::En,
        _ => Language::ZhHans,
    }
}

pub fn set_current_lang(lang: Language) {
    let val = match lang {
        Language::ZhHans => 0,
        Language::ZhHant => 1,
        Language::En => 2,
    };
    CURRENT_LANG.store(val, Ordering::Relaxed);
}

pub fn detect_system_language() -> Language {
    let loc = std::env::var("LC_ALL")
        .or_else(|_| std::env::var("LC_MESSAGES"))
        .or_else(|_| std::env::var("LANG"))
        .unwrap_or_default();
    if loc.to_ascii_lowercase().contains("tw")
        || loc.to_ascii_lowercase().contains("hk")
        || loc.to_ascii_lowercase().contains("hant")
    {
        Language::ZhHant
    } else if loc.to_ascii_lowercase().starts_with("en") {
        Language::En
    } else {
        Language::ZhHans
    }
}

/// Helper macro / function to translate keys
pub fn tr<'a>(key: &'a str) -> &'a str {
    tr_lang(current_lang(), key)
}

pub fn tr_lang<'a>(lang: Language, key: &'a str) -> &'a str {
    match lang {
        Language::ZhHans => translate_zh_hans(key),
        Language::ZhHant => translate_zh_hant(key),
        Language::En => translate_en(key),
    }
}

fn translate_zh_hans<'a>(key: &'a str) -> &'a str {
    match key {
        // Navigation
        "nav.home" => "首页",
        "nav.proxies" => "代理",
        "nav.connections" => "连接",
        "nav.profiles" => "配置",
        "nav.templates" => "模板",
        "nav.logs" => "日志",
        "nav.settings" => "设置",

        // Common
        "common.save" => "保存",
        "common.cancel" => "取消",
        "common.confirm" => "确认",
        "common.delete" => "删除",
        "common.edit" => "编辑",
        "common.copy" => "复制",
        "common.copied" => "已复制",
        "common.success" => "成功",
        "common.failed" => "失败",
        "common.refresh" => "刷新",
        "common.close" => "关闭",
        "common.open" => "打开",
        "common.search" => "搜索",
        "common.retry" => "重试",
        "common.loading" => "加载中...",
        "common.saving" => "保存中…",
        "common.custom" => "自定义",
        "common.unknown" => "未知",
        "common.enabled" => "已启用",
        "common.disabled" => "已禁用",

        // Home
        "home.status.running" => "运行中",
        "home.status.stopped" => "未运行",
        "home.status.starting" => "正在启动…",
        "home.status.stopping" => "正在停止…",
        "home.status.reloading" => "正在重载配置…",
        "home.btn.start" => "启动",
        "home.btn.stop" => "停止",
        "home.card.traffic" => "流量统计",
        "home.card.upload" => "上传",
        "home.card.download" => "下载",
        "home.card.memory" => "内核内存",
        "home.card.net_detect" => "网络检测",
        "home.card.system_proxy" => "系统代理",
        "home.card.tun_mode" => "TUN 模式",
        "home.card.active_profile" => "当前配置",
        "home.card.no_active_profile" => "未选择配置",
        "home.tun.elevated" => "虚拟网卡授权",
        "home.tun.elevate_btn" => "授权 TUN",
        "home.tun.elevated_ready" => "已获 TUN 权限",
        "home.detect.click_to_test" => "点击重新测试",
        "home.detect.testing" => "检测中...",
        "home.detect.domestic" => "国内连通",
        "home.detect.intl" => "海外出口",
        "home.tailscale.card" => "Tailscale",
        "home.tailscale.not_logged_in" => "未登录 Tailscale",
        "home.tailscale.connected" => "已连接到 Tailnet",
        "home.tailscale.login_link" => "点击登录 Tailscale",

        // Proxies
        "proxies.title" => "代理策略组",
        "proxies.test_delay" => "测速",
        "proxies.testing_delay" => "测速中...",
        "proxies.search_placeholder" => "搜索节点名称...",
        "proxies.sort.default" => "默认",
        "proxies.sort.delay" => "延迟",
        "proxies.sort.name" => "名称",
        "proxies.empty" => "暂无代理节点",
        "proxies.not_running_hint" => "内核未启动，请先启动内核",

        // Connections
        "connections.title" => "活动连接",
        "connections.close_all" => "断开全部",
        "connections.search_placeholder" => "搜索主机 / 进程 / 节点...",
        "connections.count" => "总连接数",
        "connections.upload_speed" => "实时上传",
        "connections.download_speed" => "实时下载",
        "connections.host" => "目标主机",
        "connections.network" => "网络协议",
        "connections.process" => "进程",
        "connections.outbound" => "出口",
        "connections.rule" => "命中规则",
        "connections.empty" => "当前无活动连接",
        "connections.sort.default" => "默认",
        "connections.sort.speed" => "实时速度",
        "connections.sort.traffic" => "累计流量",

        // Profiles
        "profiles.title" => "配置管理",
        "profiles.import_btn" => "导入配置",
        "profiles.assemble_btn" => "聚合生成",
        "profiles.import_title" => "导入订阅或本地文件",
        "profiles.import_name" => "配置名称",
        "profiles.import_url" => "订阅 URL / 本地路径",
        "profiles.import_interval" => "自动更新间隔 (分钟)",
        "profiles.import_submit" => "确认导入",
        "profiles.update_all" => "更新全部订阅",
        "profiles.update" => "更新",
        "profiles.updating" => "更新中...",
        "profiles.assembled_badge" => "聚合",
        "profiles.local_badge" => "本地",
        "profiles.remote_badge" => "订阅",
        "profiles.sample_badge" => "示例",
        "profiles.empty" => "暂无配置文件",
        "profiles.view" => "查看配置",
        "profiles.view_empty" => "（空配置）",
        "profiles.set_current" => "设为当前",
        "profiles.current" => "当前配置",
        "profiles.badge_current" => "当前",
        "profiles.badge_download_only" => "仅下载",
        "profiles.update_sub" => "更新订阅",
        "profiles.confirm_delete" => "确认删除",
        "profiles.empty_hint" => "点右上角「导入配置」，从二维码、剪贴板、文件或 URL 导入",

        // Templates
        "templates.title" => "规则模板",
        "templates.new_btn" => "新建模板",
        "templates.name" => "模板名称",
        "templates.ruleset" => "规则集与路由配置",
        "templates.empty" => "暂无规则模板",

        // Logs
        "logs.title" => "运行日志",
        "logs.clear_btn" => "清空日志",
        "logs.auto_scroll" => "自动滚动",
        "logs.level.all" => "全部",
        "logs.level.debug" => "Debug",
        "logs.level.info" => "Info",
        "logs.level.warn" => "Warn",
        "logs.level.error" => "Error",
        "logs.empty" => "暂无日志",

        // Settings
        "settings.title" => "设置",
        "settings.core" => "内核",
        "settings.core_channel" => "更新通道",
        "settings.core_channel_desc" => "可在「稳定版」与「测试版」之间切换。测试版含新功能，也可能不够稳定。",
        "settings.channel.stable" => "稳定版",
        "settings.channel.beta" => "测试版",
        "settings.core_path" => "内核文件路径",
        "settings.github_proxy" => "GitHub 代理",
        "settings.github_proxy_desc" => "用于检查版本与下载 sing-box。直连失败时可尝试切换或使用自定义代理前缀。",
        "settings.github_proxy_hint" => "代理地址（可改，留空 = 直连）",
        "settings.inbounds" => "端口",
        "settings.inbounds_desc" => "套用运行模板或强制端口时，会用下方端口覆盖 mixed / Clash API。",
        "settings.mixed_port" => "混合端口",
        "settings.clash_api_host" => "Clash API 地址",
        "settings.clash_api_port" => "Clash API 端口",
        "settings.ui" => "界面",
        "settings.language" => "界面语言",
        "settings.lang.system" => "跟随系统",
        "settings.lang.zh_hans" => "简体中文",
        "settings.lang.zh_hant" => "繁體中文",
        "settings.lang.en" => "English",
        "settings.theme" => "主题模式",
        "settings.theme.system" => "跟随系统",
        "settings.theme.light" => "浅色",
        "settings.theme.dark" => "深色",
        "settings.tray_enabled" => "系统托盘",
        "settings.tray_enabled_desc" => "显示托盘图标，可快速启停与切换配置",
        "settings.close_to_tray" => "关闭窗口到托盘",
        "settings.close_to_tray_desc" => "点击关闭时隐藏到托盘，而不是退出",
        "settings.launch_at_startup" => "开机自启动",
        "settings.launch_at_startup_desc" => "登录系统后自动打开 SingPanel（Windows 注册表 / macOS 登录项）",
        "settings.subscription" => "订阅",
        "settings.auto_update_subs" => "订阅自动更新",
        "settings.auto_update_subs_desc" => "远程配置按间隔自动拉取（最短 15 分钟）",
        "settings.auto_update_interval" => "更新间隔（分钟）",
        "settings.assemble" => "规则模板",
        "settings.assemble_desc" => "导入订阅时可自动套用预设分流规则与本地端口，默认保留订阅原样。",
        "settings.default_assemble" => "导入时默认套用规则模板",
        "settings.default_assemble_desc" => "关闭则保留订阅原文",
        "settings.force_ports" => "统一本地端口",
        "settings.force_ports_desc" => "套用模板时使用上方设置的代理端口",
        "settings.default_template" => "默认规则模板",
        "settings.tailscale" => "Tailscale",
        "settings.tailscale_desc" => "在首页一键启停。无需修改订阅文件，开启后代理与私有内网同时生效。可留空 Auth Key 并在启动后通过浏览器快捷授权。",
        "settings.ts_tag" => "节点标签",
        "settings.ts_auth" => "Auth Key（可留空使用网页授权）",
        "settings.ts_auth_hint" => "留空时，启动后可点击登录链接在浏览器中授权",
        "settings.ts_hostname" => "设备名称（可选）",
        "settings.ts_exit" => "出口节点（可选）",
        "settings.ts_routes" => "通告的子网网段",
        "settings.ts_domain" => "Tailscale 专属域名后缀",
        "settings.ts_accept_routes" => "接受子网路由",
        "settings.ts_inject_dns" => "启用 MagicDNS",
        "settings.ts_inject_dns_desc" => "自动解析 Tailscale 内网设备名称",
        "settings.ts_preferred" => "优先匹配 Tailscale 路由",
        "settings.ts_replace" => "覆盖配置中原有的 Tailscale",
        "settings.ts_sysif" => "使用系统网卡接口",
        "settings.ts_sysif_desc" => "高级选项，通常保持关闭",
        "settings.other" => "其他",
        "settings.disclaimer" => "免责声明",
        "settings.disclaimer_desc" => "开源软件使用约定",
        "settings.about" => "关于我们",
        "settings.about_desc" => "版本、内核与参考项目",
        "settings.loading_prefs" => "正在读取本机设置…",

        // Tray
        "tray.show_window" => "显示主窗口",
        "tray.start_core" => "启动内核",
        "tray.stop_core" => "停止内核",
        "tray.quit" => "退出 SingPanel",

        _ => key,
    }
}

fn translate_zh_hant<'a>(key: &'a str) -> &'a str {
    match key {
        // Navigation
        "nav.home" => "首頁",
        "nav.proxies" => "代理",
        "nav.connections" => "連線",
        "nav.profiles" => "設定檔",
        "nav.templates" => "範本",
        "nav.logs" => "記錄",
        "nav.settings" => "設定",

        // Common
        "common.save" => "儲存",
        "common.cancel" => "取消",
        "common.confirm" => "確認",
        "common.delete" => "刪除",
        "common.edit" => "編輯",
        "common.copy" => "複製",
        "common.copied" => "已複製",
        "common.success" => "成功",
        "common.failed" => "失敗",
        "common.refresh" => "重新整理",
        "common.close" => "關閉",
        "common.open" => "開啟",
        "common.search" => "搜尋",
        "common.retry" => "重試",
        "common.loading" => "載入中...",
        "common.saving" => "儲存中…",
        "common.custom" => "自訂",
        "common.unknown" => "未知",
        "common.enabled" => "已啟用",
        "common.disabled" => "已停用",

        // Home
        "home.status.running" => "執行中",
        "home.status.stopped" => "未執行",
        "home.status.starting" => "正在啟動…",
        "home.status.stopping" => "正在停止…",
        "home.status.reloading" => "正在重新載入設定…",
        "home.btn.start" => "啟動",
        "home.btn.stop" => "停止",
        "home.card.traffic" => "流量統計",
        "home.card.upload" => "上傳",
        "home.card.download" => "下載",
        "home.card.memory" => "核心記憶體",
        "home.card.net_detect" => "網路檢測",
        "home.card.system_proxy" => "系統代理",
        "home.card.tun_mode" => "TUN 模式",
        "home.card.active_profile" => "目前設定檔",
        "home.card.no_active_profile" => "未選擇設定檔",
        "home.tun.elevated" => "虛擬網卡授權",
        "home.tun.elevate_btn" => "授權 TUN",
        "home.tun.elevated_ready" => "已獲 TUN 權限",
        "home.detect.click_to_test" => "點擊重新檢測",
        "home.detect.testing" => "檢測中...",
        "home.detect.domestic" => "連通測試",
        "home.detect.intl" => "海外節點出口",
        "home.tailscale.card" => "Tailscale",
        "home.tailscale.not_logged_in" => "未登入 Tailscale",
        "home.tailscale.connected" => "已連線到 Tailnet",
        "home.tailscale.login_link" => "點擊登入 Tailscale",

        // Proxies
        "proxies.title" => "代理策略組",
        "proxies.test_delay" => "測速",
        "proxies.testing_delay" => "測速中...",
        "proxies.search_placeholder" => "搜尋節點名稱...",
        "proxies.sort.default" => "預設",
        "proxies.sort.delay" => "延遲",
        "proxies.sort.name" => "名稱",
        "proxies.empty" => "暫無代理節點",
        "proxies.not_running_hint" => "核心未啟動，請先啟動核心",

        // Connections
        "connections.title" => "作用中連線",
        "connections.close_all" => "中斷全部",
        "connections.search_placeholder" => "搜尋主機 / 行程 / 節點...",
        "connections.count" => "總連線數",
        "connections.upload_speed" => "即時上傳",
        "connections.download_speed" => "即時下載",
        "connections.host" => "目標主機",
        "connections.network" => "網路通訊協定",
        "connections.process" => "行程",
        "connections.outbound" => "出口",
        "connections.rule" => "命中規則",
        "connections.empty" => "目前無作用中連線",
        "connections.sort.default" => "預設",
        "connections.sort.speed" => "即時速度",
        "connections.sort.traffic" => "累積流量",

        // Profiles
        "profiles.title" => "設定檔管理",
        "profiles.import_btn" => "匯入設定檔",
        "profiles.assemble_btn" => "聚合生成",
        "profiles.import_title" => "匯入訂閱或本機檔案",
        "profiles.import_name" => "設定檔名稱",
        "profiles.import_url" => "訂閱 URL / 本機路徑",
        "profiles.import_interval" => "自動更新間隔 (分鐘)",
        "profiles.import_submit" => "確認匯入",
        "profiles.update_all" => "更新全部訂閱",
        "profiles.update" => "更新",
        "profiles.updating" => "更新中...",
        "profiles.assembled_badge" => "聚合",
        "profiles.local_badge" => "本機",
        "profiles.remote_badge" => "訂閱",
        "profiles.sample_badge" => "示例",
        "profiles.empty" => "暫無設定檔",
        "profiles.view" => "查看設定",
        "profiles.view_empty" => "（空設定）",
        "profiles.set_current" => "設為目前",
        "profiles.current" => "目前設定",
        "profiles.badge_current" => "目前",
        "profiles.badge_download_only" => "僅下載",
        "profiles.update_sub" => "更新訂閱",
        "profiles.confirm_delete" => "確認刪除",
        "profiles.empty_hint" => "點右上角「匯入設定檔」，從二維碼、剪貼簿、檔案或 URL 匯入",

        // Templates
        "templates.title" => "規則範本",
        "templates.new_btn" => "新增範本",
        "templates.name" => "範本名稱",
        "templates.ruleset" => "規則集與路由設定",
        "templates.empty" => "暫無規則範本",

        // Logs
        "logs.title" => "執行記錄",
        "logs.clear_btn" => "清除記錄",
        "logs.auto_scroll" => "自動捲動",
        "logs.level.all" => "全部",
        "logs.level.debug" => "Debug",
        "logs.level.info" => "Info",
        "logs.level.warn" => "Warn",
        "logs.level.error" => "Error",
        "logs.empty" => "暫無記錄",

        // Settings
        "settings.title" => "設定",
        "settings.core" => "核心",
        "settings.core_channel" => "更新通道",
        "settings.core_channel_desc" => "可在「穩定版」與「測試版」之間切換。測試版包含新功能，也可能不夠穩定。",
        "settings.channel.stable" => "穩定版",
        "settings.channel.beta" => "測試版",
        "settings.core_path" => "核心檔案路徑",
        "settings.github_proxy" => "GitHub 代理",
        "settings.github_proxy_desc" => "僅用於檢查版本和下載官方核心。直接連線失敗時請更換，或改為自己的反向代理前綴。",
        "settings.github_proxy_hint" => "代理位址（可變更，留空 = 直接連線）",
        "settings.inbounds" => "連接埠",
        "settings.inbounds_desc" => "套用執行範本或強制連接埠時，會用下方連接埠覆蓋 mixed / Clash API。",
        "settings.mixed_port" => "混合連接埠",
        "settings.clash_api_host" => "Clash API 位址",
        "settings.clash_api_port" => "Clash API 連接埠",
        "settings.ui" => "介面",
        "settings.language" => "介面語言",
        "settings.lang.system" => "跟隨系統",
        "settings.lang.zh_hans" => "简体中文",
        "settings.lang.zh_hant" => "繁體中文",
        "settings.lang.en" => "English",
        "settings.theme" => "主題外觀",
        "settings.theme.system" => "跟隨系統",
        "settings.theme.light" => "亮色",
        "settings.theme.dark" => "暗色",
        "settings.tray_enabled" => "系統匣",
        "settings.tray_enabled_desc" => "顯示系統匣圖示，可快速啟停與切換設定檔",
        "settings.close_to_tray" => "關閉視窗到系統匣",
        "settings.close_to_tray_desc" => "按一下關閉時隱藏到系統匣，而非結束",
        "settings.launch_at_startup" => "開機自動啟動",
        "settings.launch_at_startup_desc" => "登入系統後自動開啟 SingPanel（Windows 登錄檔 / macOS 登入項目）",
        "settings.subscription" => "訂閱",
        "settings.auto_update_subs" => "自動更新訂閱",
        "settings.auto_update_subs_desc" => "遠端設定按間隔自動擷取（最短 15 分鐘）",
        "settings.auto_update_interval" => "更新間隔（分鐘）",
        "settings.assemble" => "裝配",
        "settings.assemble_desc" => "匯入訂閱時可選「套用範本」：用統一的本機設定承載節點。預設仍按原文儲存。",
        "settings.default_assemble" => "匯入時預設套用範本",
        "settings.default_assemble_desc" => "關閉則按訂閱原文儲存",
        "settings.force_ports" => "統一本地連接埠",
        "settings.force_ports_desc" => "套用範本時使用上方混合連接埠",
        "settings.default_template" => "預設執行範本",
        "settings.tailscale" => "Tailscale",
        "settings.tailscale_desc" => "在首頁 Tailscale 卡片用開關開啟 / 關閉。此處只改應用層級節點參數，不改訂閱檔案。Auth Key 可留空：開啟並啟動後瀏覽器授權即可。官方核心 ≥1.13 即可加入 tailnet；1.14+ 才有 preferred_by / 單標籤 MagicDNS。未列出的欄位儲存時會保留。",
        "settings.ts_tag" => "節點名稱（內部標記）",
        "settings.ts_auth" => "Auth Key（可留空 = 瀏覽器授權）",
        "settings.ts_auth_hint" => "留空合法：記錄裡會出現 login.tailscale.com 連結。一次性 Key 用過後會失效。",
        "settings.ts_hostname" => "裝置顯示名（可留空）",
        "settings.ts_exit" => "出口節點（可留空）",
        "settings.ts_routes" => "廣播的區域網路網段",
        "settings.ts_domain" => "走 Tailscale 的網域名稱後綴",
        "settings.ts_accept_routes" => "接受子網路路由",
        "settings.ts_inject_dns" => "啟用 MagicDNS",
        "settings.ts_inject_dns_desc" => "解析 tailnet 內裝置名稱；1.13 用 ip_accept_any，1.14+ 用 preferred_by",
        "settings.ts_preferred" => "優先走 Tailscale 路由",
        "settings.ts_replace" => "取代訂閱裡已有的 Tailscale",
        "settings.ts_sysif" => "使用系統網卡介面",
        "settings.ts_sysif_desc" => "進階選項，一般保持關閉",
        "settings.other" => "其他",
        "settings.disclaimer" => "免責聲明",
        "settings.disclaimer_desc" => "開放原始碼軟體使用約定",
        "settings.about" => "關於我們",
        "settings.about_desc" => "版本、核心與參考專案",
        "settings.loading_prefs" => "正在讀取本機設定…",

        // Tray
        "tray.show_window" => "顯示主視窗",
        "tray.start_core" => "啟動核心",
        "tray.stop_core" => "停止核心",
        "tray.quit" => "結束 SingPanel",

        _ => translate_zh_hans(key),
    }
}

fn translate_en<'a>(key: &'a str) -> &'a str {
    match key {
        // Navigation
        "nav.home" => "Home",
        "nav.proxies" => "Proxies",
        "nav.connections" => "Connections",
        "nav.profiles" => "Profiles",
        "nav.templates" => "Templates",
        "nav.logs" => "Logs",
        "nav.settings" => "Settings",

        // Common
        "common.save" => "Save",
        "common.cancel" => "Cancel",
        "common.confirm" => "Confirm",
        "common.delete" => "Delete",
        "common.edit" => "Edit",
        "common.copy" => "Copy",
        "common.copied" => "Copied",
        "common.success" => "Success",
        "common.failed" => "Failed",
        "common.refresh" => "Refresh",
        "common.close" => "Close",
        "common.open" => "Open",
        "common.search" => "Search",
        "common.retry" => "Retry",
        "common.loading" => "Loading...",
        "common.saving" => "Saving…",
        "common.custom" => "Custom",
        "common.unknown" => "Unknown",
        "common.enabled" => "Enabled",
        "common.disabled" => "Disabled",

        // Home
        "home.status.running" => "Running",
        "home.status.stopped" => "Stopped",
        "home.status.starting" => "Starting…",
        "home.status.stopping" => "Stopping…",
        "home.status.reloading" => "Reloading config…",
        "home.btn.start" => "Start",
        "home.btn.stop" => "Stop",
        "home.card.traffic" => "Traffic",
        "home.card.upload" => "Upload",
        "home.card.download" => "Download",
        "home.card.memory" => "Core Memory",
        "home.card.net_detect" => "Network Status",
        "home.card.system_proxy" => "System Proxy",
        "home.card.tun_mode" => "TUN Mode",
        "home.card.active_profile" => "Active Profile",
        "home.card.no_active_profile" => "No Profile Selected",
        "home.tun.elevated" => "TUN Authorization",
        "home.tun.elevate_btn" => "Grant TUN",
        "home.tun.elevated_ready" => "TUN Authorized",
        "home.detect.click_to_test" => "Click to re-test",
        "home.detect.testing" => "Testing...",
        "home.detect.domestic" => "Direct",
        "home.detect.intl" => "Outbound",
        "home.tailscale.card" => "Tailscale",
        "home.tailscale.not_logged_in" => "Not logged into Tailscale",
        "home.tailscale.connected" => "Connected to Tailnet",
        "home.tailscale.login_link" => "Click to login to Tailscale",

        // Proxies
        "proxies.title" => "Proxy Groups",
        "proxies.test_delay" => "Speed Test",
        "proxies.testing_delay" => "Testing...",
        "proxies.search_placeholder" => "Search proxy nodes...",
        "proxies.sort.default" => "Default",
        "proxies.sort.delay" => "Latency",
        "proxies.sort.name" => "Name",
        "proxies.empty" => "No Proxies Available",
        "proxies.not_running_hint" => "Core is not running. Please start the core first.",

        // Connections
        "connections.title" => "Active Connections",
        "connections.close_all" => "Close All",
        "connections.search_placeholder" => "Search host / process / node...",
        "connections.count" => "Connections",
        "connections.upload_speed" => "Upload Speed",
        "connections.download_speed" => "Download Speed",
        "connections.host" => "Host",
        "connections.network" => "Network",
        "connections.process" => "Process",
        "connections.outbound" => "Outbound",
        "connections.rule" => "Rule",
        "connections.empty" => "No active connections",
        "connections.sort.default" => "Default",
        "connections.sort.speed" => "Speed",
        "connections.sort.traffic" => "Traffic",

        // Profiles
        "profiles.title" => "Profile Management",
        "profiles.import_btn" => "Import Profile",
        "profiles.assemble_btn" => "Assemble",
        "profiles.import_title" => "Import Subscription or File",
        "profiles.import_name" => "Profile Name",
        "profiles.import_url" => "Subscription URL / File Path",
        "profiles.import_interval" => "Auto Update Interval (min)",
        "profiles.import_submit" => "Import",
        "profiles.update_all" => "Update All",
        "profiles.update" => "Update",
        "profiles.updating" => "Updating...",
        "profiles.assembled_badge" => "Assembled",
        "profiles.local_badge" => "Local",
        "profiles.remote_badge" => "Remote",
        "profiles.sample_badge" => "Sample",
        "profiles.empty" => "No profiles found",
        "profiles.view" => "View Config",
        "profiles.view_empty" => "(empty config)",
        "profiles.set_current" => "Set as current",
        "profiles.current" => "Current",
        "profiles.badge_current" => "Current",
        "profiles.badge_download_only" => "Download only",
        "profiles.update_sub" => "Update",
        "profiles.confirm_delete" => "Confirm delete",
        "profiles.empty_hint" => "Use Import to add a QR, clipboard, file, or URL profile",

        // Templates
        "templates.title" => "Rule Templates",
        "templates.new_btn" => "New Template",
        "templates.name" => "Template Name",
        "templates.ruleset" => "Rulesets & Routing",
        "templates.empty" => "No templates found",

        // Logs
        "logs.title" => "Runtime Logs",
        "logs.clear_btn" => "Clear Logs",
        "logs.auto_scroll" => "Auto Scroll",
        "logs.level.all" => "All",
        "logs.level.debug" => "Debug",
        "logs.level.info" => "Info",
        "logs.level.warn" => "Warn",
        "logs.level.error" => "Error",
        "logs.empty" => "No logs recorded",

        // Settings
        "settings.title" => "Settings",
        "settings.core" => "Core",
        "settings.core_channel" => "Update Channel",
        "settings.core_channel_desc" => "Switch between Stable and Beta channels. Beta has newer features but may be less stable.",
        "settings.channel.stable" => "Stable",
        "settings.channel.beta" => "Beta",
        "settings.core_path" => "Core Binary Path",
        "settings.github_proxy" => "GitHub Proxy",
        "settings.github_proxy_desc" => "Used only for version checking and official core downloads. Change if direct connection fails.",
        "settings.github_proxy_hint" => "Proxy URL (leave empty = Direct)",
        "settings.inbounds" => "Ports",
        "settings.inbounds_desc" => "When applying templates or forcing ports, mixed and Clash API ports will be overridden by the values below.",
        "settings.mixed_port" => "Mixed Port",
        "settings.clash_api_host" => "Clash API Host",
        "settings.clash_api_port" => "Clash API Port",
        "settings.ui" => "Interface",
        "settings.language" => "Language",
        "settings.lang.system" => "Follow System",
        "settings.lang.zh_hans" => "简体中文",
        "settings.lang.zh_hant" => "繁體中文",
        "settings.lang.en" => "English",
        "settings.theme" => "Theme",
        "settings.theme.system" => "Follow System",
        "settings.theme.light" => "Light",
        "settings.theme.dark" => "Dark",
        "settings.tray_enabled" => "System Tray",
        "settings.tray_enabled_desc" => "Show tray icon for quick control and profile switching",
        "settings.close_to_tray" => "Close Window to Tray",
        "settings.close_to_tray_desc" => "Minimize to tray when window is closed instead of quitting",
        "settings.launch_at_startup" => "Launch at Startup",
        "settings.launch_at_startup_desc" => "Automatically start SingPanel on system login (Windows Registry / macOS LaunchAgents)",
        "settings.subscription" => "Subscription",
        "settings.auto_update_subs" => "Auto Update Subscriptions",
        "settings.auto_update_subs_desc" => "Automatically pull remote profiles periodically (min 15 mins)",
        "settings.auto_update_interval" => "Update Interval (minutes)",
        "settings.assemble" => "Assembly",
        "settings.assemble_desc" => "Optionally assemble on import: applies standard local settings to imported nodes. Default saves original content.",
        "settings.default_assemble" => "Assemble on Import by Default",
        "settings.default_assemble_desc" => "Disabled saves original subscription as-is",
        "settings.force_ports" => "Enforce Local Ports",
        "settings.force_ports_desc" => "Apply mixed port from above when assembling template",
        "settings.default_template" => "Default Runtime Template",
        "settings.tailscale" => "Tailscale",
        "settings.tailscale_desc" => "Enable/disable via switch on Home page. Changes application-level parameters without altering subscriptions. Official core >=1.13 supported.",
        "settings.ts_tag" => "Node Tag (internal)",
        "settings.ts_auth" => "Auth Key (leave empty = browser login)",
        "settings.ts_auth_hint" => "Empty is valid: login.tailscale.com URL will appear in logs. One-time keys expire after use.",
        "settings.ts_hostname" => "Device Display Name (optional)",
        "settings.ts_exit" => "Exit Node (optional)",
        "settings.ts_routes" => "Advertised LAN Routes",
        "settings.ts_domain" => "Tailscale Domain Suffix",
        "settings.ts_accept_routes" => "Accept Subnet Routes",
        "settings.ts_inject_dns" => "Enable MagicDNS",
        "settings.ts_inject_dns_desc" => "Resolves tailnet device names; uses ip_accept_any on 1.13, preferred_by on 1.14+",
        "settings.ts_preferred" => "Prefer Tailscale Routes",
        "settings.ts_replace" => "Replace Existing Tailscale in Profiles",
        "settings.ts_sysif" => "Use System Interface",
        "settings.ts_sysif_desc" => "Advanced option, keep disabled normally",
        "settings.other" => "Other",
        "settings.disclaimer" => "Disclaimer",
        "settings.disclaimer_desc" => "Open-source software usage terms",
        "settings.about" => "About",
        "settings.about_desc" => "Version, core, and references",
        "settings.loading_prefs" => "Reading local settings…",

        // Tray
        "tray.show_window" => "Show Window",
        "tray.start_core" => "Start Core",
        "tray.stop_core" => "Stop Core",
        "tray.quit" => "Quit SingPanel",

        _ => key,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_language_codes() {
        assert_eq!(Language::from_code("zh-Hans"), Language::ZhHans);
        assert_eq!(Language::from_code("zh-CN"), Language::ZhHans);
        assert_eq!(Language::from_code("zh-Hant"), Language::ZhHant);
        assert_eq!(Language::from_code("zh-TW"), Language::ZhHant);
        assert_eq!(Language::from_code("zh-HK"), Language::ZhHant);
        assert_eq!(Language::from_code("en"), Language::En);
        assert_eq!(Language::from_code("en-US"), Language::En);
    }

    #[test]
    fn set_and_get_language() {
        set_current_lang(Language::En);
        assert_eq!(current_lang(), Language::En);
        assert_eq!(tr("nav.home"), "Home");

        set_current_lang(Language::ZhHant);
        assert_eq!(current_lang(), Language::ZhHant);
        assert_eq!(tr("nav.home"), "首頁");

        set_current_lang(Language::ZhHans);
        assert_eq!(current_lang(), Language::ZhHans);
        assert_eq!(tr("nav.home"), "首页");
    }

    #[test]
    fn test_all_nav_keys_translated() {
        let keys = [
            "nav.home",
            "nav.proxies",
            "nav.connections",
            "nav.profiles",
            "nav.templates",
            "nav.logs",
            "nav.settings",
        ];
        for lang in Language::ALL {
            for key in keys {
                let text = tr_lang(lang, key);
                assert_ne!(text, key, "Key {key} missing in {lang:?}");
            }
        }
    }

    #[test]
    fn test_common_settings_keys() {
        let keys = [
            "settings.title",
            "settings.language",
            "settings.launch_at_startup",
            "settings.close_to_tray",
            "settings.core",
            "settings.inbounds",
            "settings.ui",
            "settings.subscription",
            "settings.assemble",
            "settings.tailscale",
            "settings.other",
            "home.status.running",
            "proxies.title",
            "connections.title",
            "profiles.title",
            "profiles.view",
            "profiles.set_current",
            "profiles.current",
            "profiles.update_sub",
            "profiles.confirm_delete",
            "common.copy",
            "common.close",
            "common.delete",
            "common.cancel",
            "templates.title",
            "logs.title",
            "tray.show_window",
            "tray.quit",
        ];
        for lang in Language::ALL {
            for key in keys {
                let text = tr_lang(lang, key);
                assert_ne!(text, key, "Key {key} missing in {lang:?}");
            }
        }
    }
}
