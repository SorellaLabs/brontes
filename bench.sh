#!/bin/sh
if cargo +nightly bench --features sorella-server; then : ; else exit; fi
