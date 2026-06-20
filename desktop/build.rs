//! Build script — embeds Windows resources (icon) into the executable.

fn main() {
    #[cfg(windows)]
    {
        if let Err(e) = winres::WindowsResource::new()
            .set_icon("assets/icon.ico")
            .set("FileDescription", "pw4you - 文件夹保险箱")
            .set("ProductName", "pw4you")
            .set("LegalCopyright", "pw4you")
            .set("OriginalFilename", "pw4you.exe")
            .compile()
        {
            println!("cargo:warning=Failed to compile windows resources: {}", e);
        }
    }
}
