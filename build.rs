#[cfg(windows)]
fn main() {
    let mut res = winresource::WindowsResource::new();
    res.set_icon("icon.ico")
        .set("FileDescription", "样品管理系统")
        .set("ProductName", "样品管理系统")
        .set("CompanyName", "WorkloadTool")
        .set("LegalCopyright", "WorkloadTool")
        .set("OriginalFilename", "workload-tool.exe");

    if let Err(err) = res.compile() {
        panic!("failed to compile Windows resources: {err}");
    }
}

#[cfg(not(windows))]
fn main() {}
