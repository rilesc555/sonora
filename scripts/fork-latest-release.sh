#!/bin/bash
# Resolve GitHub's latest official stable release to its exact tagged commit, not main.
set -euo pipefail

release=$(gh api repos/sonorahq/sonora/releases/latest)
jq -e '
  .draft == false and .prerelease == false and
  (.tag_name | test("^v[0-9]+\\.[0-9]+\\.[0-9]+$"))
' <<< "$release" > /dev/null
tag=$(jq -r .tag_name <<< "$release")
git fetch --no-tags https://github.com/sonorahq/sonora.git "refs/tags/$tag"
commit=$(git rev-parse 'FETCH_HEAD^{commit}')
jq -n --arg tag "$tag" --arg commit "$commit" '{tag: $tag, commit: $commit}'
