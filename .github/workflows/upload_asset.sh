#!/bin/bash
set -euo pipefail

# Upload a release asset using the GitHub CLI.
# Usage: upload_asset.sh <FILE> <TOKEN>
#
# The gh CLI handles draft releases, duplicate asset deletion, retries,
# and multipart uploads correctly out of the box.

if [ $# -lt 2 ]; then
    echo "Usage: upload_asset.sh <FILE> <TOKEN>"
    exit 1
fi

file_path=$1
export GH_TOKEN=$2

tag="$(git describe --tags --abbrev=0)"
if [ -z "$tag" ]; then
    printf "\e[31mError: Unable to find git tag\e[0m\n"
    exit 1
fi

file_name="${file_path##*/}"
echo "Uploading $file_name to release $tag..."

# Delete existing asset with the same name (gh upload --clobber requires
# a published release; for drafts we delete manually).
existing_id=$(
    gh api "repos/{owner}/{repo}/releases" --paginate -q \
        ".[] | select(.tag_name == \"$tag\") | .assets[] | select(.name == \"$file_name\") | .id" \
    2>/dev/null || true
)
if [ -n "$existing_id" ]; then
    echo "Deleting existing asset $file_name (id: $existing_id)..."
    gh api -X DELETE "repos/{owner}/{repo}/releases/assets/$existing_id" 2>/dev/null || true
fi

# Upload. gh release upload works with draft releases.
gh release upload "$tag" "$file_path" --clobber

printf "\e[32mSuccess\e[0m\n"
