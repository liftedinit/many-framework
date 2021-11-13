#!/usr/bin/env bash

[ "$1" ] || {
  echo You need to pass in a destination pem file.
  exit 1
}

( [ "$(openssl version)" ] && grep 3\\.0 ) || {
  echo "You need OpenSSL version 3 or superior (support for Ed25519)."
}

openssl genpkey -algorithm Ed25519 -out $1
