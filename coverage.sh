#!/bin/bash
set -euxo pipefail
cd `/usr/bin/dirname $0`

# cargo install cargo-llvm-cov
# rustup component add llvm-tools-preview

# cargo llvm-cov
# cargo llvm-cov --html

cargo llvm-cov nextest --html --workspace \
--target aarch64-apple-darwin \
--coverage-target-only \
--exclude verse-client \
--exclude verse \
--exclude verse-dummyclient
