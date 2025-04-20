# Portainer Metrics to InfluxDB

This application collects Docker container metrics from the Portainer API and sends them to an InfluxDB instance for storage and analysis.

## Configuration

The application requires the following environment variables to be configured:

| **Variable**             | **Description**                                                                                     |
|---------------------------|-----------------------------------------------------------------------------------------------------|
| `PORTAINER_URL`           | The URL of the Portainer instance (e.g., `https://portainer.example.com`).                         |
| `PORTAINER_TOKEN_FILE`    | Path to the file containing the token for authenticating with the Portainer API. The file path must start with `/app/secrets` and match the name specified in the Docker secrets configuration. |
| `INFLUXDB_URL`            | The URL of the InfluxDB instance (e.g., `https://influxdb.example.com`).                           |
| `INFLUXDB_ORG`            | The target organization for storing metrics in InfluxDB.                                          |
| `INFLUXDB_BUCKET`         | The target bucket for storing metrics in InfluxDB.                                                |
| `INFLUXDB_TOKEN_FILE`     | Path to the file containing the token for authenticating with the InfluxDB API. The file path must start with `/app/secrets` and match the name specified in the Docker secrets configuration. |
| `POLL_INTERVAL_SECONDS`   | The interval (in seconds) between polling the Portainer instance for metrics.                     |

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
2. Ensure the secrets files (`/secrets/portainer_token` and `/secrets/influxdb_token`) are correctly created and accessible by the container.
