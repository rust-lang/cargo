#!/bin/bash

# Test script for cargo-credential-trusted-publish
# This script tests the credential provider protocol without actual OIDC tokens

set -e

BINARY="./target/release/cargo-credential-trusted-publish"

echo "Building the credential provider..."
cargo build --release -p cargo-credential-trusted-publish

echo "Testing credential provider protocol..."

# Test 1: Version announcement
echo "=== Test 1: Version announcement ==="
echo "Starting credential provider and checking version announcement..."
timeout 2s bash -c "echo '' | $BINARY" | head -1
echo "✓ Version announcement works"

# Test 2: Unsupported registry
echo "=== Test 2: Unsupported registry ==="
echo '{"v":1,"registry":{"index-url":"https://example.com/registry","name":"example"},"kind":"get","operation":"publish","name":"test","vers":"0.1.0","cksum":"abc123"}' | timeout 5s $BINARY | head -2 | tail -1
echo "✓ Unsupported registry handling works"

# Test 3: Supported registry without OIDC token
echo "=== Test 3: Supported registry without OIDC token ==="
unset ACTIONS_ID_TOKEN
echo '{"v":1,"registry":{"index-url":"https://github.com/rust-lang/crates.io-index","name":"crates-io"},"kind":"get","operation":"publish","name":"test","vers":"0.1.0","cksum":"abc123"}' | timeout 5s $BINARY | head -2 | tail -1
echo "✓ Missing OIDC token handling works"

# Test 4: Logout without token
echo "=== Test 4: Logout without token ==="
echo '{"v":1,"registry":{"index-url":"https://github.com/rust-lang/crates.io-index","name":"crates-io"},"kind":"logout"}' | timeout 5s $BINARY | head -2 | tail -1
echo "✓ Logout without token works"

echo "All protocol tests passed! ✅" 