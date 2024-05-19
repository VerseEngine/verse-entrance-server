set -euxo pipefail
cd `/usr/bin/dirname $0`

_SED=gsed
_VERSION_FILE=hubserv/src/version.rs
_DATE=`date +%Y.%m.%d`

$_SED -i -e "s#pub const VERSION: &str = \".*\";#pub const VERSION: \&str = \"$_DATE\";#g" $_VERSION_FILE

RUSTFLAGS="--cfg tokio_unstable" cargo build --target aarch64-unknown-linux-musl --bin verse-hubserv
RUSTFLAGS="--cfg tokio_unstable" cargo build --target x86_64-unknown-linux-musl --bin verse-hubserv 
echo "OK target/x86_64-unknown-linux-musl/debug/verse-hubserv"
echo "OK target/aarch64-unknown-linux-musl/debug/verse-hubserv"

# Usage:
# ./hubserver \
# --status-port 9098 \
# --http-port 443 \
# --public-ip $_IP \
# --aws-ec2-region $_REGION \
# --use-https \
# --cache /home/ec2-user/certs \
# --max-connections-by-url 100 \
# --http-host verse.example.org
