use winres::WindowsResource;

fn main() {
    // Add win32 resources to the EXE file
    if cfg!(target_os = "windows") {
        let mut res = WindowsResource::new();
        res.set("ProductName", "DoH Proxy");
        res.set("FileDescription", "DNS over HTTPS Proxy, a replacement for cloudflared proxy-dns command");
        res.set("CompanyName", "Jay Wang");
        res.set("OriginalFilename", "doh-proxy.exe");
        res.set("LegalCopyright", "MIT License");
        res.set("ProductVersion", env!("CARGO_PKG_VERSION"));
        res.set("FileVersion", env!("CARGO_PKG_VERSION"));
        res.compile().unwrap();
    }
}
