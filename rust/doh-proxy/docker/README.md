# DoH Proxy Docker Container

Docker container for running the DoH Proxy on Linux.

## Building

```bash
cd docker
chmod +x build.sh
./build.sh
```

Or build with custom name/tag:
```bash
./build.sh -n my-doh-proxy -t v1.0.0
```

Or build manually:
```bash
docker build -t doh-proxy:latest -f docker/dockerfile ..
```

## Running

### Basic Usage

Run with default settings (Cloudflare + Google DNS):
```bash
docker run -d \
  --name doh-proxy \
  -p 5053:5053/udp \
  -p 5053:5053/tcp \
  doh-proxy:latest
```

### Custom Configuration via Environment Variables

```bash
docker run -d \
  --name doh-proxy \
  -p 5053:5053/udp \
  -p 5053:5053/tcp \
  -e LISTEN_ADDR=0.0.0.0:5053 \
  -e DOH_SERVERS=https://dns.quad9.net/dns-query,https://dns.adguard.com/dns-query \
  -e TIMEOUT_SECS=10 \
  -e CACHE_SIZE=50000 \
  -e VERBOSE=true \
  doh-proxy:latest
```

### Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `LISTEN_ADDR` | Address to listen for DNS queries | `0.0.0.0:5053` |
| `DOH_SERVERS` | Comma-separated list of DoH servers | `https://cloudflare-dns.com/dns-query,https://dns.google/dns-query` |
| `TIMEOUT_SECS` | Timeout for DoH queries in seconds | `5` |
| `CACHE_SIZE` | Maximum number of cached DNS responses | `10000` |
| `VERBOSE` | Enable verbose logging (true/false) | `false` |

### Example Configurations

**Cloudflare with Malware Blocking:**
```bash
docker run -d \
  --name doh-proxy-secure \
  -p 5053:5053/udp \
  -p 5053:5053/tcp \
  -e DOH_SERVERS=https://security.cloudflare-dns.com/dns-query \
  doh-proxy:latest
```

**Cloudflare Family-Friendly (blocks malware + adult content):**
```bash
docker run -d \
  --name doh-proxy-family \
  -p 5053:5053/udp \
  -p 5053:5053/tcp \
  -e DOH_SERVERS=https://family.cloudflare-dns.com/dns-query \
  doh-proxy:latest
```

**Multiple DoH Servers with Fallback:**
```bash
docker run -d \
  --name doh-proxy \
  -p 5053:5053/udp \
  -p 5053:5053/tcp \
  -e DOH_SERVERS=https://dns.quad9.net/dns-query,https://dns.adguard.com/dns-query,https://cloudflare-dns.com/dns-query \
  doh-proxy:latest
```

**Verbose Logging:**
```bash
docker run -d \
  --name doh-proxy \
  -p 5053:5053/udp \
  -p 5053:5053/tcp \
  -e VERBOSE=true \
  doh-proxy:latest
```

## Docker Compose

Create a `docker-compose.yml`:

```yaml
version: '3.8'

services:
  doh-proxy:
    build:
      context: ..
      dockerfile: docker/dockerfile
    container_name: doh-proxy
    restart: unless-stopped
    ports:
      - "5053:5053/udp"
      - "5053:5053/tcp"
    environment:
      LISTEN_ADDR: "0.0.0.0:5053"
      DOH_SERVERS: "https://cloudflare-dns.com/dns-query,https://dns.google/dns-query"
      TIMEOUT_SECS: "5"
      CACHE_SIZE: "10000"
      VERBOSE: "false"
```

Then run:
```bash
docker-compose up -d
```

## Testing

Once running, test the DNS proxy:

```bash
# From host
nslookup example.com 127.0.0.1

# Using dig
dig @127.0.0.1 example.com
```

## Viewing Logs

```bash
docker logs doh-proxy

# Follow logs
docker logs -f doh-proxy
```

## Stopping and Removing

```bash
# Stop
docker stop doh-proxy

# Remove
docker rm doh-proxy

# Remove image
docker rmi doh-proxy:latest
```

## Notes

- Port 5053 is used instead of standard DNS port 53 to avoid conflicts and permission issues
- If port 5053 is already in use on your host, use a different port mapping:
  ```bash
  docker run -d -p 5054:5053/udp -p 5054:5053/tcp doh-proxy:latest
  ```
- The image is based on `debian:trixie-slim` for a small footprint
- SSL/TLS certificates are included for HTTPS DoH requests
