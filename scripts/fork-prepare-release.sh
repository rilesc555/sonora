#!/bin/bash
# Create one fork-owned commit from an official stable release's source changes.
set -euo pipefail

tag=${1:?pass the official stable release tag}
upstream=${2:?pass the official stable release commit}
branch=${3:?pass the update branch}
marker=packaging/fork-upstream.json
official=https://github.com/sonorahq/sonora.git

[[ "$tag" =~ ^v[0-9]+\.[0-9]+\.[0-9]+$ ]]
[[ "$upstream" =~ ^[0-9a-f]{40}$ ]]
[[ "$branch" == "sync/release-$tag-${upstream:0:12}" ]]
git diff --quiet
git diff --cached --quiet

base_tag=$(jq -er '.tag' "$marker")
base_commit=$(jq -er '.commit' "$marker")
[[ "$base_tag" =~ ^v[0-9]+\.[0-9]+\.[0-9]+$ ]]
[[ "$base_commit" =~ ^[0-9a-f]{40}$ ]]
[[ "$base_commit" != "$upstream" ]]

fetch_tag() {
  local wanted=$1
  git fetch --no-tags "$official" "refs/tags/$wanted" >&2
  git rev-parse 'FETCH_HEAD^{commit}'
}

test "$(fetch_tag "$base_tag")" = "$base_commit"
test "$(fetch_tag "$tag")" = "$upstream"
if ! git merge-base --is-ancestor "$base_commit" "$upstream"; then
  echo "::error::Official release $tag does not follow recorded release $base_tag."
  exit 1
fi

base=$(git rev-parse HEAD)
git switch -C "$branch" "$base"

if ! git merge-base --is-ancestor "$upstream" "$base"; then
  common=$(git merge-base "$base" "$upstream")
  if git merge-base --is-ancestor "$base_commit" "$common"; then
    apply_from=$common
  elif git merge-base --is-ancestor "$common" "$base_commit"; then
    apply_from=$base_commit
  else
    echo "::error::The fork and recorded release have incompatible upstream histories."
    exit 1
  fi
  temporary=${RUNNER_TEMP:-${TMPDIR:-/tmp}}
  patch=$(mktemp "$temporary/sonora-release.XXXXXX.patch")
  trap 'rm -f "$patch"' EXIT
  git diff --binary --full-index --no-renames "$apply_from" "$upstream" -- \
    . ':(exclude).github/workflows/**' > "$patch"
  if ! git apply --3way --index --quiet "$patch"; then
    echo "::error::Official release $tag conflicts with fork source. Prepare a manual maintenance update."
    exit 1
  fi
fi

if ! git diff --cached --quiet "$base" -- .github/workflows; then
  echo "::error::The release import changed fork workflow files."
  exit 1
fi

jq -n --arg tag "$tag" --arg commit "$upstream" \
  '{tag: $tag, commit: $commit}' > "$marker"
git add "$marker"
git config user.name 'github-actions[bot]'
git config user.email '41898282+github-actions[bot]@users.noreply.github.com'
git commit -m "chore: import official release $tag"
