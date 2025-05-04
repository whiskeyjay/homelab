mod influxdb_writer;
mod models;

use crate::models::*;
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use crossbeam::channel::Sender;
use futures_util::future::join_all;
use influxdb_writer::InfluxDBWriter;
use log::{error, info};
use log4rs::{
    append::console::{ConsoleAppender, Target},
    config::{Appender, Config, Root},
    encode::pattern::PatternEncoder,
};
use reqwest::Client as HttpClient;
use std::time::{Duration, Instant};
use std::{env, fs, thread};

struct AppConfig {
    portainer_url: String,
    portainer_api_key: String,
    influxdb_url: String,
    influxdb_token: String,
    influxdb_org: String,
    influxdb_bucket: String,
    poll_interval: Duration,
}

#[tokio::main]
async fn main() -> Result<()> {
    let config = load_config()?;

    // Initialize logging
    init_logging().context("Failed to initialize logging")?;

    info!("Starting Portainer metrics to InfluxDB exporter...");

    // Initialize clients
    let http_client = HttpClient::new();

    let data_sender = create_influxdb_writer(
        &config.influxdb_url,
        &config.influxdb_org,
        &config.influxdb_bucket,
        &config.influxdb_token,
    )?;

    loop {
        info!(
            "Fetching endpoints from Portainer endpoint \"{}\"...",
            config.portainer_url
        );

        let last_poll = Instant::now();
        let endpoints = get_endpoints(&http_client, &config)
            .await
            .context("Failed to get endpoints")?;

        info!("Found {} endpoints.", endpoints.len());

        let futures: Vec<_> = endpoints
            .iter()
            .map(|endpoint| process_endpoint(&http_client, &data_sender, &config, endpoint))
            .collect();

        for result in join_all(futures).await {
            if let Err(e) = result {
                error!("Error processing endpoint: {:?}", e);
            }
        }

        match config.poll_interval.checked_sub(last_poll.elapsed()) {
            Some(sleep_time) => {
                info!("Waiting {} seconds for next poll...", sleep_time.as_secs());
                thread::sleep(sleep_time);
            }
            None => {}
        }
    }
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

fn create_influxdb_writer(
    influxdb_url: &str,
    influxdb_org: &str,
    influxdb_bucket: &str,
    influxdb_token: &str,
) -> Result<Sender<String>> {
    let writer = InfluxDBWriter::new(influxdb_url, influxdb_org, influxdb_bucket, influxdb_token);
    writer.run()?;

    Ok(writer.get_sender())
}

async fn get_endpoints(client: &HttpClient, config: &AppConfig) -> Result<Vec<Endpoint>> {
    let url = format!("{}/api/endpoints", &config.portainer_url);
    let response = client
        .get(&url)
        .header("X-API-Key", &config.portainer_api_key)
        .send()
        .await
        .context("Failed to get endpoints")?;

    let endpoints: Vec<Endpoint> = response
        .json()
        .await
        .context("Failed to parse endpoints JSON")?;

    Ok(endpoints)
}

async fn process_endpoint(
    http_client: &HttpClient,
    data_sender: &Sender<String>,
    config: &AppConfig,
    endpoint: &Endpoint,
) -> Result<()> {
    info!("Fetching containers from endpoint \"{}\"...", endpoint.name);

    let containers = get_containers(http_client, config, endpoint.id).await?;

    info!(
        "Found {} containers in endpoint \"{}\".",
        containers.len(),
        endpoint.name
    );

    let futures = containers.iter().map(|container| {
        let container_name = container
            .names
            .get(0)
            .map(|s| s.trim_start_matches('/'))
            .unwrap_or("unknown");

        async move {
            if let Err(e) = process_container(
                http_client,
                data_sender,
                config,
                endpoint,
                container,
                container_name,
            )
            .await
            {
                error!(
                    "Error processing container \"{}\" (endpoint \"{}\"): {:?}",
                    container_name, endpoint.name, e
                );
            }
        }
    });

    // Execute all futures concurrently
    join_all(futures).await;

    Ok(())
}

async fn get_containers(
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
        .await
        .context("Failed to get containers")?;

    let containers: Vec<Container> = response
        .json()
        .await
        .context("Failed to parse containers JSON")?;
    Ok(containers)
}

async fn process_container(
    http_client: &HttpClient,
    data_sender: &Sender<String>,
    config: &AppConfig,
    endpoint: &Endpoint,
    container: &Container,
    container_name: &str,
) -> Result<()> {
    info!(
        "Fetching stats for container \"{}\" (ID: {}) on endpoint \"{}\"...",
        container_name, container.id, endpoint.name
    );

    let stats =
        get_container_stats(http_client, config, endpoint.id, container.id.as_str()).await?;
    let mut main_stats = String::new();

    // Set tags
    main_stats.push_str(&format!(
        "docker_container_stats,endpoint_id={},endpoint_name={},container_id={},container_name={}",
        endpoint.id,
        endpoint.name.replace(" ", "\\ "),
        container.id.replace(" ", "\\ "),
        container_name.replace(" ", "\\ ")
    ));

    // CPU Metrics
    let cpu_percent = calculate_cpu_percent(&stats);

    main_stats.push_str(&format!(
        " cpu_percent={},cpu_total_usage={}i,cpu_kernelmode_usage={}i,cpu_usermode_usage={}i,cpu_online_cpus={}i,cpu_throttle_periods={}i,cpu_throttled_periods={}i,cpu_throttled_time={}i",
        cpu_percent,
        stats.cpu_stats.cpu_usage.total_usage as i64,
        stats.cpu_stats.cpu_usage.usage_in_kernelmode as i64,
        stats.cpu_stats.cpu_usage.usage_in_usermode as i64,
        stats.cpu_stats.online_cpus as i64,
        stats.cpu_stats.throttling_data.periods as i64,
        stats.cpu_stats.throttling_data.throttled_periods as i64,
        stats.cpu_stats.throttling_data.throttled_time as i64
    ));

    // Memory Metrics
    let memory_usage_mb = stats.memory_stats.usage as f64 / 1024.0 / 1024.0;
    let memory_limit_mb = stats.memory_stats.limit as f64 / 1024.0 / 1024.0;
    let memory_percent = if memory_limit_mb > 0.0 {
        (memory_usage_mb / memory_limit_mb) * 100.0
    } else {
        0.0
    };

    main_stats.push_str(&format!(
        ",memory_usage_mb={},memory_limit_mb={},memory_percent={},memory_active_anon_mb={},memory_active_file_mb={},memory_inactive_anon_mb={},memory_inactive_file_mb={},memory_file_mapped_mb={}",
        memory_usage_mb,
        memory_limit_mb,
        memory_percent,
        stats.memory_stats.stats.active_anon as f64 / 1024.0 / 1024.0,
        stats.memory_stats.stats.active_file as f64 / 1024.0 / 1024.0,
        stats.memory_stats.stats.inactive_anon as f64 / 1024.0 / 1024.0,
        stats.memory_stats.stats.inactive_file as f64 / 1024.0 / 1024.0,
        stats.memory_stats.stats.file_mapped as f64 / 1024.0 / 1024.0
    ));

    // Process Metrics
    if let Some(pids_current) = stats.pids_stats.get("current") {
        main_stats.push_str(&format!(",pids_current={}i ", *pids_current as i64));
    }

    // Timestamp
    let timestamp: DateTime<Utc> = stats.read.parse().unwrap_or(Utc::now());

    main_stats.push_str(&format!(
        " {}",
        timestamp
            .timestamp_nanos_opt()
            .context("Failed to convert timestamp to nanoseconds")?
    ));

    // Write main stats
    data_sender
        .send(main_stats)
        .context("Failed to send main data point to the InfluxDB writer")?;

    // Network Metrics (per interface)
    if let Some(networks) = stats.networks {
        for (interface, net_stats) in networks {
            let data_point = format!(
                "docker_container_network,endpoint_id={},endpoint_name={},container_id={},container_name={},interface={} rx_bytes={}i,rx_packets={}i,rx_errors={}i,rx_dropped={}i,tx_bytes={}i,tx_packets={}i,tx_errors={}i,tx_dropped={}i {}",
                endpoint.id,
                endpoint.name.replace(" ", "\\ "),
                container.id.replace(" ", "\\ "),
                container_name.replace(" ", "\\ "),
                interface.replace(" ", "\\ "),
                net_stats.rx_bytes as i64,
                net_stats.rx_packets as i64,
                net_stats.rx_errors as i64,
                net_stats.rx_dropped as i64,
                net_stats.tx_bytes as i64,
                net_stats.tx_packets as i64,
                net_stats.tx_errors as i64,
                net_stats.tx_dropped as i64,
                timestamp
                    .timestamp_nanos_opt()
                    .context("Failed to convert timestamp to nanoseconds")?
            );

            data_sender
                .send(data_point)
                .context("Failed to send network data point to the InfluxDB writer")?;
        }
    }

    // Block I/O Metrics
    if let Some(io_service_bytes_recursive) = stats.blkio_stats.io_service_bytes_recursive {
        for blkio_stat in io_service_bytes_recursive {
            let device = format!("{}:{}", blkio_stat.major, blkio_stat.minor);
            let op = blkio_stat.op.to_lowercase();
            let field_name = format!("blkio_{}_bytes", op);
            let data_point = format!(
                "docker_container_blkio,endpoint_id={},endpoint_name={},container_id={},container_name={},device={},operation={} {}={}i {}",
                endpoint.id,
                endpoint.name.replace(" ", "\\ "),
                container.id.replace(" ", "\\ "),
                container_name.replace(" ", "\\ "),
                device.replace(" ", "\\ "),
                op.replace(" ", "\\ "),
                field_name.replace(" ", "\\ "),
                blkio_stat.value as i64,
                timestamp
                    .timestamp_nanos_opt()
                    .context("Failed to convert timestamp to nanoseconds")?
            );

            data_sender
                .send(data_point)
                .context("Failed to send blkio data point to the InfluxDB writer")?;
        }
    }

    info!(
        "Sent metrics for container \"{}\" (endpoint \"{}\") to the outgoing queue",
        container_name, endpoint.name
    );

    Ok(())
}

async fn get_container_stats(
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
        .await
        .context("Failed to get container stats")?;

    let stats: Stats = response
        .json()
        .await
        .context("Failed to parse stats JSON")?;
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
