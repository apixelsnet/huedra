#!/usr/bin/env bash
# Locks the GitHub repository down so only its admins change what ships:
# - main: no deletion or force push; changes come through a pull request
#   with one approval and both CI checks green. Admins may push directly.
# - Tags v* and update-channel: only admins and the Release workflow
#   (GitHub Actions) may create, move or delete them.
# - Workflows get a read-only token unless a job asks for more, and runs
#   for pull requests from outside contributors wait for an admin.
# Repository rules need a public repository (or a paid plan). Run once, as an
# admin, with the GitHub CLI logged in:  bash build/protect-repo.sh
# Feature doc: docs/features/12-packaging.md.
set -euo pipefail

repo=$(gh repo view --json nameWithOwner --jq .nameWithOwner)
admin='{ "actor_id": 5, "actor_type": "RepositoryRole", "bypass_mode": "always" }'
actions='{ "actor_id": 15368, "actor_type": "Integration", "bypass_mode": "always" }'

ruleset() {
  local name=$1 body=$2 id
  id=$(gh api "repos/$repo/rulesets" --jq ".[] | select(.name == \"$name\") | .id")
  if [ -n "$id" ]; then
    gh api -X PUT "repos/$repo/rulesets/$id" --input - <<<"$body" --silent
    echo "updated: $name"
  else
    gh api -X POST "repos/$repo/rulesets" --input - <<<"$body" --silent
    echo "created: $name"
  fi
}

ruleset "Protect main" '{
  "name": "Protect main",
  "target": "branch",
  "enforcement": "active",
  "conditions": { "ref_name": { "include": ["~DEFAULT_BRANCH"], "exclude": [] } },
  "bypass_actors": ['"$admin"'],
  "rules": [
    { "type": "deletion" },
    { "type": "non_fast_forward" },
    { "type": "pull_request", "parameters": {
        "required_approving_review_count": 1,
        "dismiss_stale_reviews_on_push": true,
        "require_code_owner_review": false,
        "require_last_push_approval": true,
        "required_review_thread_resolution": false } },
    { "type": "required_status_checks", "parameters": {
        "strict_required_status_checks_policy": false,
        "required_status_checks": [
          { "context": "Check, test and build" },
          { "context": "Run on Windows and pack" } ] } }
  ]
}'

ruleset "Protect release tags" '{
  "name": "Protect release tags",
  "target": "tag",
  "enforcement": "active",
  "conditions": { "ref_name": { "include": ["refs/tags/v*", "refs/tags/update-channel"], "exclude": [] } },
  "bypass_actors": ['"$admin"', '"$actions"'],
  "rules": [ { "type": "creation" }, { "type": "update" }, { "type": "deletion" } ]
}'

gh api -X PUT "repos/$repo/actions/permissions/workflow" \
  -f default_workflow_permissions=read -F can_approve_pull_request_reviews=false --silent
echo "workflow token: read-only"

gh api -X PUT "repos/$repo/actions/permissions/fork-pr-contributor-approval" \
  -f approval_policy=all_external_contributors --silent
echo "outside contributors' workflow runs: wait for approval"
