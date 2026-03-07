fn main() {
    // Add win32 resources to the EXE file
    #[cfg(target_os = "windows")]
    {
        let version = env!("CARGO_PKG_VERSION");
        let mut res = winres::WindowsResource::new();
        res.set("ProductVersion", version);
        res.set("FileVersion", version);
        res.compile().unwrap();
    }
}
