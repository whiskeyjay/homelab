#!/bin/bash
# Set environment variables for the application

export PORTAINER_URL=http://your-portainer-url
export PORTAINER_TOKEN_FILE=your-portainer-token
export INFLUXDB_URL=http://your-influxdb-url
export INFLUXDB_TOKEN_FILE=your-influxdb-token
export INFLUXDB_ORG=your-org
export INFLUXDB_BUCKET=your-bucket
export POLL_INTERVAL_SECONDS=10

echo "Environment variables set."

cargo run --release --target x86_64-unknown-linux-musl
