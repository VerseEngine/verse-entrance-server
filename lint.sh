#!/bin/bash
set -euxo pipefail
cd `/usr/bin/dirname $0`

# setup: rustup component add clippy

cargo clippy --all-targets
# cargo clippy --target=wasm32-unknown-unknown --all-targets
