# DoH Proxy Docker Container

Docker container for running the DoH Proxy on Linux.

## Quick Start

Pull the latest image from Docker Hub:

```bash
docker pull whiskeyjay/doh-proxy:latest
```

Run with default settings (Cloudflare + Google DNS):

```bash
docker run -d \
  --name doh-proxy \
  -p 5053:5053/udp \
  -p 5053:5053/tcp \
  --restart unless-stopped \
  whiskeyjay/doh-proxy:latest
```

## Configuration

### Custom Configuration via Environment Variables

```bash
docker run -d \
  --name doh-proxy \
  -p 5053:5053/udp \
  -p 5053:5053/tcp \
  --restart unless-stopped \
  -e LISTEN_ADDR=0.0.0.0:5053 \
  -e DOH_SERVERS=https://9.9.9.9/dns-query,https://94.140.14.14/dns-query \
  -e TIMEOUT_SECS=10 \
  -e CACHE_SIZE=50000 \
  -e VERBOSE=true \
  whiskeyjay/doh-proxy:latest
```

### Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `LISTEN_ADDR` | Address to listen for DNS queries | `0.0.0.0:5053` |
| `DOH_SERVERS` | Comma-separated list of DoH servers | `https://1.1.1.1/dns-query,https://8.8.8.8/dns-query` |
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
  --restart unless-stopped \
  -e DOH_SERVERS=https://1.1.1.2/dns-query \
  whiskeyjay/doh-proxy:latest
```

**Cloudflare Family-Friendly (blocks malware + adult content):**

```bash
docker run -d \
  --name doh-proxy-family \
  -p 5053:5053/udp \
  -p 5053:5053/tcp \
  --restart unless-stopped \
  -e DOH_SERVERS=https://1.1.1.3/dns-query \
  whiskeyjay/doh-proxy:latest
```

**Multiple DoH Servers with Fallback (Recommended for Pi-hole):**

```bash
docker run -d \
  --name doh-proxy \
  -p 5053:5053/udp \
  -p 5053:5053/tcp \
  --restart unless-stopped \
  -e DOH_SERVERS=https://9.9.9.9/dns-query,https://94.140.14.14/dns-query,https://1.1.1.1/dns-query \
  whiskeyjay/doh-proxy:latest
```

**Verbose Logging:**

```bash
docker run -d \
  --name doh-proxy \
  -p 5053:5053/udp \
  -p 5053:5053/tcp \
  --restart unless-stopped \
  -e VERBOSE=true \
  whiskeyjay/doh-proxy:latest
```

## Docker Compose

Create a `docker-compose.yml`:

```yaml
services:
  doh-proxy:
    image: whiskeyjay/doh-proxy:latest
    container_name: doh-proxy
    restart: unless-stopped
    ports:
      - "5053:5053/udp"
      - "5053:5053/tcp"
    environment:
      LISTEN_ADDR: "0.0.0.0:5053"
      DOH_SERVERS: "https://1.1.1.1/dns-query,https://8.8.8.8/dns-query"
      TIMEOUT_SECS: "5"
      CACHE_SIZE: "10000"
      VERBOSE: "false"
```

Then run:

```bash
docker compose up -d
```

## Testing

Once running, test the DNS proxy:

```bash
# Using dig
dig @127.0.0.1 -p 5053 example.com

# Using nslookup
nslookup example.com 127.0.0.1 -port=5053

# Test DNSSEC support
dig @127.0.0.1 -p 5053 example.com +dnssec
```

## Viewing Logs

```bash
docker logs doh-proxy

# Follow logs
docker logs -f doh-proxy
```

## Management

### Viewing Logs

```bash
# View logs
docker logs doh-proxy

# Follow logs in real-time
docker logs -f doh-proxy

# View logs with verbose output
docker logs -f doh-proxy --tail 100
```

### Stopping and Removing

```bash
# Stop
docker stop doh-proxy

# Remove
docker rm doh-proxy

# Stop and remove in one command
docker rm -f doh-proxy
```

### Updating to Latest Version

```bash
# Pull latest image
docker pull whiskeyjay/doh-proxy:latest

# Stop and remove old container
docker rm -f doh-proxy

# Start new container with same settings
docker run -d \
  --name doh-proxy \
  -p 5053:5053/udp \
  -p 5053:5053/tcp \
  --restart unless-stopped \
  whiskeyjay/doh-proxy:latest
```

## Pi-hole Integration

Configure Pi-hole to use this DoH proxy as its upstream DNS server:

1. Run the DoH proxy container as shown above
2. In Pi-hole admin interface, go to **Settings** → **DNS**
3. Uncheck all upstream DNS servers
4. Add custom upstream DNS server: `127.0.0.1#5053`
5. Optionally enable DNSSEC in Pi-hole
6. Save settings

## Notes

- **Port 5053** is used instead of standard DNS port 53 to avoid conflicts and permission issues
- **DNSSEC Support**: Full DNSSEC validation is supported - compatible with Pi-hole
- **IP-based DoH servers**: Using IP addresses (e.g., `1.1.1.1`) instead of hostnames avoids DNS bootstrap problems
- If port 5053 is already in use, change the port mapping:

  ```bash
  docker run -d -p 5054:5053/udp -p 5054:5053/tcp whiskeyjay/doh-proxy:latest
  ```

- The image is based on `debian:trixie-slim` for minimal size (~50MB)
- SSL/TLS certificates are included for secure HTTPS DoH requests

## Available DoH Servers (by IP)

| Provider | IP Address | Description |
|----------|------------|-------------|
| Cloudflare | `https://1.1.1.1/dns-query` | Standard DNS |
| Cloudflare | `https://1.1.1.2/dns-query` | Malware blocking |
| Cloudflare | `https://1.1.1.3/dns-query` | Malware + adult content blocking |
| Google | `https://8.8.8.8/dns-query` | Standard DNS |
| Quad9 | `https://9.9.9.9/dns-query` | Privacy-focused, malware blocking |
| AdGuard | `https://94.140.14.14/dns-query` | Ad blocking |
| OpenDNS | `https://146.112.41.2/dns-query` | Standard DNS |
