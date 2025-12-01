use anyhow::Result;
use clap::Parser;
use std::net::SocketAddr;

#[derive(Parser, Debug, Clone)]
#[command(name = "doh-proxy")]
#[command(author, version, about = "DNS-over-HTTPS proxy server", long_about = None)]
pub struct Config {
    /// Address to listen for DNS queries
    /// Use 0.0.0.0:53 for all interfaces, 127.0.0.1:53 for localhost only
    #[arg(short, long, default_value = "0.0.0.0:53")]
    pub listen_addr: String,

    /// DoH upstream servers (can be specified multiple times)
    /// Example: --doh-server https://cloudflare-dns.com/dns-query
    #[arg(short = 's', long = "doh-server", default_values_t = vec![
        "https://cloudflare-dns.com/dns-query".to_string(),
        "https://dns.google/dns-query".to_string(),
    ])]
    pub doh_servers: Vec<String>,

    /// Timeout for DoH queries in seconds
    #[arg(short, long, default_value = "5")]
    pub timeout_secs: u64,

    /// Maximum number of cached DNS responses (0 to disable caching)
    #[arg(short, long, default_value = "10000")]
    pub cache_size: u64,

    /// Enable verbose logging (shows cache hits/misses)
    #[arg(short, long)]
    pub verbose: bool,
}

impl Config {
    pub fn parse_listen_addr(&self) -> Result<SocketAddr> {
        Ok(self.listen_addr.parse()?)
    }
}
