#!/usr/bin/env bash
# Cut the next SingPanel release tag. App version follows that tag.
#
#   ./scripts/release.sh              # bump only if main has commits after the last tag
#   ./scripts/release.sh minor
#   ./scripts/release.sh major
#   ./scripts/release.sh 0.1.0
#   ./scripts/release.sh --retry      # same tag, re-run CI (build failed, code unchanged)
#   ./scripts/release.sh --print patch
#   ./scripts/release.sh --no-push
set -euo pipefail

print_only=0
do_push=1
do_retry=0
bump="patch"

usage() {
  sed -n '2,12p' "$0" | sed 's/^# \{0,1\}//'
  exit "${1:-0}"
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    -h|--help) usage 0 ;;
    --print) print_only=1 ;;
    --no-push) do_push=0 ;;
    --retry) do_retry=1 ;;
    patch|minor|major) bump="$1" ;;
    v*) bump="${1#v}" ;;
    [0-9]*.[0-9]*.[0-9]*) bump="$1" ;;
    *)
      echo "unknown argument: $1" >&2
      usage 1
      ;;
  esac
  shift
done

repo_root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$repo_root"

latest_tag() {
  git tag -l 'v[0-9]*' --sort=-v:refname | head -1
}

tag_commit() {
  git rev-list -n 1 "$1" 2>/dev/null || true
}

next_version() {
  local spec="$1"
  if [[ "$spec" =~ ^[0-9]+\.[0-9]+\.[0-9]+([.-].*)?$ ]]; then
    printf '%s\n' "$spec"
    return
  fi
  local last
  last="$(latest_tag)"
  last="${last#v}"
  if [[ -z "$last" ]]; then
    printf '0.0.1\n'
    return
  fi
  local major minor patch
  IFS='.' read -r major minor patch <<<"${last%%-*}"
  major="${major:-0}"
  minor="${minor:-0}"
  patch="${patch:-0}"
  case "$spec" in
    major) printf '%s.0.0\n' "$((major + 1))" ;;
    minor) printf '%s.%s.0\n' "$major" "$((minor + 1))" ;;
    patch) printf '%s.%s.%s\n' "$major" "$minor" "$((patch + 1))" ;;
    *)
      echo "unknown bump: $spec" >&2
      exit 1
      ;;
  esac
}

retry_same_tag() {
  local tag="$1"
  local ver="${tag#v}"
  echo "HEAD is already ${tag}. Re-running Build & Release for ${ver} (no version bump)."
  if command -v gh >/dev/null 2>&1 && gh auth status >/dev/null 2>&1; then
    local run_id
    run_id="$(gh run list --workflow "Build & Release" --limit 20 \
      --json databaseId,headBranch,event \
      --jq "[.[] | select(.headBranch == \"${tag}\" or .headBranch == \"${ver}\")][0].databaseId" \
      2>/dev/null || true)"
    if [[ -n "${run_id}" && "${run_id}" != "null" ]]; then
      gh run rerun "${run_id}" --failed
      echo "re-ran failed jobs: ${run_id}"
      echo "https://github.com/asdfgh2026/singplane/actions/runs/${run_id}"
      return 0
    fi
    gh workflow run "Build & Release" --ref main \
      -f "version=${ver}" \
      -f create_release=true \
      -f prerelease=false
    echo "started Build & Release for ${ver} from main"
    return 0
  fi
  echo "Open Actions → the failed run → Re-run failed jobs."
  echo "Do not cut a new tag. Version stays ${ver}."
}

if [[ "$print_only" -eq 1 ]]; then
  next_version "$bump"
  exit 0
fi

last="$(latest_tag)"
head="$(git rev-parse HEAD)"

if [[ "$do_retry" -eq 1 ]]; then
  if [[ -z "$last" ]]; then
    echo "no v* tag to retry" >&2
    exit 1
  fi
  retry_same_tag "$last"
  exit 0
fi

if [[ -n "$last" && "$(tag_commit "$last")" == "$head" ]]; then
  echo "HEAD is already tagged ${last}. Code did not change; version stays ${last#v}."
  echo "If CI failed: ./scripts/release.sh --retry"
  echo "A new version is only created after new commits on main."
  exit 1
fi

ver="$(next_version "$bump")"

if [[ -n "$(git status --porcelain)" ]]; then
  echo "warning: working tree is dirty; tagging the last commit only ($(git rev-parse --short HEAD))"
fi

branch="$(git rev-parse --abbrev-ref HEAD)"
if [[ "$branch" != "main" ]]; then
  echo "checkout main before releasing (now on $branch)" >&2
  exit 1
fi

if git rev-parse -q --verify "refs/tags/v${ver}" >/dev/null; then
  echo "tag v${ver} already exists" >&2
  exit 1
fi

git tag -a "v${ver}" -m "SingPanel ${ver}"
echo "created tag v${ver} -> $(git rev-parse --short HEAD)"

if [[ "$do_push" -eq 1 ]]; then
  git push origin "v${ver}"
  echo "pushed v${ver}; Build & Release should start"
else
  echo "not pushed; run: git push origin v${ver}"
fi
