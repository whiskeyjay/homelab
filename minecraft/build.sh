#!/bin/bash

docker login -u whiskeyjay
docker buildx build -t whiskeyjay/whmcsvr:latest -t whiskeyjay/whmcsvr:v1.21.8  --platform linux/amd64,linux/arm64 . --push
