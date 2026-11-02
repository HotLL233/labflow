#[cfg(windows)]
fn main() {
    let version = std::env::var("CARGO_PKG_VERSION").expect("application version");
    let mut parts = version.split('.').map(|part| part.parse::<u64>().unwrap());
    let numeric = (parts.next().unwrap() << 48)
        | (parts.next().unwrap() << 32)
        | (parts.next().unwrap() << 16);
    winresource::WindowsResource::new()
        .set_icon("icon.ico")
        .set("ProductName", "labflow")
        .set("FileDescription", "labflow")
        .set("CompanyName", "HotLL")
        .set("OriginalFilename", "labflow.exe")
        .set("InternalName", "labflow")
        .set("LegalCopyright", "Copyright © 2026 HotLL")
        .set_version_info(winresource::VersionInfo::FILEVERSION, numeric)
        .set_version_info(winresource::VersionInfo::PRODUCTVERSION, numeric)
        .set_manifest(
            &include_str!("src/desktop.manifest").replace("@VERSION@", &format!("{version}.0")),
        )
        .append_rc_content(include_str!("src/desktop.rc"))
        .compile()
        .expect("compile labflow icon, version and desktop dialogs");
    for path in ["icon.ico", "src/desktop.manifest", "src/desktop.rc"] {
        println!("cargo:rerun-if-changed={path}");
    }
}

#[cfg(not(windows))]
fn main() {}
