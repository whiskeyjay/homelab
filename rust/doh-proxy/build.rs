use winres::WindowsResource;

fn main() {
    // Add win32 resources to the EXE file
    if cfg!(target_os = "windows") {
        let mut res = WindowsResource::new();
        res.set("ProductVersion", env!("CARGO_PKG_VERSION"));
        res.set("FileVersion", env!("CARGO_PKG_VERSION"));
        res.compile().unwrap();
    }
}
