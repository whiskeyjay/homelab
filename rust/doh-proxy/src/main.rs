mod config;
mod dns_handler;
mod doh_client;

use anyhow::Result;
use clap::Parser;
use config::Config;
use dns_handler::DnsHandler;
use doh_client::DohClient;
use hickory_server::ServerFuture;
use std::sync::Arc;
use tokio::net::{TcpListener, UdpSocket};
use tracing::{error, info};

#[tokio::main]
async fn main() -> Result<()> {
    // Parse command line arguments first
    let config = Config::parse();

    // Initialize tracing with appropriate level
    let log_level = if config.verbose {
        tracing::Level::DEBUG
    } else {
        tracing::Level::INFO
    };

    tracing_subscriber::fmt().with_max_level(log_level).init();

    info!("DoH Proxy starting...");
    info!("Listen address: {}", config.listen_addr);
    info!("DoH servers: {:?}", config.doh_servers);
    info!("Timeout: {}s", config.timeout_secs);
    if config.cache_size > 0 {
        info!("Cache: enabled ({} entries)", config.cache_size);
    } else {
        info!("Cache: disabled");
    }

    // Validate configuration
    let listen_addr = config.parse_listen_addr()?;
    config.validate_doh_servers()?;

    // Create DoH client
    let doh_client = Arc::new(DohClient::new(
        config.doh_servers,
        config.timeout_secs,
        config.cache_size,
    )?);

    // Spawn periodic cache stats logger
    if config.cache_size > 0 {
        let cache_client = Arc::clone(&doh_client);
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(300));
            interval.tick().await; // skip immediate first tick
            loop {
                interval.tick().await;
                if let Some(count) = cache_client.cache_entry_count() {
                    info!("Cache entries: {}", count);
                }
            }
        });
    }

    // Create DNS handler
    let handler = DnsHandler::new(doh_client);

    // Create DNS server
    let mut server = ServerFuture::new(handler);

    // Bind UDP socket
    let udp_socket = match UdpSocket::bind(&listen_addr).await {
        Ok(socket) => socket,
        Err(e) => {
            error!("Failed to bind UDP socket to {}: {}", listen_addr, e);
            if listen_addr.port() == 53 {
                error!("Port 53 is likely in use by another service.");
                error!("Solutions:");
                error!("  1. Run as Administrator/root");
                error!("  2. Stop conflicting DNS services (Windows DNS Client, systemd-resolved, etc.)");
                error!("  3. Use a different port: doh-proxy -l 127.0.0.1:5353");
            }
            return Err(e.into());
        }
    };
    info!("UDP DNS server listening on {}", listen_addr);
    server.register_socket(udp_socket);

    // Bind TCP listener
    let tcp_listener = match TcpListener::bind(&listen_addr).await {
        Ok(listener) => listener,
        Err(e) => {
            error!("Failed to bind TCP listener to {}: {}", listen_addr, e);
            return Err(e.into());
        }
    };
    info!("TCP DNS server listening on {}", listen_addr);
    server.register_listener(tcp_listener, std::time::Duration::from_secs(5));

    info!("DoH Proxy is ready to serve DNS queries");

    // Wait for shutdown signal or server completion
    tokio::select! {
        result = server.block_until_done() => {
            match result {
                Ok(_) => info!("Server shutdown gracefully"),
                Err(e) => {
                    error!("Server error: {}", e);
                    return Err(e.into());
                }
            }
        }
        _ = shutdown_signal() => {
            info!("Received shutdown signal, shutting down...");
            if let Err(e) = server.shutdown_gracefully().await {
                error!("Error during shutdown: {}", e);
            } else {
                info!("Server shutdown gracefully");
            }
        }
    }

    Ok(())
}

/// Wait for SIGINT (Ctrl+C) or SIGTERM (docker stop / kill)
async fn shutdown_signal() {
    let ctrl_c = tokio::signal::ctrl_c();

    #[cfg(unix)]
    {
        let mut sigterm =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                .expect("failed to register SIGTERM handler");
        tokio::select! {
            _ = ctrl_c => {}
            _ = sigterm.recv() => {}
        }
    }

    #[cfg(not(unix))]
    {
        ctrl_c.await.ok();
    }
}
