#!/bin/bash
set -euxo pipefail
cd `/usr/bin/dirname $0`

find . -name "*.clean"|xargs rm
