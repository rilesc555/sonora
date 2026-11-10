#!/bin/bash
# Select a bot-created upstream PR's exact merge commit, allowing GitHub time to prepare it.
# The caller must use a trusted checkout and pass GitHub's output file and read-only token.
set -euo pipefail

pr=${1:?pass the upstream PR number}
[[ "$pr" =~ ^[0-9]+$ ]]
: "${GH_REPO:?set GH_REPO}"
: "${GITHUB_OUTPUT:?set GITHUB_OUTPUT}"

for attempt in {1..36}; do
  proposal=$(gh api "repos/$GH_REPO/pulls/$pr")
  jq -e --arg repo "$GH_REPO" '
    .base.ref == "main" and
    (.head.ref | startswith("sync/upstream-")) and
    .head.repo.full_name == $repo and
    .user.login == "github-actions[bot]" and
    .state == "open"
  ' <<< "$proposal" > /dev/null
  if jq -e '.mergeable == false' <<< "$proposal" > /dev/null; then
    echo "::error::Upstream PR #$pr has merge conflicts. Resolve them before retrying."
    exit 1
  fi

  head=$(jq -r .head.sha <<< "$proposal")
  base=$(jq -r .base.sha <<< "$proposal")
  if git fetch origin "refs/pull/$pr/merge" &&
    [[ "$(git rev-parse FETCH_HEAD^1)" == "$base" ]] &&
    [[ "$(git rev-parse FETCH_HEAD^2)" == "$head" ]]; then
    git checkout --detach FETCH_HEAD
    {
      echo "pr=$pr"
      echo "head=$head"
      echo "base=$base"
    } >> "$GITHUB_OUTPUT"
    exit 0
  fi

  if (( attempt < 36 )); then
    echo "::notice::Waiting for GitHub to prepare PR #$pr's current merge ref ($attempt/36)."
    sleep 5
  fi
done

echo "::error::GitHub did not provide a current merge ref for PR #$pr. The next sync will retry."
exit 1
