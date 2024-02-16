#!/bin/sh
  git pull
  git checkout $1
  git pull
  rustup default nightly
  if cargo +nightly test --features sorella-server; then : ; else exit; fi
  git checkout main
