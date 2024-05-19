#!/bin/bash
set -euxo pipefail
cd `/usr/bin/dirname $0`

cargo nextest run --workspace \
--exclude verse-client \
--exclude verse \
--exclude verse-dummyclient


# cargo nextest run --workspace \
# --exclude verse-client \
# --exclude verse \
# --exclude verse-dummyclient \
# --nocapture

#  -- --nocapture


# npx wasm-pack test --headless --firefox

# cargo test  -- --nocapture 
