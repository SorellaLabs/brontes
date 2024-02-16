#!/bin/sh
rm -rf /home/data/brontes-test/*
if cargo +nightly bench --features sorella-server; then : ; else exit; fi
