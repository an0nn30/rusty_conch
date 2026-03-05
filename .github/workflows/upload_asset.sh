#!/bin/bash

# Assure parameters are correct.
if [ $# -lt 2 ]; then
    echo "Usage: upload_asset.sh <FILE> <TOKEN>"
    exit 1
fi

repo="an0nn30/rusty_conch"
file_path=$1
bearer=$2

echo "Starting asset upload from $file_path to $repo."

# Get the release for this tag.
tag="$(git describe --tags --abbrev=0)"

# Make sure the git tag could be determined.
if [ -z "$tag" ]; then
    printf "\e[31mError: Unable to find git tag\e[0m\n"
    exit 1
fi

echo "Git tag: $tag"

# Get the upload URL and release ID for the current tag.
#
# Since this might be a draft release, we can't just use the /releases/tags/:tag
# endpoint which only shows published releases.
echo "Checking for existing release..."
releases_json=$(\
    curl \
        --http1.1 \
        -H "Authorization: Bearer $bearer" \
        "https://api.github.com/repos/$repo/releases" \
        2> /dev/null \
)

upload_url=$(\
    echo "$releases_json" \
    | grep -E "(upload_url|tag_name)" \
    | paste - - \
    | grep -e "tag_name\": \"$tag\"" \
    | head -n 1 \
    | sed 's/.*\(https.*assets\).*/\1/' \
)

release_id=$(\
    echo "$releases_json" \
    | grep -E "(\"id\":|tag_name)" \
    | paste - - \
    | grep -e "tag_name\": \"$tag\"" \
    | head -n 1 \
    | sed 's/.*"id": \([0-9]*\).*/\1/' \
)

# Create a new release if we didn't find one for this tag.
if [ -z "$upload_url" ]; then
    echo "No release found."
    echo "Creating new release..."

    # Create new release.
    response=$(
        curl -f \
            --http1.1 \
            -X POST \
            -H "Authorization: Bearer $bearer" \
            -d "{\"tag_name\":\"$tag\",\"draft\":true}" \
            "https://api.github.com/repos/$repo/releases" \
            2> /dev/null\
    )

    # Abort if the release could not be created.
    if [ $? -ne 0 ]; then
        printf "\e[31mError: Unable to create new release.\e[0m\n"
        exit 1;
    fi

    # Extract upload URL and release ID from new release.
    upload_url=$(\
        echo "$response" \
        | grep "upload_url" \
        | sed 's/.*: "\(.*\){.*/\1/' \
    )
    release_id=$(\
        echo "$response" \
        | grep '"id":' \
        | head -n 1 \
        | sed 's/.*"id": \([0-9]*\).*/\1/' \
    )
fi

# Propagate error if no URL for asset upload could be found.
if [ -z "$upload_url" ]; then
    printf "\e[31mError: Unable to find release upload url.\e[0m\n"
    exit 2
fi

# Delete existing asset with the same name (if any) to allow re-upload.
file_name=${file_path##*/}
if [ -n "$release_id" ]; then
    existing_asset_id=$(\
        curl \
            --http1.1 \
            -H "Authorization: Bearer $bearer" \
            "https://api.github.com/repos/$repo/releases/$release_id/assets" \
            2> /dev/null \
        | grep -B 2 "\"name\": \"$file_name\"" \
        | grep '"id":' \
        | sed 's/.*"id": \([0-9]*\).*/\1/' \
    )
    if [ -n "$existing_asset_id" ]; then
        echo "Deleting existing asset $file_name (id: $existing_asset_id)..."
        curl -f \
            --http1.1 \
            -X DELETE \
            -H "Authorization: Bearer $bearer" \
            "https://api.github.com/repos/$repo/releases/assets/$existing_asset_id" \
            &> /dev/null
    fi
fi

# Upload the file to the tag's release.
echo "Uploading asset $file_name to $upload_url..."
curl -f \
    --http1.1 \
    -X POST \
    -H "Authorization: Bearer $bearer" \
    -H "Content-Type: application/octet-stream" \
    --data-binary @"$file_path" \
    "$upload_url?name=$file_name" \
    &> /dev/null \
|| { \
    printf "\e[31mError: Unable to upload asset.\e[0m\n" \
    && exit 3; \
}

printf "\e[32mSuccess\e[0m\n"
