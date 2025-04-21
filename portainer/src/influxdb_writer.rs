use anyhow::{Context, Result};
use crossbeam::channel::{Receiver, RecvTimeoutError};
use log::{error, info};
use std::{thread, time::Instant};

const MAX_SEND_DELAY_SEC: u64 = 30;
const SEND_BUFFER_SIZE: usize = 0x100000; // 1 MB

#[derive(Debug)]
pub(crate) struct InfluxDBWriter {
    url: String,
    org: String,
    bucket: String,
    token: String,
    sender: crossbeam::channel::Sender<String>,
    receiver: crossbeam::channel::Receiver<String>,
}

impl InfluxDBWriter {
    pub(crate) fn new(url: &str, org: &str, bucket: &str, token: &str) -> Self {
        let (sender, receiver) = crossbeam::channel::unbounded();

        InfluxDBWriter {
            url: url.to_string(),
            org: org.to_string(),
            token: token.to_string(),
            bucket: bucket.to_string(),
            sender,
            receiver,
        }
    }

    pub(crate) fn get_sender(&self) -> crossbeam::channel::Sender<String> {
        self.sender.clone()
    }

    pub(crate) fn run(&self) -> Result<()> {
        let url = self.url.clone();
        let org = self.org.clone();
        let bucket = self.bucket.clone();
        let token = self.token.clone();
        let receiver = self.receiver.clone();

        info!("Starting InfluxDB writer thread");

        thread::Builder::new()
            .name("influxdb_writer".to_string())
            .spawn(move || {
                let runtime = match tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .context("Failed to create tokio runtime")
                {
                    Ok(runtime) => runtime,
                    Err(e) => {
                        error!("Failed to create tokio runtime: {}", e);
                        return;
                    }
                };

                runtime.block_on(Self::send_loop(receiver, url, org, &bucket, token));
            })
            .context("Failed to spawn InfluxDB writer thread")?;

        Ok(())
    }

    async fn send_loop(
        receiver: Receiver<String>,
        url: String,
        org: String,
        bucket: &str,
        token: String,
    ) {
        info!("InfluxDB writer thread started");

        let client = influxdb2::Client::new(url, org.clone(), token);
        let mut last_send_time = Instant::now();
        let mut send_buffer = String::with_capacity(SEND_BUFFER_SIZE);

        loop {
            match receiver.recv_timeout(std::time::Duration::from_secs(MAX_SEND_DELAY_SEC)) {
                Ok(data_point) => {
                    send_buffer.push_str(&format!("{}\n", data_point));

                    if send_buffer.len() >= SEND_BUFFER_SIZE
                        || last_send_time.elapsed().as_secs() >= MAX_SEND_DELAY_SEC
                    {
                        // Max buffer size reached or max send delay reached, send the data points
                        send_buffer = Self::send_data_points(&client, &org, bucket, send_buffer)
                            .await
                            .unwrap_or(String::with_capacity(SEND_BUFFER_SIZE));
                        last_send_time = Instant::now();
                    }
                }
                Err(RecvTimeoutError::Timeout) => {
                    // Maximum delay reached, send any buffered data points
                    send_buffer = Self::send_data_points(&client, &org, bucket, send_buffer)
                        .await
                        .unwrap_or(String::with_capacity(SEND_BUFFER_SIZE));
                    last_send_time = Instant::now();
                }
                Err(RecvTimeoutError::Disconnected) => {
                    error!("Receiver disconnected, exiting");
                    break;
                }
            }
        }

        info!("InfluxDB writer thread exiting");
    }

    async fn send_data_points(
        client: &influxdb2::Client,
        org: &str,
        bucket: &str,
        send_buffer: String,
    ) -> Result<String> {
        let len = send_buffer.len();

        if len == 0 {
            return Ok(send_buffer);
        }

        match client.write_line_protocol(&org, bucket, send_buffer).await {
            Ok(_) => info!("Successfully sent {} bytes to InfluxDB", len),
            Err(e) => {
                // TODO: Add retry logic
                error!("Failed to send data points to InfluxDB: {}", e);
                return Err(anyhow::anyhow!(
                    "Failed to send data points to InfluxDB: {}",
                    e
                ));
            }
        }

        Ok(String::with_capacity(SEND_BUFFER_SIZE))
    }
}
