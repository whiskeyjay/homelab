# Quick Start Guide

## Important: Port 53 Conflicts

Port 53 is often already in use by system DNS services. You have two options:

### Option A: Use a different port (easiest)**

```powershell
# No admin rights needed for ports > 1024
.\target\release\doh-proxy.exe -l 127.0.0.1:5353
```

### Option B: Free up port 53 (Windows)**

```powershell
# Stop Windows DNS Client service (run as Administrator)
Stop-Service -Name "Dnscache"

# Then run the proxy
.\target\release\doh-proxy.exe
```

## Running the DoH Proxy

### Step 1: Build the project (if not already done)

**Windows (MSVC target):**

```powershell
# Build for x86_64-pc-windows-msvc (recommended for Windows)
cargo build --release --target x86_64-pc-windows-msvc
```

The executable will be located at:

```powershell
.\target\x86_64-pc-windows-msvc\release\doh-proxy.exe
```

**Standard build (any platform):**

```powershell
cargo build --release
```

The executable will be at:

```powershell
.\target\release\doh-proxy.exe
```

### Step 2: View available options

```powershell
.\target\release\doh-proxy.exe --help
```

### Step 3: Run as Administrator

The proxy needs to bind to port 53, which requires administrator privileges on Windows.

> **Note**: If you built with `--target x86_64-pc-windows-msvc`, use `.\target\x86_64-pc-windows-msvc\release\doh-proxy.exe` instead of `.\target\release\doh-proxy.exe` in the commands below.

**Run with default settings (Cloudflare + Google DNS):**

```powershell
.\target\release\doh-proxy.exe
# Or for MSVC target:
.\target\x86_64-pc-windows-msvc\release\doh-proxy.exe
```

**Run with custom DoH servers:**

```powershell
.\target\release\doh-proxy.exe -s https://dns.quad9.net/dns-query -s https://dns.adguard.com/dns-query
```

**Run on localhost only:**

```powershell
.\target\release\doh-proxy.exe -l 127.0.0.1:53
```

**Run on a different port (doesn't require admin):**

```powershell
.\target\release\doh-proxy.exe -l 0.0.0.0:5353
```

**Listen on specific network interface:**

```powershell
.\target\release\doh-proxy.exe -l 192.168.1.10:53
```

### Step 4: Test the proxy

Open another terminal and test with one of these methods:

**Using nslookup:**

```powershell
nslookup example.com 127.0.0.1
```

**Using Resolve-DnsName (PowerShell):**

```powershell
Resolve-DnsName example.com -Server 127.0.0.1
```

**Using dig (if installed):**

```bash
dig @127.0.0.1 example.com
```

## Example Output

When running successfully, you should see:

```text
DoH Proxy starting...
Listen address: 0.0.0.0:53
DoH servers: ["https://1.1.1.1/dns-query", "https://8.8.8.8/dns-query"]
Timeout: 5s
Cache size: 10000 entries
UDP DNS server listening on 0.0.0.0:53
TCP DNS server listening on 0.0.0.0:53
DoH Proxy is ready to serve DNS queries
```

## Command Line Options

```text
Options:
  -l, --listen-addr <LISTEN_ADDR>
          Address to listen for DNS queries [default: 0.0.0.0:53]
  -s, --doh-server <DOH_SERVERS>...
          DoH upstream servers (can be specified multiple times)
  -t, --timeout-secs <TIMEOUT_SECS>
          Timeout for DoH queries in seconds [default: 5]
  -c, --cache-size <CACHE_SIZE>
          Maximum number of cached DNS responses (0 to disable) [default: 10000]
  -v, --verbose
          Enable verbose logging (shows cache hits/misses)
  -h, --help
          Print help
```

## Troubleshooting

### "Address already in use" error

Another DNS service is using port 53. Options:

1. Stop the Windows DNS Client service
2. Use a different port: `doh-proxy.exe -l 0.0.0.0:5353`

### Permission denied

You must run as Administrator on Windows to bind to port 53.

### No response from queries

1. Check your internet connection
2. Verify firewall allows HTTPS traffic
3. Try a different DoH server in `config.toml`
