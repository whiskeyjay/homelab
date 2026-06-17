#!/bin/bash

docker login -u whiskeyjay
docker buildx build -t whiskeyjay/whmcsvr:latest -t whiskeyjay/whmcsvr:v26.2.0 --platform linux/amd64,linux/arm64 . --push
