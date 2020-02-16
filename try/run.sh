#! /bin/sh

this_dir=`dirname "$0"`
cargo run -- --listen-port 15432 --target-host 127.0.0.1 --cert-p12-file "$this_dir"/cert.p12