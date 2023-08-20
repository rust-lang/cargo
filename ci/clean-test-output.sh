#!/bin/bash
# This script remove test and benchmark output and displays disk usage.

set -euo pipefail

df -h
rm -rf target/tmp
df -h
