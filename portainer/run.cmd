@echo off
REM Set environment variables for testing the application

set PORTAINER_URL=https://portainer.wang-home.net
set PORTAINER_TOKEN_FILE=%~dp0%.secrets\portainer
set INFLUXDB_URL=https://influxdb.wang-home.net
set INFLUXDB_TOKEN_FILE=%~dp0%.secrets\influxdb
set INFLUXDB_ORG=18315178b6b5bc7e
set INFLUXDB_BUCKET=a4e04404f8dd4cdb
set POLL_INTERVAL_SECONDS=10

echo Environment variables set.

cargo run --release --target x86_64-pc-windows-msvc
