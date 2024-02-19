#!/bin/sh

while read line
do
    echo -n "$line" | base64 --decode
done < "${1:-/dev/stdin}"
