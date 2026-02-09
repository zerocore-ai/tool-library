#!/usr/bin/env bash
set -euo pipefail

#--------------------------------------------------------------------------------------------------
# vendor-release.sh - Pull and publish vendor mcpb release artifacts
#
# Usage:
#   ./scripts/vendor-release.sh vendor/<mcpb> --pull [--version <tag>]
#   ./scripts/vendor-release.sh vendor/<mcpb> --publish
#   ./scripts/vendor-release.sh vendor/<mcpb> --pull --publish [--version <tag>]
#
# Options:
#   --pull              Download release artifacts from GitHub
#   --publish           Publish bundles via tool-cli multi-platform publish
#   --version <tag>     Specify release version (default: latest)
#   --dry-run           Show what would be done without executing
#   -h, --help          Show this help message
#--------------------------------------------------------------------------------------------------

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

#--------------------------------------------------------------------------------------------------
# Functions
#--------------------------------------------------------------------------------------------------

usage() {
    sed -n '3,14p' "$0" | sed 's/^# \?//'
    exit 0
}

log_info() { echo -e "${BLUE}ℹ${NC} $*"; }
log_success() { echo -e "${GREEN}✓${NC} $*"; }
log_warn() { echo -e "${YELLOW}⚠${NC} $*"; }
log_error() { echo -e "${RED}✗${NC} $*" >&2; }

die() { log_error "$@"; exit 1; }

check_dependencies() {
    local missing=()
    command -v gh &>/dev/null || missing+=("gh (GitHub CLI)")
    command -v tool &>/dev/null || missing+=("tool (tool-cli)")
    command -v jq &>/dev/null || missing+=("jq")

    if [[ ${#missing[@]} -gt 0 ]]; then
        die "Missing dependencies: ${missing[*]}"
    fi
}

get_repo_from_remote() {
    local vendor_dir="$1"
    local remote_url

    remote_url=$(cd "$vendor_dir" && git remote get-url origin 2>/dev/null) || die "Failed to get git remote for $vendor_dir"

    # Extract owner/repo from git@github.com:owner/repo.git or https://github.com/owner/repo.git
    if [[ "$remote_url" =~ github\.com[:/]([^/]+)/([^/.]+)(\.git)?$ ]]; then
        echo "${BASH_REMATCH[1]}/${BASH_REMATCH[2]}"
    else
        die "Could not parse GitHub repo from remote: $remote_url"
    fi
}

check_has_release_workflow() {
    local vendor_dir="$1"
    if [[ ! -f "$vendor_dir/.github/workflows/release.yml" ]]; then
        die "No release workflow found at $vendor_dir/.github/workflows/release.yml"
    fi
}

pull_artifacts() {
    local vendor_dir="$1"
    local version="$2"
    local dry_run="$3"
    local repo artifacts_dir

    repo=$(get_repo_from_remote "$vendor_dir")
    artifacts_dir="$vendor_dir/dist"

    log_info "Pulling artifacts from $repo (version: ${version:-latest})"

    if [[ "$dry_run" == "true" ]]; then
        log_info "[dry-run] Would download to: $artifacts_dir"
        log_info "[dry-run] gh release download ${version:-<latest>} -R $repo -D $artifacts_dir -p '*.mcpb' -p '*.mcpbx' -p '*.sha256' --clobber"
        return 0
    fi

    # Create artifacts directory
    mkdir -p "$artifacts_dir"

    # Download release assets (omit tag for latest)
    local gh_args=(-R "$repo" -D "$artifacts_dir" -p '*.mcpb' -p '*.mcpbx' -p '*.sha256' --clobber)
    [[ -n "$version" ]] && gh_args+=("$version")

    if ! gh release download "${gh_args[@]}"; then
        die "Failed to download release artifacts"
    fi

    # List downloaded files
    log_success "Downloaded artifacts to $artifacts_dir:"
    ls -la "$artifacts_dir"
}

publish_artifacts() {
    local vendor_dir="$1"
    local dry_run="$2"
    local artifacts_dir="$vendor_dir/dist"

    if [[ ! -d "$artifacts_dir" ]]; then
        die "Dist directory not found: $artifacts_dir (run with --pull first)"
    fi

    log_info "Publishing from $artifacts_dir"

    # Find bundles for each platform
    local darwin_arm64="" darwin_x64="" linux_arm64="" linux_x64="" win32_arm64="" win32_x64=""

    for f in "$artifacts_dir"/*.mcpb "$artifacts_dir"/*.mcpbx; do
        [[ -f "$f" ]] || continue
        case "$(basename "$f")" in
            *-darwin-arm64.*) darwin_arm64="$f" ;;
            *-darwin-x86_64.*|*-darwin-x64.*) darwin_x64="$f" ;;
            *-linux-arm64.*) linux_arm64="$f" ;;
            *-linux-x86_64.*|*-linux-x64.*) linux_x64="$f" ;;
            *-win32-arm64.*) win32_arm64="$f" ;;
            *-win32-x86_64.*|*-win32-x64.*) win32_x64="$f" ;;
        esac
    done

    # Build tool publish command
    local publish_args=(publish --multi-platform)
    [[ -n "$darwin_arm64" ]] && publish_args+=(--darwin-arm64 "$darwin_arm64")
    [[ -n "$darwin_x64" ]] && publish_args+=(--darwin-x64 "$darwin_x64")
    [[ -n "$linux_arm64" ]] && publish_args+=(--linux-arm64 "$linux_arm64")
    [[ -n "$linux_x64" ]] && publish_args+=(--linux-x64 "$linux_x64")
    [[ -n "$win32_arm64" ]] && publish_args+=(--win32-arm64 "$win32_arm64")
    [[ -n "$win32_x64" ]] && publish_args+=(--win32-x64 "$win32_x64")

    # Check we found at least one bundle
    if [[ -z "$darwin_arm64" && -z "$darwin_x64" && -z "$linux_arm64" && -z "$linux_x64" && -z "$win32_arm64" && -z "$win32_x64" ]]; then
        die "No platform bundles found in $artifacts_dir"
    fi

    log_info "Found bundles:"
    [[ -n "$darwin_arm64" ]] && log_info "  darwin-arm64: $(basename "$darwin_arm64")"
    [[ -n "$darwin_x64" ]] && log_info "  darwin-x64:   $(basename "$darwin_x64")"
    [[ -n "$linux_arm64" ]] && log_info "  linux-arm64:  $(basename "$linux_arm64")"
    [[ -n "$linux_x64" ]] && log_info "  linux-x64:    $(basename "$linux_x64")"
    [[ -n "$win32_arm64" ]] && log_info "  win32-arm64:  $(basename "$win32_arm64")"
    [[ -n "$win32_x64" ]] && log_info "  win32-x64:    $(basename "$win32_x64")"

    if [[ "$dry_run" == "true" ]]; then
        log_info "[dry-run] Would run: (cd $vendor_dir && tool ${publish_args[*]})"
        return 0
    fi

    log_info "Running: tool ${publish_args[*]}"
    (cd "$vendor_dir" && tool "${publish_args[@]}")

    log_success "Published successfully"
}

#--------------------------------------------------------------------------------------------------
# Main
#--------------------------------------------------------------------------------------------------

main() {
    local vendor_path=""
    local do_pull=false
    local do_publish=false
    local version=""
    local dry_run=false

    # Parse arguments
    while [[ $# -gt 0 ]]; do
        case "$1" in
            -h|--help) usage ;;
            --pull) do_pull=true; shift ;;
            --publish) do_publish=true; shift ;;
            --version) version="$2"; shift 2 ;;
            --dry-run) dry_run=true; shift ;;
            -*)
                die "Unknown option: $1"
                ;;
            *)
                if [[ -z "$vendor_path" ]]; then
                    vendor_path="$1"
                else
                    die "Unexpected argument: $1"
                fi
                shift
                ;;
        esac
    done

    # Validate arguments
    [[ -z "$vendor_path" ]] && die "Missing vendor path. Usage: $0 vendor/<mcpb> --pull|--publish"
    [[ "$do_pull" == "false" && "$do_publish" == "false" ]] && die "Must specify --pull and/or --publish"

    # Resolve vendor directory
    local vendor_dir
    if [[ "$vendor_path" = /* ]]; then
        vendor_dir="$vendor_path"
    else
        vendor_dir="$ROOT_DIR/$vendor_path"
    fi

    [[ -d "$vendor_dir" ]] || die "Vendor directory not found: $vendor_dir"

    check_dependencies

    # Check for release workflow if pulling
    if [[ "$do_pull" == "true" ]]; then
        check_has_release_workflow "$vendor_dir"
    fi

    # Execute operations
    if [[ "$do_pull" == "true" ]]; then
        pull_artifacts "$vendor_dir" "$version" "$dry_run"
    fi

    if [[ "$do_publish" == "true" ]]; then
        publish_artifacts "$vendor_dir" "$dry_run"
    fi
}

main "$@"
