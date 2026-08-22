fn main() {
    println!("cargo:rerun-if-changed=assets/app_icon.ico");
    println!("cargo:rerun-if-env-changed=SINGPANEL_VERSION");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let ver = std::env::var("SINGPANEL_VERSION").unwrap_or_else(|_| {
        env!("CARGO_PKG_VERSION").to_string()
    });
    let mut res = winres::WindowsResource::new();
    res.set_icon("assets/app_icon.ico");
    res.set("ProductName", "SingPanel");
    res.set("FileDescription", "SingPanel");
    res.set("OriginalFilename", "singpanel-gpui.exe");
    res.set("ProductVersion", &ver);
    res.set("FileVersion", &ver);
    res.compile().expect("embed Windows icon");
}
