#!/usr/bin/env bash
# Install/update rustup.
#
# It is helpful to have this as a separate script due to some issues on
# Windows where immediately after `rustup self update`, rustup can fail with
# "Device or resource busy".

set -e
if [[ -z "$1" ]]; then
    echo "First parameter must be toolchain to install."
    exit 1
fi
TOOLCHAIN="$1"

# Install/update rustup.
RUSTUP_MINOR_VER=$(rustup -V 2> /dev/null | grep -o -E '1\.[0-9]{2}' | cut -d . -f2)
if [[ -n $RUSTUP_MINOR_VER && $RUSTUP_MINOR_VER -ge 20 ]]; then
    echo "$(rustup -V)" already installed
    rustup set profile minimal
else
    curl -sSL https://sh.rustup.rs | sh -s -- -y --default-toolchain="$TOOLCHAIN" --profile=minimal
    echo "##[add-path]$HOME/.cargo/bin"
fi
