#!/usr/bin/env bash

[ "$1" ] || {
  echo You need to pass in a destination pem file.
  exit 1
}

openssl genpkey -algorithm Ed25519 -out $1
