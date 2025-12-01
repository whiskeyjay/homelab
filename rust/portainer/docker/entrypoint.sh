#!/bin/sh

# This script is run as root user when the container starts.
# It copy secrets from Docker secrets to a local directory in case 
# the secret files in /run/secrets are not readable by the app user.
mkdir -p /app/secrets

if [ -d /run/secrets ]; then
    cp /run/secrets/* /app/secrets/ 2>/dev/null || true
    chown appuser:appgroup /app/secrets/*
    chmod 400 /app/secrets/*
fi

# Execute the main application as the appuser
exec su -s /bin/sh appuser -c "/app/ptn2influx"
