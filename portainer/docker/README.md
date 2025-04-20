# Portainer Metrics to InfluxDB

This application collects Docker container metrics from the Portainer API and sends them to an InfluxDB instance for storage and analysis.

## Configuration

The application requires the following environment variables to be configured:

| **Variable**|**Description**|
|-|-|
|`PORTAINER_URL`| The URL of the Portainer instance (e.g., `https://portainer.example.com`).|
|`PORTAINER_TOKEN_FILE`| Path to the file containing the token for authenticating with the Portainer API.|
|`INFLUXDB_URL`| The URL of the InfluxDB instance (e.g., `https://influxdb.example.com`).|
|`INFLUXDB_ORG`| The target organization for storing metrics in InfluxDB.|
|`INFLUXDB_BUCKET`| The target bucket for storing metrics in InfluxDB.|
|`INFLUXDB_TOKEN_FILE`| Path to the file containing the token for authenticating with the InfluxDB API.|
|`POLL_INTERVAL_SECONDS`| The interval (in seconds) between polling the Portainer instance for metrics.|

### Notes

In this container, the main application operates as a non-root user (`appuser`). Consequently, in some cases, it may lack sufficient permissions to access secret files located in the default `/run/secrets` directory (for example when the secret file on Docker host has permission `600` and ownership `root:root`). To address this issue, a startup script is employed to handle secrets. The script performs the following actions:

- Copying Secrets: The script, running with root privileges, copies the secret files from the `/run/secrets` directory to `/app/secrets`.
- Adjusting Permissions: Ownership of the files in `/app/secrets` is changed to `appuser`, and file permissions are updated to `0400` (read-only for the owner).
- Launching the Application: After managing the secrets, the script starts the main program under the non-root user `appuser`.

To ensure proper access to secret files, use `/app/secrets` as the base path for the following environment variables:

- `PORTAINER_TOKEN_FILE`
- `INFLUXDB_TOKEN_FILE`

## Deployment with Docker Compose

Below is an example `docker-compose.yml` file to deploy the application:

```yaml
version: '3.9'

services:
  ptn2influx:
    image: whiskeyjay/ptn2influx:latest
    restart: unless-stopped
    environment:
      - PORTAINER_URL=${PORTAINER_URL}
      - PORTAINER_TOKEN_FILE=/app/secrets/portainer
      - INFLUXDB_URL=${INFLUXDB_URL}
      - INFLUXDB_ORG=${INFLUXDB_ORG}
      - INFLUXDB_BUCKET=${INFLUXDB_BUCKET}
      - INFLUXDB_TOKEN_FILE=/app/secrets/influxdb
      - POLL_INTERVAL_SECONDS=${POLL_INTERVAL_SECONDS}
    secrets:
      - portainer
      - influxdb

secrets:
  portainer:
    file: /secrets/portainer_token
  influxdb:
    file: /secrets/influxdb_token
```

### Notes:

1. Replace the placeholders (e.g., `${PORTAINER_URL}`, `${INFLUXDB_URL}`) with actual values or define them in an `.env` file.
2. Ensure the secrets are correctly created and mapped into the container.
