#!/bin/bash
set -e

# Build command arguments from environment variables
ARGS=()

# Listen address
if [ -n "$LISTEN_ADDR" ]; then
    ARGS+=("-l" "$LISTEN_ADDR")
fi

# DoH servers (comma-separated)
if [ -n "$DOH_SERVERS" ]; then
    IFS=',' read -ra SERVERS <<< "$DOH_SERVERS"
    for server in "${SERVERS[@]}"; do
        ARGS+=("-s" "$server")
    done
fi

# Timeout
if [ -n "$TIMEOUT_SECS" ]; then
    ARGS+=("-t" "$TIMEOUT_SECS")
fi

# Cache size
if [ -n "$CACHE_SIZE" ]; then
    ARGS+=("-c" "$CACHE_SIZE")
fi

# Verbose mode
if [ "$VERBOSE" = "true" ]; then
    ARGS+=("-v")
fi

# Run the application
exec /app/doh-proxy "${ARGS[@]}"
