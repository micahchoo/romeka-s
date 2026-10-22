#!/usr/bin/env bash
# The held-out arrangement of the import proposer (see the manifest's header).
#
#   heldout.sh strip [worktree]      leave every manifest path out of a build
#                                    worktree (a non-cone sparse checkout)
#   heldout.sh verify <branch> [base]
#                                    fail when the branch, since it left base
#                                    (default main), touches a manifest path
#                                    or the manifest
#   heldout.sh deny                  print Read and Grep deny rules of every
#                                    manifest path, for a builder worktree's
#                                    .claude/settings.local.json
set -euo pipefail

MANIFEST=crates/omeka-server/tests/heldout/HELDOUT

die() { echo "heldout: $*" >&2; exit 2; }

# The manifest's lines from a text on stdin: no comments, no blank lines.
entries() { sed -e 's/[[:space:]]*$//' | grep -v -e '^#' -e '^$' || true; }

strip() {
  local tree=${1:-.}
  cd "$tree"
  [[ -f $MANIFEST ]] || die "no $MANIFEST in $(pwd)"
  [[ -z $(git status --porcelain) ]] || die "$(pwd) has changes; strip a clean worktree"
  local patterns=('/*') path
  while IFS= read -r path; do
    case $path in
      ../*) echo "outside the repository, not stripped: $path" ;;
      *) patterns+=("!/$path") ;;
    esac
  done < <(entries <"$MANIFEST")
  git sparse-checkout set --no-cone "${patterns[@]}"
  local left=0
  while IFS= read -r path; do
    [[ $path == ../* ]] && continue
    if [[ -e $path ]]; then echo "still on disk: $path" >&2; left=1; fi
  done < <(entries <"$MANIFEST")
  [[ $left == 0 ]] || die "strip left held-out files on disk"
  [[ -z $(git status --porcelain) ]] || die "strip changed the worktree status"
  echo "stripped $(( ${#patterns[@]} - 1 )) held-out paths from $(pwd)"
}

verify() {
  local branch=${1:?usage: heldout.sh verify <branch> [base]} base=${2:-main}
  # The manifest of either side, so a branch that adds or empties it is caught.
  local listed
  listed=$( { git show "$base:$MANIFEST" 2>/dev/null || true
              git show "$branch:$MANIFEST" 2>/dev/null || true; } | entries | sort -u)
  [[ -n $listed ]] || die "neither $base nor $branch has $MANIFEST"
  # --no-renames: a held-out file moved away shows as its old path deleted.
  local changed path bad=0
  changed=$(git diff --no-renames --name-only "$base...$branch")
  while IFS= read -r path; do
    [[ -z $path ]] && continue
    if [[ $path == "$MANIFEST" ]]; then
      echo "changes the manifest: $path"; bad=1; continue
    fi
    while IFS= read -r held; do
      [[ $held == ../* ]] && continue
      if [[ $path == "$held" || ( $held == */ && $path == "$held"* ) ]]; then
        echo "touches a held-out path: $path"; bad=1
      fi
    done <<<"$listed"
  done <<<"$changed"
  if [[ $bad == 1 ]]; then
    echo "heldout: $branch touches held-out files since $base" >&2
    exit 1
  fi
  echo "heldout: $branch touches no held-out file since $base"
}

deny() {
  [[ -f $MANIFEST ]] || die "no $MANIFEST in $(pwd)"
  local path rules=() i main
  # A `../` line is relative to the main checkout, which every worktree shares.
  main=$(dirname "$(git rev-parse --path-format=absolute --git-common-dir)")
  while IFS= read -r path; do
    case $path in
      ../*) path="/$(realpath -m "$main/$path")/**" ;;
      */) path="/$path**" ;;
      *) path="/$path" ;;
    esac
    rules+=("Read($path)" "Grep($path)")
  done < <(entries <"$MANIFEST")
  printf '{ "permissions": { "deny": [\n'
  for i in "${!rules[@]}"; do
    printf '  "%s"%s\n' "${rules[$i]}" "$( (( i < ${#rules[@]} - 1 )) && echo ,)"
  done
  printf '] } }\n'
}

case ${1:-} in
  strip) shift; strip "$@" ;;
  verify) shift; verify "$@" ;;
  deny) shift; deny "$@" ;;
  *) die "usage: heldout.sh strip [worktree] | verify <branch> [base] | deny" ;;
esac
