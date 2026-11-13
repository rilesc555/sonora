#!/bin/bash
# Find a successful same-repository build of this exact tree under this exact workflow.
# Missing artifacts or an unavailable API leave the caller on the normal full-build path.
set -euo pipefail

: "${GH_REPO:?set GH_REPO}"
: "${GITHUB_OUTPUT:?set GITHUB_OUTPUT}"

workflow=.github/workflows/fork-package.yml
tree=$(git rev-parse 'HEAD^{tree}')
definition=$(git rev-parse "HEAD:$workflow")
temporary=$(mktemp -d)
trap 'rm -rf "$temporary"' EXIT

if ! runs=$(gh api "repos/$GH_REPO/actions/workflows/fork-package.yml/runs?status=success&per_page=30"); then
    echo "::notice::Cannot look up checked builds; building the source again."
    exit 0
fi

while read -r run event head; do
    # Never accept builds from another repository, or this publication run itself.
    [[ "$run" != "${GITHUB_RUN_ID:-}" ]] || continue
    git fetch --quiet --no-tags origin "$head" || continue
    [[ "$(git rev-parse "$head:$workflow" 2>/dev/null)" == "$definition" ]] || continue
    directory="$temporary/$run"
    if ! gh run download "$run" --repo "$GH_REPO" --name "fork-source-$run" --dir "$directory"; then
        continue
    fi
    checked=$(jq -er '.sha | select(test("^[0-9a-f]{40}$"))' "$directory/source.json") || continue
    git fetch --quiet --no-tags origin "$checked" || continue
    [[ "$(git rev-parse "$checked^{tree}")" == "$tree" ]] || continue
    case "$event" in
        pull_request)
            # GitHub builds a synthetic merge for a PR, not the PR head alone.
            [[ "$(git rev-parse "$checked^2" 2>/dev/null)" == "$head" ]] || continue
            ;;
        workflow_dispatch)
            # The trusted dispatch selects and validates its release merge in the source job.
            ;;
        *) continue ;;
    esac
    # Both immutable platform artifacts must still be available.
    artifacts=$(gh api "repos/$GH_REPO/actions/runs/$run/artifacts?per_page=100") || continue
    jq -e --arg linux "fork-build-$run" --arg windows "fork-windows-$run" '
        [.artifacts[] | select(.expired == false) | .name] |
        index($linux) != null and index($windows) != null
    ' <<< "$artifacts" > /dev/null || continue

    base_version=$(git show "$checked:Cargo.toml" | sed -n 's/^version = "\(.*\)"/\1/p' | head -n 1)
    revision=$(git rev-list --count "$checked")
    short=$(git rev-parse --short=12 "$checked")
    {
        echo "artifact_run=$run"
        echo "build_sha=$checked"
        echo "version=$base_version.r$revision.g$short"
        echo "app_version=$base_version-rilesc555.r$revision.g$short"
    } >> "$GITHUB_OUTPUT"
    echo "::notice::Reusing checked Linux and Windows artifacts from run $run, source $checked."
    exit 0
done < <(jq -r --arg repo "$GH_REPO" '
    .workflow_runs[] |
    select(.head_repository.full_name == $repo) |
    select(.event == "pull_request" or .event == "workflow_dispatch") |
    [.id, .event, .head_sha] | @tsv
' <<< "$runs")

echo "::notice::No matching checked artifacts; building the source again."
