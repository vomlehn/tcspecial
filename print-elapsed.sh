#!/bin/bash

set -eu

if [ $# -ne 1 ]; then
	echo "Must supply a single numeric argument" 1>&2
	exit 1
fi

start_time="$1"
end_time=$(date +"%s")
delta=$((end_time - start_time))

sec=$((delta % 60))
delta=$((delta / 60));

min=$((delta % 60))
delta=$((delta / 60))

hour=$delta

printf "Elapsed time: %u:%02u:%02u\\n" $hour $min $sec
