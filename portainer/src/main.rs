mod models;

use crate::models::*;
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use influxdb2::Client as InfluxClient;
use influxdb2::models::DataPoint;
use log::{error, info};
use log4rs::{
    append::console::{ConsoleAppender, Target},
    config::{Appender, Config, Root},
    encode::pattern::PatternEncoder,
};
use reqwest::blocking::Client as HttpClient;
use std::time::Duration;
use std::{env, fs, thread};
use tokio::runtime::Runtime;

struct AppConfig {
    portainer_url: String,
    portainer_api_key: String,
    influxdb_url: String,
    influxdb_token: String,
    influxdb_org: String,
    influxdb_bucket: String,
    poll_interval: Duration,
}

fn load_config() -> Result<AppConfig> {
    // Load Portainer URL and token from environment variables
    let portainer_url =
        env::var("PORTAINER_URL").context("PORTAINER_URL environment variable is required")?;

    let portainer_token = fs::read_to_string(
        env::var("PORTAINER_TOKEN_FILE")
            .context("PORTAINER_TOKEN_FILE environment variable is required")?,
    )
    .context("Failed to read Portainer token file")?
    .trim()
    .to_string();

    // Load InfluxDB URL, token, org, and bucket from environment variables
    let influxdb_url =
        env::var("INFLUXDB_URL").context("INFLUXDB_URL environment variable is required")?;

    let influxdb_token = fs::read_to_string(
        env::var("INFLUXDB_TOKEN_FILE")
            .context("INFLUXDB_TOKEN_FILE environment variable is required")?,
    )
    .context("Failed to read InfluxDB token file")?
    .trim()
    .to_string();

    let influxdb_org =
        env::var("INFLUXDB_ORG").context("INFLUXDB_ORG environment variable is required")?;

    let influxdb_bucket =
        env::var("INFLUXDB_BUCKET").context("INFLUXDB_BUCKET environment variable is required")?;

    // Load poll interval from environment variable
    let poll_interval_secs = env::var("POLL_INTERVAL_SECONDS")
        .unwrap_or_else(|_| "10".to_string())
        .parse::<u64>()
        .context("Invalid POLL_INTERVAL_SECONDS")?;

    Ok(AppConfig {
        portainer_url,
        portainer_api_key: portainer_token,
        influxdb_url,
        influxdb_token,
        influxdb_org,
        influxdb_bucket,
        poll_interval: Duration::from_secs(poll_interval_secs),
    })
}

fn init_logging() -> Result<()> {
    let stderr = ConsoleAppender::builder()
        .encoder(Box::new(PatternEncoder::new(
            "{d(%H:%M:%S%.3f%:z)} {l} {M}:{L} - {m}\n",
        )))
        .target(Target::Stderr)
        .build();
    let config_builder =
        Config::builder().appender(Appender::builder().build("stderr", Box::new(stderr)));
    let root_builder = Root::builder().appender("stderr");

    let config = config_builder.build(root_builder.build(log::LevelFilter::Info))?;
    log4rs::init_config(config)?;

    Ok(())
}

fn main() -> Result<()> {
    let config = load_config()?;

    // Initialize logging
    init_logging().context("Failed to initialize logging")?;

    // Initialize clients
    let http_client = HttpClient::new();
    let influx_client = InfluxClient::new(
        &config.influxdb_url,
        &config.influxdb_org,
        &config.influxdb_token,
    );

    info!("Starting Portainer metrics to InfluxDB exporter...");

    loop {
        info!(
            "Fetching endpoints from Portainer endpoint \"{}\"...",
            config.portainer_url
        );

        let endpoints = get_endpoints(&http_client, &config).context("Failed to get endpoints")?;

        info!("Found {} endpoints.", endpoints.len());

        for endpoint in &endpoints {
            if let Err(e) = process_endpoint(&http_client, &influx_client, &config, endpoint) {
                eprintln!("Error processing endpoint {}: {:?}", endpoint.id, e);
            }
        }
        thread::sleep(config.poll_interval);
    }
}

fn get_endpoints(client: &HttpClient, config: &AppConfig) -> Result<Vec<Endpoint>> {
    let url = format!("{}/api/endpoints", &config.portainer_url);
    let response = client
        .get(&url)
        .header("X-API-Key", &config.portainer_api_key)
        .send()
        .context("Failed to get endpoints")?;

    let endpoints: Vec<Endpoint> = response.json().context("Failed to parse endpoints JSON")?;

    Ok(endpoints)
}

fn process_endpoint(
    http_client: &HttpClient,
    influx_client: &InfluxClient,
    config: &AppConfig,
    endpoint: &Endpoint,
) -> Result<()> {
    info!("Fetching containers from endpoint \"{}\"...", endpoint.name);

    let containers = get_containers(http_client, config, endpoint.id)?;

    info!(
        "Found {} containers in endpoint \"{}\".",
        containers.len(),
        endpoint.name
    );

    for container in &containers {
        let container_name = container
            .names
            .get(0)
            .map(|s| s.trim_start_matches('/'))
            .unwrap_or("unknown");

        if let Err(e) = process_container(
            http_client,
            influx_client,
            config,
            endpoint,
            container,
            container_name,
        ) {
            error!(
                "Error processing container \"{}\" (endpoint \"{}\"): {:?}",
                container_name, endpoint.name, e
            );
        }
    }
    Ok(())
}

fn get_containers(
    client: &HttpClient,
    config: &AppConfig,
    endpoint_id: i32,
) -> Result<Vec<Container>> {
    let url = format!(
        "{}/api/endpoints/{}/docker/containers/json?all=true",
        config.portainer_url, endpoint_id
    );
    let response = client
        .get(&url)
        .header("X-API-Key", &config.portainer_api_key)
        .send()
        .context("Failed to get containers")?;

    let containers: Vec<Container> = response.json().context("Failed to parse containers JSON")?;
    Ok(containers)
}

fn process_container(
    http_client: &HttpClient,
    influx_client: &InfluxClient,
    config: &AppConfig,
    endpoint: &Endpoint,
    container: &Container,
    container_name: &str,
) -> Result<()> {
    info!(
        "Fetching stats for container \"{}\" (ID: {}) on endpoint \"{}\"...",
        container_name, container.id, endpoint.name
    );

    let stats = get_container_stats(http_client, config, endpoint.id, container.id.as_str())?;
    let timestamp: DateTime<Utc> = stats.read.parse().context("Failed to parse timestamp")?;
    let mut builder = DataPoint::builder("docker_container_stats")
        .timestamp(
            timestamp
                .timestamp_nanos_opt()
                .context("Failed to convert timestamp to nanoseconds")?,
        )
        .tag("endpoint_id", endpoint.id.to_string())
        .tag("endpoint_name", endpoint.name.to_string())
        .tag("container_id", container.id.to_string())
        .tag("container_name", container_name.to_string());

    // CPU Metrics
    let cpu_percent = calculate_cpu_percent(&stats);
    builder = builder
        .field("cpu_percent", cpu_percent)
        .field(
            "cpu_total_usage",
            stats.cpu_stats.cpu_usage.total_usage as i64,
        )
        .field(
            "cpu_kernelmode_usage",
            stats.cpu_stats.cpu_usage.usage_in_kernelmode as i64,
        )
        .field(
            "cpu_usermode_usage",
            stats.cpu_stats.cpu_usage.usage_in_usermode as i64,
        )
        .field("cpu_online_cpus", stats.cpu_stats.online_cpus as i64)
        .field(
            "cpu_throttle_periods",
            stats.cpu_stats.throttling_data.periods as i64,
        )
        .field(
            "cpu_throttled_periods",
            stats.cpu_stats.throttling_data.throttled_periods as i64,
        )
        .field(
            "cpu_throttled_time",
            stats.cpu_stats.throttling_data.throttled_time as i64,
        );

    // Memory Metrics
    let memory_usage_mb = stats.memory_stats.usage as f64 / 1024.0 / 1024.0;
    let memory_limit_mb = stats.memory_stats.limit as f64 / 1024.0 / 1024.0;
    let memory_percent = if memory_limit_mb > 0.0 {
        (memory_usage_mb / memory_limit_mb) * 100.0
    } else {
        0.0
    };

    builder = builder
        .field("memory_usage_mb", memory_usage_mb)
        .field("memory_limit_mb", memory_limit_mb)
        .field("memory_percent", memory_percent)
        .field(
            "memory_active_anon_mb",
            stats.memory_stats.stats.active_anon as f64 / 1024.0 / 1024.0,
        )
        .field(
            "memory_active_file_mb",
            stats.memory_stats.stats.active_file as f64 / 1024.0 / 1024.0,
        )
        .field(
            "memory_inactive_anon_mb",
            stats.memory_stats.stats.inactive_anon as f64 / 1024.0 / 1024.0,
        )
        .field(
            "memory_inactive_file_mb",
            stats.memory_stats.stats.inactive_file as f64 / 1024.0 / 1024.0,
        )
        .field(
            "memory_file_mapped_mb",
            stats.memory_stats.stats.file_mapped as f64 / 1024.0 / 1024.0,
        );

    let runtime = Runtime::new().context("Failed to create runtime")?;

    // Network Metrics (per interface)
    if let Some(networks) = stats.networks {
        for (interface, net_stats) in networks {
            let net_point = DataPoint::builder("docker_container_network")
                .tag("endpoint_id", endpoint.id.to_string())
                .tag("endpoint_name", endpoint.name.to_string())
                .tag("container_id", container.id.to_string())
                .tag("container_name", container_name.to_string())
                .tag("interface", interface.clone())
                .field("rx_bytes", net_stats.rx_bytes as i64)
                .field("rx_packets", net_stats.rx_packets as i64)
                .field("rx_errors", net_stats.rx_errors as i64)
                .field("rx_dropped", net_stats.rx_dropped as i64)
                .field("tx_bytes", net_stats.tx_bytes as i64)
                .field("tx_packets", net_stats.tx_packets as i64)
                .field("tx_errors", net_stats.tx_errors as i64)
                .field("tx_dropped", net_stats.tx_dropped as i64)
                .build()
                .context("Failed to build network data point")?;

            runtime
                .block_on(influx_client.write(
                    &config.influxdb_bucket,
                    futures_util::stream::iter(vec![net_point]),
                ))
                .context("Failed to write network stats to InfluxDB")?;
        }
    }

    // Block I/O Metrics
    if let Some(io_service_bytes_recursive) = stats.blkio_stats.io_service_bytes_recursive {
        for blkio_stat in io_service_bytes_recursive {
            let device = format!("{}:{}", blkio_stat.major, blkio_stat.minor);
            let op = blkio_stat.op.to_lowercase();
            let field_name = format!("blkio_{}_bytes", op);
            let blkio_point = DataPoint::builder("docker_container_blkio")
                .tag("endpoint_id", endpoint.id.to_string())
                .tag("endpoint_name", endpoint.name.to_string())
                .tag("container_id", container.id.to_string())
                .tag("container_name", container_name.to_string())
                .tag("device", device)
                .tag("operation", op)
                .field(&field_name, blkio_stat.value as i64)
                .build()
                .context("Failed to build blkio data point")?;

            runtime
                .block_on(influx_client.write(
                    &config.influxdb_bucket,
                    futures_util::stream::iter(vec![blkio_point]),
                ))
                .context("Failed to write blkio stats to InfluxDB")?;
        }
    }

    // Process Metrics
    if let Some(pids_current) = stats.pids_stats.get("current") {
        builder = builder.field("pids_current", *pids_current as i64);
    }

    // Write main stats
    let point = builder.build().context("Failed to build main data point")?;

    runtime
        .block_on(influx_client.write(
            &config.influxdb_bucket,
            futures_util::stream::iter(vec![point]),
        ))
        .context("Failed to write main stats to InfluxDB")?;

    info!(
        "Written metrics for container \"{}\" (endpoint \"{}\")",
        container_name, endpoint.name
    );

    Ok(())
}

fn get_container_stats(
    client: &HttpClient,
    config: &AppConfig,
    endpoint_id: i32,
    container_id: &str,
) -> Result<Stats> {
    let url = format!(
        "{}/api/endpoints/{}/docker/containers/{}/stats?stream=false",
        config.portainer_url, endpoint_id, container_id
    );
    let response = client
        .get(&url)
        .header("X-API-Key", &config.portainer_api_key)
        .send()
        .context("Failed to get container stats")?;

    let stats: Stats = response.json().context("Failed to parse stats JSON")?;
    Ok(stats)
}

fn calculate_cpu_percent(stats: &Stats) -> f64 {
    let cpu_delta = stats.cpu_stats.cpu_usage.total_usage as i64
        - stats.precpu_stats.cpu_usage.total_usage as i64;
    let system_delta = stats.cpu_stats.system_cpu_usage.unwrap_or(0) as i64
        - stats.precpu_stats.system_cpu_usage.unwrap_or(0) as i64;

    if system_delta > 0 && cpu_delta > 0 {
        let num_cpus = stats.cpu_stats.online_cpus as f64;
        ((cpu_delta as f64 / system_delta as f64) * num_cpus * 100.0) as f64
    } else {
        0.0
    }
}
