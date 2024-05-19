#!/bin/bash
set -euxo pipefail
cd `/usr/bin/dirname $0`

wasm-pack test  --headless --chrome
