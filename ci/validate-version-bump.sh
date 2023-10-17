#!/bin/bash
# This script checks if a crate needs a version bump.
#
# At the time of writing, it doesn't check what kind of bump is required.
# In the future, we could take SemVer compatibliity into account, like
# integrating `cargo-semver-checks` of else
#
# Inputs:
#     BASE_SHA    The commit SHA of the branch where the PR wants to merge into.
#     HEAD_SHA    The commit SHA that triggered the workflow.

set -euo pipefail

# When `BASE_SHA` is missing, we assume it is from bors merge commit,
# so hope `HEAD~` to find the previous commit on master branch.
base_sha=$(git rev-parse "${BASE_SHA:-HEAD~1}")
head_sha=$(git rev-parse "${HEAD_SHA:-HEAD}")

echo "Base revision is $base_sha"
echo "Head revision is $head_sha"

cargo bump-check --base-rev "$base_sha" --head-rev "$head_sha"
