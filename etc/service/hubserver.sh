#!/bin/bash
_BASE_DIR=`/usr/bin/dirname $0`
cd $_BASE_DIR 
_LOG_DIR=${_BASE_DIR}/log

if [ ! -e $_LOG_DIR ]; then
  mkdir $_LOG_DIR
fi


_TOKEN=
for i in {1..10}
do
  _TOKEN=`curl -s -X PUT "http://169.254.169.254/latest/api/token" -H "X-aws-ec2-metadata-token-ttl-seconds: 120"`
  if [ -z "$_TOKEN" ]; then
    sleep $i
  else
    break
  fi
done
_ID=`curl -s -H "X-aws-ec2-metadata-token: $_TOKEN" http://169.254.169.254/latest/meta-data/instance-id/`
_IP=`curl -s -H "X-aws-ec2-metadata-token: $_TOKEN" http://169.254.169.254/latest/meta-data/public-ipv4`
_REGION=$(curl -s -H "X-aws-ec2-metadata-token: $_TOKEN" http://169.254.169.254/latest/meta-data/placement/availability-zone | sed -e 's/.$//')

_NEXT_WAIT=0
#_CMD="aws s3 cp s3://deploy-mdev/hubserver/hubserver-dev.linux-arm64 /home/ec2-user/hubserver"
_CMD="aws s3 cp s3://deploy-mdev/hubserver/hubserver.linux-arm64 /home/ec2-user/hubserver"
until $_CMD || [ $_NEXT_WAIT -eq 10 ]; do
    sleep $(( _NEXT_WAIT++ ))
    echo "retry $_NEXT_WAIT"
done

chmod u+x /home/ec2-user/hubserver

export AWS_EC2_INSTANCE_ID=$_ID

RUST_LOG=info,webrtc=warn,hyper=warn,rustls=warn,verse_hubserv=info \
./hubserver \
  --status-port 9098 \
  --http-port 80 \
  --public-ip $_IP \
  --aws-ec2-region $_REGION \
  --cache /home/ec2-user/certs \
  --max-connections-by-url 100 \
  --prometheus-prefix hubsrv_ \
  --http-host entrance.verseengine.cloud \
  --access-log-path=${_LOG_DIR}/entrance-access.log \
  --http-log-path=${_LOG_DIR}/entrance-http.log \
  --cluster-node-stage=dev \
  --cluster-node-role=CellServer \
  --cluster-json-s3-bucket=mdev-test-data \
  --cluster-json-s3-key=cluster/cluster-dev.json \
  --update-cluster-key=dfqowjaskdfjgsal \
  --cluster-node-list-url https://mdev-test-data.s3.ap-northeast-1.amazonaws.com/cluster/cluster-dev.json
