#!/bin/bash
# Weekly cleanup of developer build output and caches on this Mac.
#
#   disk_cleanup.sh run [--dry-run]   clean now (--dry-run lists, deletes nothing)
#   disk_cleanup.sh install           copy this script out of ~/Documents and
#                                     schedule it weekly with launchd
#   disk_cleanup.sh uninstall         remove the launchd job
#
# What a run deletes:
#   - Cargo target/ dirs under ROOTS with nothing written in TARGET_DAYS days
#   - node_modules under ROOTS whose project has no edits in NODE_DAYS days
#   - the npm cache, Homebrew's download cache and old versions,
#     Xcode DerivedData, and simulators for runtimes that are no longer installed
# Xcode Archives, iOS DeviceSupport and working simulators are left alone.
#
# Log: ~/Library/Logs/disk-cleanup.log

set -u

TARGET_DAYS=14
NODE_DAYS=30
ROOTS=("$HOME/Documents" "$HOME/Projects" "$HOME/GameMakerProjects")

LABEL="local.disk-cleanup"
INSTALL_DIR="$HOME/Library/Application Support/disk-cleanup"
PLIST="$HOME/Library/LaunchAgents/$LABEL.plist"
LOG="$HOME/Library/Logs/disk-cleanup.log"

# launchd starts jobs with a bare PATH.
export PATH="/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin"

DRY_RUN=0

log() { printf '%s  %s\n' "$(date '+%Y-%m-%d %H:%M:%S')" "$*"; }

size_of() { du -sh "$1" 2>/dev/null | cut -f1; }

remove() {
    local path="$1" why="$2"
    if [ "$DRY_RUN" = 1 ]; then
        log "would delete ($(size_of "$path"), $why): $path"
    else
        log "deleting ($(size_of "$path"), $why): $path"
        rm -rf "$path"
    fi
}

# Directories never worth descending into while looking for build output.
prune_expr=( \( -name .git -o -name .Trash -o -name '*.app' -o -name Library \) -prune )

clean_cargo_targets() {
    local root dir
    for root in "${ROOTS[@]}"; do
        [ -d "$root" ] || continue
        find "$root" "${prune_expr[@]}" -o -type d -name node_modules -prune \
            -o -type d -name target -print -prune 2>/dev/null |
        while IFS= read -r dir; do
            # Only Cargo's target dirs: cargo writes .rustc_info.json there.
            [ -f "$dir/.rustc_info.json" ] || [ -f "$(dirname "$dir")/Cargo.toml" ] || continue
            # A build writes into target/<profile>/deps etc., so recent
            # activity shows up within a few levels.
            if [ -z "$(find "$dir" -maxdepth 3 -mtime -"$TARGET_DAYS" -print -quit 2>/dev/null)" ]; then
                remove "$dir" "cargo target, idle ${TARGET_DAYS}+ days"
            fi
        done
    done
}

clean_node_modules() {
    local root dir project
    for root in "${ROOTS[@]}"; do
        [ -d "$root" ] || continue
        find "$root" "${prune_expr[@]}" -o -type d -name target -prune \
            -o -type d -name node_modules -print -prune 2>/dev/null |
        while IFS= read -r dir; do
            project="$(dirname "$dir")"
            if [ -z "$(find "$project" \( -name node_modules -o -name .git \) -prune \
                        -o -mtime -"$NODE_DAYS" -print -quit 2>/dev/null)" ]; then
                remove "$dir" "project idle ${NODE_DAYS}+ days"
            fi
        done
    done
}

clean_caches() {
    if [ "$DRY_RUN" = 1 ]; then
        [ -d "$HOME/.npm/_cacache" ] && log "would clear npm cache ($(size_of "$HOME/.npm/_cacache"))"
        command -v brew >/dev/null && log "would run brew cleanup ($(size_of "$(brew --cache)") cached)"
        [ -d "$HOME/Library/Developer/Xcode/DerivedData" ] &&
            log "would clear Xcode DerivedData ($(size_of "$HOME/Library/Developer/Xcode/DerivedData"))"
        command -v xcrun >/dev/null && log "would delete unavailable simulators"
        return
    fi

    if command -v npm >/dev/null; then
        log "npm cache clean"
        npm cache clean --force 2>&1 | sed 's/^/    /'
    fi
    if command -v brew >/dev/null; then
        log "brew cleanup"
        brew cleanup --prune=all -s 2>&1 | sed 's/^/    /'
    fi
    if [ -d "$HOME/Library/Developer/Xcode/DerivedData" ]; then
        log "clearing Xcode DerivedData ($(size_of "$HOME/Library/Developer/Xcode/DerivedData"))"
        find "$HOME/Library/Developer/Xcode/DerivedData" -mindepth 1 -maxdepth 1 -exec rm -rf {} +
    fi
    if xcrun --find simctl >/dev/null 2>&1; then
        log "deleting unavailable simulators"
        xcrun simctl delete unavailable 2>&1 | sed 's/^/    /'
    fi
}

check_access() {
    local root
    for root in "${ROOTS[@]}"; do
        [ -d "$root" ] || continue
        ls "$root" >/dev/null 2>&1 ||
            log "WARNING: can't read $root, skipping it. Give /bin/bash Full Disk Access in System Settings > Privacy & Security."
    done
}

free_space() { df -h "$HOME" | awk 'NR==2 {print $4}'; }

run() {
    [ "${1:-}" = "--dry-run" ] && DRY_RUN=1
    log "=== disk cleanup start$([ "$DRY_RUN" = 1 ] && echo ' (dry run)'), free: $(free_space)"
    check_access
    clean_cargo_targets
    clean_node_modules
    clean_caches
    log "=== disk cleanup done, free: $(free_space)"
}

install() {
    mkdir -p "$INSTALL_DIR" "$(dirname "$PLIST")" "$(dirname "$LOG")"
    # launchd can't read scripts inside ~/Documents (macOS privacy rules), so
    # run a copy. Re-run install after editing this file.
    cp "$0" "$INSTALL_DIR/disk_cleanup.sh"
    chmod +x "$INSTALL_DIR/disk_cleanup.sh"
    cat > "$PLIST" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>$LABEL</string>
    <key>ProgramArguments</key>
    <array>
        <string>/bin/bash</string>
        <string>$INSTALL_DIR/disk_cleanup.sh</string>
        <string>run</string>
    </array>
    <!-- Sundays at 12:00. A run missed while asleep happens on wake. -->
    <key>StartCalendarInterval</key>
    <dict>
        <key>Weekday</key>
        <integer>0</integer>
        <key>Hour</key>
        <integer>12</integer>
        <key>Minute</key>
        <integer>0</integer>
    </dict>
    <key>StandardOutPath</key>
    <string>$LOG</string>
    <key>StandardErrorPath</key>
    <string>$LOG</string>
    <key>Nice</key>
    <integer>10</integer>
    <key>LowPriorityIO</key>
    <true/>
</dict>
</plist>
EOF
    launchctl bootout "gui/$(id -u)/$LABEL" 2>/dev/null
    launchctl bootstrap "gui/$(id -u)" "$PLIST"
    echo "Installed $LABEL (weekly, Sundays 12:00). Log: $LOG"
    echo "Run it now with: launchctl kickstart gui/$(id -u)/$LABEL"
}

uninstall() {
    launchctl bootout "gui/$(id -u)/$LABEL" 2>/dev/null
    rm -f "$PLIST"
    rm -rf "$INSTALL_DIR"
    echo "Removed $LABEL"
}

case "${1:-run}" in
    run) shift 2>/dev/null; run "$@" ;;
    install) install ;;
    uninstall) uninstall ;;
    *) echo "usage: $0 [run [--dry-run] | install | uninstall]" >&2; exit 2 ;;
esac
