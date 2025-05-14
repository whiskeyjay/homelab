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

# Determine the system's CPU architecture
ARCH=$(uname -m)
if [ "$ARCH" == "x86_64" ]; then
    TARGET="x86_64-unknown-linux-musl"
elif [ "$ARCH" == "aarch64" ]; then
    TARGET="aarch64-unknown-linux-musl"
else
    echo "Unsupported architecture: $ARCH"
    exit 1
fi

# Run the application with the appropriate target
cargo run --release --target $TARGET
