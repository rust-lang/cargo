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

echo "Base branch  is $base_sha"
echo "Current head is $head_sha"

# Gets crate names of members that has been changed from $bash_sha to $head_sha.
changed_crates=$(
    git diff --name-only "$base_sha" "$head_sha" -- crates/ credential/ benches/ \
    | cut  -d'/' -f2 \
    | sort -u
)

if  [ -z "$changed_crates" ]
then
    echo "No file changed in member crates."
    exit 0
fi

# Checks publish status for only crates with code changes.
publish_status_table=$(
    echo "$changed_crates" \
    | xargs printf -- '--package %s\n' \
    | xargs cargo unpublished
)

# "yes" -> code changed but no version difference -> need a bump
# Prints 2nd column (sep by space), which is the name of the crate.
crates_need_bump=$(
    echo "$publish_status_table" \
    | { grep '| yes ' || true; } \
    | awk '{print $2}'
)

if  [ -z "$crates_need_bump" ]
then
    echo "No version bump needed for member crates."
    exit 0
fi

echo "Detected changes in these crates but no version bump found:"
echo "$crates_need_bump"
echo
echo "Please bump at least one patch version for each corresponding Cargo.toml:"
echo 'Run "cargo unpublished" to read the publish status table for details.'
exit 1
