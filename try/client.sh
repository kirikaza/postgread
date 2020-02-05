#! /bin/sh

docker run -it --rm --network host postgres:alpine psql 'sslmode=disable host=localhost port=15432 user=postgres' "$@"
