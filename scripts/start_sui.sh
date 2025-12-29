#!/usr/bin/env bash
# Start SUI localnet with faucet and fresh genesis
RUST_LOG="off,sui_node=info" sui start --with-faucet --force-regenesis --epoch-duration-ms 10000
