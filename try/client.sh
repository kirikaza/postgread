#! /bin/sh

ssl_mode="$1"
shift
docker run -it --rm --network host postgres:alpine psql "$ssl_mode host=localhost port=15432 user=postgres" "$@"
