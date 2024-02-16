#!/bin/sh
if cargo +nightly test --features sorella-server; then : ; else exit; fi
