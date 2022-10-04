#!/usr/bin/env bash
echo "[]" > ${1}/allow_addrs.json5

for i in ${2}/*.pem;
do
    jq --arg id $(many id ${i}) '. += [$id]' < ${1}/allow_addrs.json5 > ${1}/allow_addrs_tmp.json5
    mv ${1}/allow_addrs_tmp.json5 ${1}/allow_addrs.json5
done
