#!/bin/bash
# This script outputs the channel where a CI workflow wants to merge into.
#
# Inputs:
#     BASE_SHA    The commit SHA of the branch where the PR wants to merge into.
#
# GitHub Action Outputs:
#     CHANNEL     Target channel where the PR wants to merge into.

set -euo pipefail

# When `BASE_SHA` is missing, we assume it is from bors merge commit,
# so hope `HEAD~` to find the previous commit on master branch.
base_sha=$(git rev-parse "${BASE_SHA:-HEAD~1}")

# Get symbolic names for the base_sha.
# Assumption: Cargo branches are always in the format of `rust-1.*.0`,
#             otherwise `git name-rev` will return "undefined".
ref=$(git name-rev --name-only --refs='origin/rust-1.*.0' $base_sha)

# Get the latest `rust-1.*.0` branch from remote origin.
# Assumption: The latest branch is always beta branch.
beta=$(
    git branch --remotes --list 'origin/rust-1.*.0' \
    | sort --version-sort \
    | tail -n1 \
    | tr -d "[:space:]"
)

master=$(git rev-parse origin/master)

# Backport pull requests always target at a `rust-1.*.0` branch.
if [[ "$ref" = "undefined" ]] || [[ "$base_sha" = "$master" ]]
then
    # Should be nightly but for convenience in CI let's call it master.
    channel="master"
elif [[ "$ref" = "$beta" ]]
then
    channel="beta"
else
    channel="stable"
fi

echo "Base sha: $base_sha"
echo "Git Ref:  $ref"
echo "master:   $master"
echo "beta:     $beta"
echo "Channel:  $channel"

echo "CHANNEL=$channel" >> "$GITHUB_OUTPUT"
