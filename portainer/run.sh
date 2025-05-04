#!/bin/bash

SCRIPT_DIR=$(dirname "$(realpath "$0")")

# Set environment variables for the application

export PORTAINER_URL=https://portainer.wang-home.net
export PORTAINER_TOKEN_FILE=$SCRIPT_DIR/.secrets/portainer
export INFLUXDB_URL=https://influxaz.wang-home.net
export INFLUXDB_TOKEN_FILE=$SCRIPT_DIR/.secrets/influxdb
export INFLUXDB_ORG=18315178b6b5bc7e
export INFLUXDB_BUCKET=a4e04404f8dd4cdb
export POLL_INTERVAL_SECONDS=10

echo "Environment variables set."

cargo run --release --target x86_64-unknown-linux-musl
