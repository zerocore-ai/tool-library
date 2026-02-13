#!/usr/bin/env bash
set -euo pipefail

#--------------------------------------------------------------------------------------------------
# release.sh - Pull and publish mcpb release artifacts from submodules
#
# Usage:
#   ./scripts/release.sh <paths...> --pull [--version <tag>]
#   ./scripts/release.sh <paths...> --publish
#   ./scripts/release.sh <paths...> --pull --publish [--version <tag>]
#
# Examples:
#   ./scripts/release.sh external/github --pull --publish
#   ./scripts/release.sh core/bash --pull --version v0.1.0
#   ./scripts/release.sh core/* --pull --publish
#   ./scripts/release.sh external/* core/web --publish
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
CYAN='\033[0;36m'
DIM='\033[2m'
BOLD='\033[1m'
NC='\033[0m' # No Color

#--------------------------------------------------------------------------------------------------
# Functions
#--------------------------------------------------------------------------------------------------

usage() {
    echo -e "${BOLD}release.sh${NC} - Pull and publish mcpb release artifacts from submodules

${BOLD}USAGE${NC}
    ./scripts/release.sh <paths...> [options]

${BOLD}ARGUMENTS${NC}
    <paths...>          One or more paths to mcpb submodules
                        Supports glob patterns: core/* external/*

${BOLD}OPTIONS${NC}
    --pull              Download release artifacts from GitHub Releases
    --publish           Publish bundles to tool.store registry
    --version <tag>     Specify release version (default: latest)
    --dry-run           Show what would be done without executing
    -h, --help          Show this help message

${BOLD}EXAMPLES${NC}
    ${DIM}# Pull latest release for a single package${NC}
    ./scripts/release.sh external/github --pull

    ${DIM}# Pull and publish all external packages${NC}
    ./scripts/release.sh external/* --pull --publish

    ${DIM}# Pull specific version${NC}
    ./scripts/release.sh core/bash --pull --version v0.2.0

    ${DIM}# Dry run to see what would happen${NC}
    ./scripts/release.sh core/* external/* --pull --publish --dry-run

${BOLD}WORKFLOW${NC}
    1. Submodule repos build & upload .mcpb artifacts to GitHub Releases
    2. ${BOLD}--pull${NC} downloads those artifacts to <submodule>/dist/
    3. ${BOLD}--publish${NC} uploads them to tool.store registry

${BOLD}SKIP CONDITIONS${NC}
    Packages are skipped (not failed) when:
    • No .github/workflows/release.yml in submodule (--pull)
    • No GitHub releases exist for the repo (--pull)
    • No dist/ directory exists (--publish)
    • No .mcpb/.mcpbx bundles in dist/ (--publish)

${BOLD}REQUIREMENTS${NC}
    • gh (GitHub CLI) - authenticated
    • tool (tool-cli) - for publishing
    • jq - for JSON parsing"
    exit 0
}

log_info() { echo -e "${BLUE}ℹ${NC} $*"; }
log_success() { echo -e "${GREEN}✓${NC} $*"; }
log_warn() { echo -e "${YELLOW}⚠${NC} $*"; }
log_error() { echo -e "${RED}✗${NC} $*" >&2; }
log_skip() { echo -e "${DIM}○${NC} ${DIM}$*${NC}"; }
log_header() { echo -e "\n${BOLD}$1${NC}"; echo "─────────────────────────────────────────"; }

check_dependencies() {
    local missing=()
    command -v gh &>/dev/null || missing+=("gh (GitHub CLI)")
    command -v tool &>/dev/null || missing+=("tool (tool-cli)")
    command -v jq &>/dev/null || missing+=("jq")

    if [[ ${#missing[@]} -gt 0 ]]; then
        log_error "Missing dependencies: ${missing[*]}"
        exit 1
    fi
}

get_repo_from_remote() {
    local dir="$1"
    local remote_url

    remote_url=$(cd "$dir" && git remote get-url origin 2>/dev/null) || return 1

    # Extract owner/repo from git@github.com:owner/repo.git or https://github.com/owner/repo.git
    if [[ "$remote_url" =~ github\.com[:/]([^/]+)/([^/.]+)(\.git)?$ ]]; then
        echo "${BASH_REMATCH[1]}/${BASH_REMATCH[2]}"
    else
        return 1
    fi
}

get_latest_release() {
    local repo="$1"
    gh release view -R "$repo" --json tagName -q '.tagName' 2>/dev/null || echo ""
}

get_current_version() {
    local dir="$1"
    local manifest="$dir/dist/manifest.json"
    if [[ -f "$manifest" ]]; then
        jq -r '.version // empty' "$manifest" 2>/dev/null || echo ""
    else
        echo ""
    fi
}

check_has_release_workflow() {
    local dir="$1"
    [[ -f "$dir/.github/workflows/release.yml" ]]
}

pull_artifacts() {
    local dir="$1"
    local version="$2"
    local dry_run="$3"
    local name repo artifacts_dir latest_release current_version

    name=$(basename "$dir")
    repo=$(get_repo_from_remote "$dir") || { log_error "[$name] Failed to get git remote"; return 1; }
    artifacts_dir="$dir/dist"

    # Get version info
    if [[ -n "$version" ]]; then
        latest_release="$version"
    else
        latest_release=$(get_latest_release "$repo")
        if [[ -z "$latest_release" ]]; then
            log_skip "[$name] No releases found → skipping"
            return 2  # Skip code
        fi
    fi

    current_version=$(get_current_version "$dir")

    # Determine if update is needed
    local version_status=""
    if [[ -z "$current_version" ]]; then
        version_status="${GREEN}new${NC}"
    elif [[ "$current_version" == "$latest_release" || "v$current_version" == "$latest_release" ]]; then
        version_status="${DIM}up-to-date${NC}"
    else
        version_status="${CYAN}$current_version → $latest_release${NC}"
    fi

    echo -e "  ${BOLD}$name${NC} ${DIM}($repo)${NC}"
    echo -e "    Version: $version_status"

    if [[ "$dry_run" == "true" ]]; then
        echo -e "    ${DIM}[dry-run] Would download $latest_release to $artifacts_dir${NC}"
        return 0
    fi

    # Create artifacts directory
    mkdir -p "$artifacts_dir"

    # Download release assets
    local gh_args=(-R "$repo" -D "$artifacts_dir" -p '*.mcpb' -p '*.mcpbx' -p '*.sha256' --clobber)
    [[ -n "$version" ]] && gh_args+=("$version") || gh_args+=("$latest_release")

    if ! gh release download "${gh_args[@]}" 2>/dev/null; then
        log_error "    Failed to download artifacts"
        return 1
    fi

    # Count downloaded files
    local bundle_count
    bundle_count=$(find "$artifacts_dir" -maxdepth 1 \( -name "*.mcpb" -o -name "*.mcpbx" \) 2>/dev/null | wc -l | tr -d ' ')
    echo -e "    ${GREEN}✓${NC} Downloaded $bundle_count bundle(s)"
}

publish_artifacts() {
    local dir="$1"
    local dry_run="$2"
    local name artifacts_dir

    name=$(basename "$dir")
    artifacts_dir="$dir/dist"

    if [[ ! -d "$artifacts_dir" ]]; then
        log_skip "[$name] No dist directory → skipping (run --pull first)"
        return 2  # Skip code
    fi

    # Find bundles for each platform
    local darwin_arm64="" darwin_x64="" linux_arm64="" linux_x64="" win32_arm64="" win32_x64=""
    local platform_list=()

    for f in "$artifacts_dir"/*.mcpb "$artifacts_dir"/*.mcpbx; do
        [[ -f "$f" ]] || continue
        case "$(basename "$f")" in
            *-darwin-arm64.*) darwin_arm64="$f"; platform_list+=("darwin-arm64") ;;
            *-darwin-x86_64.*|*-darwin-x64.*) darwin_x64="$f"; platform_list+=("darwin-x64") ;;
            *-linux-arm64.*) linux_arm64="$f"; platform_list+=("linux-arm64") ;;
            *-linux-x86_64.*|*-linux-x64.*) linux_x64="$f"; platform_list+=("linux-x64") ;;
            *-win32-arm64.*) win32_arm64="$f"; platform_list+=("win32-arm64") ;;
            *-win32-x86_64.*|*-win32-x64.*) win32_x64="$f"; platform_list+=("win32-x64") ;;
        esac
    done

    # Check we found at least one bundle
    if [[ ${#platform_list[@]} -eq 0 ]]; then
        log_skip "[$name] No bundles in dist → skipping"
        return 2  # Skip code
    fi

    # Get version from manifest
    local version
    version=$(get_current_version "$dir")

    echo -e "  ${BOLD}$name${NC} ${DIM}v$version${NC}"
    echo -e "    Platforms: ${platform_list[*]}"

    # Build tool publish command
    local publish_args=(publish --multi-platform)
    [[ -n "$darwin_arm64" ]] && publish_args+=(--darwin-arm64 "$darwin_arm64")
    [[ -n "$darwin_x64" ]] && publish_args+=(--darwin-x64 "$darwin_x64")
    [[ -n "$linux_arm64" ]] && publish_args+=(--linux-arm64 "$linux_arm64")
    [[ -n "$linux_x64" ]] && publish_args+=(--linux-x64 "$linux_x64")
    [[ -n "$win32_arm64" ]] && publish_args+=(--win32-arm64 "$win32_arm64")
    [[ -n "$win32_x64" ]] && publish_args+=(--win32-x64 "$win32_x64")

    if [[ "$dry_run" == "true" ]]; then
        echo -e "    ${DIM}[dry-run] Would run: tool ${publish_args[*]}${NC}"
        return 0
    fi

    if ! (cd "$dir" && tool "${publish_args[@]}" 2>&1 | sed 's/^/    /'); then
        log_error "    Failed to publish"
        return 1
    fi

    echo -e "    ${GREEN}✓${NC} Published successfully"
}

process_path() {
    local dir="$1"
    local do_pull="$2"
    local do_publish="$3"
    local version="$4"
    local dry_run="$5"
    local name result

    name=$(basename "$dir")

    # Check for release workflow if pulling
    if [[ "$do_pull" == "true" ]]; then
        if ! check_has_release_workflow "$dir"; then
            log_skip "[$name] No release workflow → skipping"
            return 2  # Skip code
        fi
    fi

    # Execute operations
    if [[ "$do_pull" == "true" ]]; then
        pull_artifacts "$dir" "$version" "$dry_run"
        result=$?
        [[ $result -ne 0 ]] && return $result
    fi

    if [[ "$do_publish" == "true" ]]; then
        publish_artifacts "$dir" "$dry_run"
        result=$?
        [[ $result -ne 0 ]] && return $result
    fi

    return 0
}

#--------------------------------------------------------------------------------------------------
# Main
#--------------------------------------------------------------------------------------------------

main() {
    local paths=()
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
                log_error "Unknown option: $1"
                exit 1
                ;;
            *)
                paths+=("$1")
                shift
                ;;
        esac
    done

    # Validate arguments
    if [[ ${#paths[@]} -eq 0 ]]; then
        log_error "Missing path(s). Usage: $0 <paths...> --pull|--publish"
        exit 1
    fi
    if [[ "$do_pull" == "false" && "$do_publish" == "false" ]]; then
        log_error "Must specify --pull and/or --publish"
        exit 1
    fi

    check_dependencies

    # Expand and resolve paths
    local resolved_paths=()
    for pattern in "${paths[@]}"; do
        # Handle both absolute and relative paths
        if [[ "$pattern" = /* ]]; then
            # Absolute path - expand glob
            for expanded in $pattern; do
                [[ -d "$expanded" ]] && resolved_paths+=("$expanded")
            done
        else
            # Relative path - expand from ROOT_DIR
            for expanded in $ROOT_DIR/$pattern; do
                [[ -d "$expanded" ]] && resolved_paths+=("$expanded")
            done
        fi
    done

    # Check we have at least one valid path
    if [[ ${#resolved_paths[@]} -eq 0 ]]; then
        log_error "No valid directories found matching: ${paths[*]}"
        exit 1
    fi

    # Header
    local operation=""
    [[ "$do_pull" == "true" ]] && operation="Pulling"
    [[ "$do_publish" == "true" ]] && operation="${operation:+$operation & }Publishing"
    [[ "$dry_run" == "true" ]] && operation="$operation (dry-run)"

    echo -e "${BOLD}$operation ${#resolved_paths[@]} package(s)${NC}"
    echo ""

    # Process each path, collecting results
    local failed=()
    local succeeded=()
    local skipped=()

    for dir in "${resolved_paths[@]}"; do
        local result=0
        process_path "$dir" "$do_pull" "$do_publish" "$version" "$dry_run" || result=$?
        local name
        name=$(basename "$dir")

        case $result in
            0) succeeded+=("$name") ;;
            2) skipped+=("$name") ;;
            *) failed+=("$name") ;;
        esac
    done

    # Summary
    echo ""
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

    local summary_parts=()
    [[ ${#succeeded[@]} -gt 0 ]] && summary_parts+=("${GREEN}${#succeeded[@]} succeeded${NC}")
    [[ ${#skipped[@]} -gt 0 ]] && summary_parts+=("${YELLOW}${#skipped[@]} skipped${NC}")
    [[ ${#failed[@]} -gt 0 ]] && summary_parts+=("${RED}${#failed[@]} failed${NC}")

    # Join summary parts
    local summary=""
    for i in "${!summary_parts[@]}"; do
        [[ $i -gt 0 ]] && summary+=", "
        summary+="${summary_parts[$i]}"
    done
    echo -e "${BOLD}Summary:${NC} $summary"
    if [[ ${#succeeded[@]} -gt 0 ]]; then
        echo -e "  ${GREEN}✓${NC} ${succeeded[*]}"
    fi
    if [[ ${#skipped[@]} -gt 0 ]]; then
        echo -e "  ${DIM}○ ${skipped[*]}${NC}"
    fi
    if [[ ${#failed[@]} -gt 0 ]]; then
        echo -e "  ${RED}✗${NC} ${failed[*]}"
        exit 1
    fi
}

main "$@"
