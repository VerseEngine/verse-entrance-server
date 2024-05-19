#!/bin/bash
set -euxo pipefail
cd `/usr/bin/dirname $0`

cargo install cargo-update
cargo install cargo-llvm-cov
# cargo install protobuf-codegen
cargo install sccache

brew install cargo-nextest


# Update all
# >cargo install-update --all
# >rustup update
# >cargo update

# Update dependencies
# >cargo update --dry-run 
# >cargo update
