use winres::WindowsResource;

fn main() {
    // Add win32 resources to the EXE file
    if cfg!(target_os = "windows") {
        let res = WindowsResource::new();
        res.compile().unwrap();
    }
}
