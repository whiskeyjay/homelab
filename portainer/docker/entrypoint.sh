#!/bin/sh

mkdir -p /app/secrets

if [ -d /run/secrets ]; then
    cp /run/secrets/* /app/secrets/ 2>/dev/null || true
    chown appuser:appgroup /app/secrets/*
    chmod 400 /app/secrets/*
fi
exec /app/ptn2influx
