fn main() {
    // Add win32 resources to the EXE file
    #[cfg(target_os = "windows")]
    {
        let res = winres::WindowsResource::new();
        res.set("ProductVersion", env!("CARGO_PKG_VERSION"));
        res.set("FileVersion", env!("CARGO_PKG_VERSION"));
        res.compile().unwrap();
    }
}
