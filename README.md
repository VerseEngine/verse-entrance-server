# verse-entrance-server

[<img alt="MIT" src="https://img.shields.io/github/license/VerseEngine/verse-session-id?style=for-the-badge" height="20">](https://github.com/VerseEngine/verse-session-id/blob/main/LICENSE)


Servers that serve as entry points to p2p networks.


![p2p-network](https://private-user-images.githubusercontent.com/125547575/331844981-c01ef6c0-f151-4b76-bec6-7d9d27ed848a.png?jwt=eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJpc3MiOiJnaXRodWIuY29tIiwiYXVkIjoicmF3LmdpdGh1YnVzZXJjb250ZW50LmNvbSIsImtleSI6ImtleTUiLCJleHAiOjE3MTYwODk0ODYsIm5iZiI6MTcxNjA4OTE4NiwicGF0aCI6Ii8xMjU1NDc1NzUvMzMxODQ0OTgxLWMwMWVmNmMwLWYxNTEtNGI3Ni1iZWM2LTdkOWQyN2VkODQ4YS5wbmc_WC1BbXotQWxnb3JpdGhtPUFXUzQtSE1BQy1TSEEyNTYmWC1BbXotQ3JlZGVudGlhbD1BS0lBVkNPRFlMU0E1M1BRSzRaQSUyRjIwMjQwNTE5JTJGdXMtZWFzdC0xJTJGczMlMkZhd3M0X3JlcXVlc3QmWC1BbXotRGF0ZT0yMDI0MDUxOVQwMzI2MjZaJlgtQW16LUV4cGlyZXM9MzAwJlgtQW16LVNpZ25hdHVyZT0xMWRiZjY0ZTZhNTIzYmIzY2IyNzY3ODIxOWZmYWIyNGViNGU1N2YzNDlkODc1ZWIwNDUwOWJiZDJhNDY1YjUzJlgtQW16LVNpZ25lZEhlYWRlcnM9aG9zdCZhY3Rvcl9pZD0wJmtleV9pZD0wJnJlcG9faWQ9MCJ9.80gFmFiuboeq9pd3FMzvfh2sbZOjQK7ni_osXpTj3pI)


## Usage
### Set up the build environment
```bash
$ bash setup.sh
```

### Build
```bash
$ bash build-hubserv.sh
```

### Local run for debugging
```bash
$ bash run-hubserv.sh
```

### Run
```bash
$ ./hubserver \
--status-port 9098 \
--http-port 443 \
--public-ip $_IP \
--aws-ec2-region $_REGION \
--use-https \
--cache /home/ec2-user/certs \
--max-connections-by-url 100 \
--http-host verse.example.org
```

## Example for production environments
It is found in ./etc/