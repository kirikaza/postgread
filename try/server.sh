#! /bin/sh

if [ "$1" = ssl ] ; then
  ssl_enabled=on
else
  ssl_enabled=off
fi

set -x
cd "`dirname "$0"`" || exit 1
vm_dir=/var/lib/postgresql
docker run -it --rm --network host \
    -v "$PWD"/cert.pem:$vm_dir/cert.pem \
    -v "$PWD"/key.pem:$vm_dir/key.pem \
    postgres:alpine \
    -c ssl=${ssl_enabled} \
    -c ssl_cert_file=$vm_dir/cert.pem \
    -c ssl_key_file=$vm_dir/key.pem \
    -c log_min_messages=DEBUG2 |  # to log SSL connections
    grep -v 'writing stats file'  # too many such lines