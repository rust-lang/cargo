#!/bin/bash
# This script dumps information about the build environment to stdout.

set -euo pipefail
IFS=$'\n\t'

echo "environment variables:"
printenv | sort
echo

echo "disk usage:"
df -h
echo

echo "CPU info:"
if [[ "${OSTYPE}" = "darwin"* ]]; then
    system_profiler SPHardwareDataType || true
    sysctl hw || true
else
    cat /proc/cpuinfo || true
    cat /proc/meminfo || true
fi
