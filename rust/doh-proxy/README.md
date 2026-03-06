# DoH Proxy

A DNS proxy server that translates traditional DNS queries to DNS-over-HTTPS (DoH) requests.

## Features

- **DNS Server**: Listens for traditional DNS queries (both UDP and TCP) on any network interface
- **DoH Client**: Forwards queries to configurable DoH upstream servers using HTTPS
- **DNSSEC Support**: Full DNSSEC validation support - compatible with Pi-hole and other DNSSEC-aware DNS servers
- **DNS Caching**: Intelligent caching with TTL-based expiration for fast repeated queries
- **Automatic Fallback**: If one DoH server fails, automatically tries the next one
- **Fully Configurable**: All options available via command-line arguments
- **Standards Compliant**: Uses DNS wireformat for DoH queries (RFC 8484)

## Prerequisites

- Rust 1.70 or higher
- Administrator/root privileges (required to bind to port 53)

## Installation

1. Clone or download this repository
2. Build the project:
   ```bash
   cargo build --release
   ```

> **Note**: Port 53 is often already in use. If you get "address already in use" errors, either:
> - Use a different port: `doh-proxy -l 127.0.0.1:5353`
> - Or stop conflicting services (see Troubleshooting section below)

## Configuration

The proxy is configured via command-line arguments:

```
Usage: doh-proxy [OPTIONS]

Options:
  -l, --listen-addr <LISTEN_ADDR>
          Address to listen for DNS queries [default: 0.0.0.0:53]
  -s, --doh-server <DOH_SERVERS>...
          DoH upstream servers (can be specified multiple times)
          [default: https://1.1.1.1/dns-query https://8.8.8.8/dns-query]
  -t, --timeout-secs <TIMEOUT_SECS>
          Timeout for DoH queries in seconds [default: 5]
  -c, --cache-size <CACHE_SIZE>
          Maximum number of cached DNS responses (0 to disable) [default: 10000]
  -v, --verbose
          Enable verbose logging (shows cache hits/misses)
  -h, --help
          Print help
  -V, --version
          Print version
```

### Examples

**Use default settings (Cloudflare and Google):**
```bash
doh-proxy
```

**For Pi-hole with DNSSEC (recommended):**
```bash
doh-proxy -s https://9.9.9.9/dns-query
```

**Use custom DoH servers:**
```bash
doh-proxy -s https://9.9.9.9/dns-query -s https://94.140.14.14/dns-query
```

**Listen on localhost only:**
```bash
doh-proxy -l 127.0.0.1:53
```

**Use a different port:**
```bash
doh-proxy -l 0.0.0.0:5353
```

**Listen on specific interface:**
```bash
doh-proxy -l 192.168.1.10:53
```

**Custom timeout:**
```bash
doh-proxy -t 10
```

## Usage

### Linux/macOS

Run with sudo (required for port 53):

```bash
sudo ./target/release/doh-proxy

# With custom options
sudo ./target/release/doh-proxy \
  -s https://dns.quad9.net/dns-query \
  -t 10

# Use Quad9 DNS
sudo ./target/release/doh-proxy -s https://dns.quad9.net/dns-query

# Use multiple custom servers with fallback
sudo ./target/release/doh-proxy \
  -s https://dns.quad9.net/dns-query \
  -s https://dns.adguard.com/dns-query \
  -s https://cloudflare-dns.com/dns-query

# Run on alternative port (no sudo required)
./target/release/doh-proxy -l 0.0.0.0:5353

# Disable caching (not recommended)
sudo ./target/release/doh-proxy -c 0

# Enable verbose logging to see cache performance
sudo ./target/release/doh-proxy -v

# All options together
sudo ./target/release/doh-proxy \
  -l 0.0.0.0:53 \
  -s https://dns.quad9.net/dns-query \
  -t 10
```

### Windows

Run as Administrator (required for port 53):

```powershell
# Use defaults (Cloudflare + Google DNS)
.\target\release\doh-proxy.exe

# Use Quad9 DNS
.\target\release\doh-proxy.exe -s https://dns.quad9.net/dns-query

# Use multiple custom servers with fallback
.\target\release\doh-proxy.exe `
  -s https://dns.quad9.net/dns-query `
  -s https://dns.adguard.com/dns-query `
  -s https://cloudflare-dns.com/dns-query

# Listen on localhost only
.\target\release\doh-proxy.exe -l 127.0.0.1:53

# Run on alternative port (no admin required)
.\target\release\doh-proxy.exe -l 0.0.0.0:5353

# Listen on specific network interface
.\target\release\doh-proxy.exe -l 192.168.1.10:53

# Disable caching (not recommended)
.\target\release\doh-proxy.exe -c 0

# Increase cache size for busy networks
.\target\release\doh-proxy.exe -c 50000

# Enable verbose logging to see cache performance
.\target\release\doh-proxy.exe -v

# All options together
.\target\release\doh-proxy.exe `
  -l 0.0.0.0:53 `
  -s https://dns.quad9.net/dns-query `
  -t 10
```

### Testing

Once running, you can test the proxy with standard DNS tools:

**Using dig (Linux/macOS):**
```bash
dig @127.0.0.1 example.com
```

**Using nslookup:**
```bash
nslookup example.com 127.0.0.1
```

**From another machine on the network:**
```bash
nslookup example.com 192.168.1.10
```

**Using PowerShell (Windows):**
```powershell
Resolve-DnsName example.com -Server 127.0.0.1
```

## How It Works

1. **DNS Query Reception**: The proxy listens for incoming DNS queries (UDP/TCP)
2. **Cache Lookup**: Checks if the response is cached and still valid (based on TTL)
3. **Query Translation**: If not cached, converts the DNS query to DoH format (DNS wireformat over HTTPS POST)
4. **DoH Request**: Sends the query to configured upstream DoH servers
5. **Response Translation**: Receives the DoH response and converts it back to standard DNS format
6. **Caching**: Stores the response in cache with automatic TTL-based expiration
7. **Response Delivery**: Sends the DNS response back to the original client

### DNS Caching

The proxy implements intelligent DNS caching to dramatically improve performance for repeated queries:

- **TTL-Aware**: Respects the Time-To-Live values from DNS records
- **Automatic Expiration**: Cached entries are automatically removed when they expire
- **Per-Query Caching**: Each unique domain/record type combination is cached separately
- **Configurable Size**: Default 10,000 entries, adjustable via `-c` flag
- **Smart TTL Limits**: Caps TTL at 1 hour max, uses 5 minute default for records without TTL

**Performance Impact**: Cached queries return in microseconds vs. 20-100ms for upstream DoH queries.

## Architecture

```
Client (dig, nslookup, etc.)
    ↓ Traditional DNS Query (UDP/TCP)
DoH Proxy
    ↓ Check Cache
    ├─ Cache HIT → Return cached response (fast!)
    └─ Cache MISS ↓
         HTTPS POST with DNS wireformat
         ↓
DoH Server (Cloudflare, Google, etc.)
         ↓ HTTPS Response
DoH Proxy (stores in cache)
    ↓ Traditional DNS Response
Client
```

## Supported DoH Servers

The proxy works with any RFC 8484 compliant DoH server. Popular options include:

### Best for DNSSEC (Pi-hole compatible)
- **Quad9**: `https://9.9.9.9/dns-query` ⭐ **Recommended for Pi-hole**
- **Cloudflare**: `https://1.1.1.1/dns-query`
- **Cloudflare (Malware blocking)**: `https://1.1.1.2/dns-query`
- **Cloudflare (Malware + Adult content blocking)**: `https://1.1.1.3/dns-query`

### Other Options
- **Google**: `https://8.8.8.8/dns-query`
- **AdGuard**: `https://94.140.14.14/dns-query`
- **OpenDNS**: `https://146.112.41.2/dns-query`

> **Note**: Using IP addresses instead of hostnames avoids the DNS bootstrap problem (having to resolve the DoH server's hostname before you can make DNS queries).

> **Note for Pi-hole users**: Some DoH providers may return incomplete DNSSEC chains for TLD queries (e.g., `.com`, `.net`). If you see "Insecure DS reply" warnings in Pi-hole for TLDs, this is usually harmless but indicates the upstream DoH server's DNSSEC support is incomplete. Quad9 and Cloudflare generally provide the most complete DNSSEC validation. You can also disable DNSSEC in Pi-hole if these warnings are concerning: Settings → DNS → Uncheck "Use DNSSEC".

## Performance

### Cache Performance

With caching enabled (default), the proxy delivers excellent performance:

- **Cached queries**: < 1ms response time
- **Uncached queries**: 20-100ms (depends on upstream DoH server)
- **Cache hit rate**: Typically 60-90% for normal browsing

Use the `-v` flag to see cache hits/misses in real-time:

```bash
doh-proxy -v
```

You'll see log entries like:
- `Cache HIT for example.com` - Query served from cache
- `Cache MISS for example.com` - Query forwarded to upstream DoH
- `Cached response for example.com with TTL 300s` - Response stored

### DNSBench Results

When tested with DNSBench or similar tools:
- First query: Upstream latency (depends on DoH server distance)
- Repeated queries: Near-instant response from cache
- Overall throughput: Thousands of cached queries per second

## Logging

The application logs important events to stdout:
- Server startup information
- Configuration details
- Cache hits/misses (with `-v` flag)
- Query processing details (verbose mode)
- Errors and warnings

## Troubleshooting

### Port 53 Already in Use

This is the most common issue. Port 53 is typically in use by:
- **Windows**: DNS Client service (Dnscache)
- **Linux**: systemd-resolved
- **macOS**: mDNSResponder

**Solutions:**

**Option 1: Use a different port (easiest)**
```bash
# No root/admin rights needed
doh-proxy -l 127.0.0.1:5353
```
Then configure your system to use `127.0.0.1:5353` as DNS server.

**Option 2: Stop the conflicting service**

*Linux:*
```bash
sudo systemctl stop systemd-resolved
sudo systemctl disable systemd-resolved
```

*Windows (requires Administrator):*
```powershell
# Temporary (until reboot)
Stop-Service -Name "Dnscache"

# Permanent (not recommended)
Set-Service -Name "Dnscache" -StartupType Disabled
```

**Option 3: Configure the conflicting service to not use port 53**
- On Windows, you can configure DNS Client service to not bind to port 53
- On Linux, edit `/etc/systemd/resolved.conf` and set `DNSStubListener=no`

### Permission Denied

Binding to port 53 requires elevated privileges:
- **Windows**: Run as Administrator
- **Linux/macOS**: Run with `sudo`

### DoH Server Connection Issues

If queries are failing:
- Check your internet connection
- Verify the DoH server URLs are correct and accessible
- Try increasing timeout: `doh-proxy -t 10`
- Check firewall settings (ensure HTTPS/443 outbound is allowed)

## License

This project is provided as-is for educational and practical use.

## Contributing

Feel free to open issues or submit pull requests for improvements!
