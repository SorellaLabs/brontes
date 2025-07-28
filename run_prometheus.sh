#!/bin/bash

docker run --rm -d \
    --name prometheus \
    -p 9090:9090 \
    --network=host \
    -v ./etc/prometheus/prometheus.yml:/etc/prometheus/prometheus.yml \
    -v prometheus-data:/prometheus \
    prom/prometheus
