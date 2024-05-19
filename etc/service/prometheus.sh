#!/bin/bash
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
_ROLE=`curl -s -H "X-aws-ec2-metadata-token: $_TOKEN" http://169.254.169.254/latest/meta-data/tags/instance/Role`
_STAGE=`curl -s -H "X-aws-ec2-metadata-token: $_TOKEN" http://169.254.169.254/latest/meta-data/tags/instance/Stage`

cat /usr/local/prometheus-2.37.0.linux-arm64/prometheus.yml.tmpl | \
sed -e "s#__INSTANCE_ID__#$_ID#g" | \
sed -e "s#__ROLE__#$_ROLE#g" | \
sed -e "s#__STAGE__#$_STAGE#g" \
> /var/prometheus/prometheus.yml

/usr/local/prometheus-2.37.0.linux-arm64/prometheus --config.file=/var/prometheus/prometheus.yml --storage.tsdb.path=/var/prometheus
