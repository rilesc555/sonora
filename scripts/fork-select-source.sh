#!/bin/bash
# Select a checked source-only release PR's merge commit, allowing GitHub time to prepare it.
# The caller must use a trusted checkout and pass GitHub's output file and read-only token.
set -euo pipefail

pr=${1:?pass the stable release PR number}
[[ "$pr" =~ ^[0-9]+$ ]]
: "${GH_REPO:?set GH_REPO}"
: "${GITHUB_OUTPUT:?set GITHUB_OUTPUT}"

release=$(bash scripts/fork-latest-release.sh)
tag=$(jq -r .tag <<< "$release")
commit=$(jq -r .commit <<< "$release")
branch="sync/release-$tag-${commit:0:12}"
marker=packaging/fork-upstream.json
checkout=$(git rev-parse HEAD)

for attempt in {1..36}; do
  proposal=$(gh api "repos/$GH_REPO/pulls/$pr")
  if ! jq -e --arg repo "$GH_REPO" --arg branch "$branch" '
    .base.ref == "main" and
    .head.ref == $branch and
    .head.repo.full_name == $repo and
    .user.login == "github-actions[bot]" and
    .state == "open"
  ' <<< "$proposal" > /dev/null; then
    echo "::error::PR #$pr is not a bot-created stable release update."
    exit 1
  fi

  head=$(jq -r .head.sha <<< "$proposal")
  base=$(jq -r .base.sha <<< "$proposal")
  if [[ "$base" != "$checkout" ]]; then
    echo "::error::Stable release PR #$pr does not start from the checked main branch."
    exit 1
  fi
  git fetch --no-tags origin "refs/heads/$branch"
  if [[ "$(git rev-parse FETCH_HEAD)" != "$head" ]]; then
    echo "::error::Stable release PR #$pr changed while it was checked."
    exit 1
  fi
  read -ra parents <<< "$(git rev-list --parents -n 1 "$head")"
  if (( ${#parents[@]} != 2 )) || [[ "${parents[1]}" != "$base" ]]; then
    echo "::error::Stable release PR #$pr is not one update commit on main."
    exit 1
  fi
  recorded=$(git show "$head:$marker")
  if ! jq -e --arg tag "$tag" --arg commit "$commit" '
    .tag == $tag and .commit == $commit
  ' <<< "$recorded" > /dev/null; then
    echo "::error::Stable release PR #$pr does not record official release $tag."
    exit 1
  fi
  if ! git diff --quiet "$base" "$head" -- .github/workflows; then
    echo "::error::Stable release PR #$pr changes fork workflow files."
    exit 1
  fi
  if jq -e '.mergeable == false' <<< "$proposal" > /dev/null; then
    echo "::error::Stable release PR #$pr has merge conflicts. The next sync will rebuild it."
    exit 1
  fi

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
