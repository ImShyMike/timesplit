#!/bin/bash

set -euo pipefail

PROGRAM_NAME="timesplit"
LAUNCH_AGENT_LABEL="com.timesplit.timesplit"
DEFAULT_INSTALL_DIR="/usr/local/bin"
HOMEBREW_INSTALL_DIR="/opt/homebrew/bin"
GH_API="https://api.github.com/repos/ImShyMike/timesplit/releases/latest"

# Pick an install dir that is likely writable for the user
if [ -d "$HOMEBREW_INSTALL_DIR" ] && [ -w "$HOMEBREW_INSTALL_DIR" ]; then
    INSTALL_DIR="$HOMEBREW_INSTALL_DIR"
else
    INSTALL_DIR="$DEFAULT_INSTALL_DIR"
fi

PLIST_PATH="${HOME}/Library/LaunchAgents/${LAUNCH_AGENT_LABEL}.plist"

# Determine machine architecture
MACHINE=$(uname -m)
case "$MACHINE" in
    aarch64|arm64)
        ARCH="arm64"
        TARGET_TRIPLE="aarch64-apple-darwin"
        ;;
    *)
        ARCH="$MACHINE"
        TARGET_TRIPLE=""
        ;;
esac

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

print_success() {
    echo -e "${GREEN}[OK]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERR]${NC} $1"
}

print_info() {
    echo -e "${YELLOW}[INFO]${NC} $1"
}

get_download_url() {
    if [ -z "${TARGET_TRIPLE:-}" ]; then
        return 1
    fi

    if ! RELEASE_JSON=$(curl -sL "${GH_API}"); then
        return 1
    fi

    TAG=$(printf '%s' "$RELEASE_JSON" | grep -m1 '"tag_name"' | sed -E 's/.*"([^"]+)".*/\1/')
    if [ -z "$TAG" ]; then
        return 1
    fi

    VERSION=${TAG#v}

    EXPECTED_NAME="${PROGRAM_NAME}-${VERSION}-${TARGET_TRIPLE}"
    EXPECTED_ASSET="$EXPECTED_NAME"
    DOWNLOAD_URL=$(printf '%s' "$RELEASE_JSON" \
        | grep -oE '"browser_download_url"[[:space:]]*:[[:space:]]*"[^"]+' \
        | sed -E 's/"browser_download_url"[[:space:]]*:[[:space:]]*"//' \
        | grep -F "$EXPECTED_NAME" \
        | head -n1 || true)

    if [ -n "$DOWNLOAD_URL" ]; then
        printf '%s' "$DOWNLOAD_URL"
        return 0
    fi

    return 1
}

ensure_install_dir() {
    if [ ! -d "$INSTALL_DIR" ]; then
        if ! mkdir -p "$INSTALL_DIR" 2>/dev/null; then
            print_error "Install directory '$INSTALL_DIR' does not exist and could not be created. Try rerunning with sudo or adjust permissions."
            exit 1
        fi
    fi

    if [ ! -w "$INSTALL_DIR" ]; then
        print_error "Install directory '$INSTALL_DIR' is not writable. Try rerunning with sudo or choose a writable location."
        exit 1
    fi
}

create_plist() {
    mkdir -p "$(dirname "$PLIST_PATH")"

    cat > "$PLIST_PATH" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>${LAUNCH_AGENT_LABEL}</string>
    <key>ProgramArguments</key>
    <array>
        <string>${INSTALL_DIR}/${PROGRAM_NAME}</string>
        <string>run</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>EnvironmentVariables</key>
    <dict>
        <key>HOME</key>
        <string>${HOME}</string>
        <key>PATH</key>
        <string>${INSTALL_DIR}:/usr/local/bin:/opt/homebrew/bin:/usr/bin:/bin</string>
    </dict>
    <key>StandardOutPath</key>
    <string>${HOME}/Library/Logs/${PROGRAM_NAME}.log</string>
    <key>StandardErrorPath</key>
    <string>${HOME}/Library/Logs/${PROGRAM_NAME}.err.log</string>
    <key>WorkingDirectory</key>
    <string>${HOME}</string>
    <key>ProcessType</key>
    <string>Background</string>
</dict>
</plist>
EOF
}

load_service() {
    launchctl bootout "gui/$UID" "$PLIST_PATH" 2>/dev/null || true
    launchctl bootstrap "gui/$UID" "$PLIST_PATH"
    launchctl enable "gui/$UID/${LAUNCH_AGENT_LABEL}"
    launchctl kickstart -k "gui/$UID/${LAUNCH_AGENT_LABEL}"
}

unload_service() {
    launchctl bootout "gui/$UID" "$PLIST_PATH" 2>/dev/null || true
}

install_program() {
    if [ -z "${TARGET_TRIPLE:-}" ]; then
        print_error "Unsupported architecture '${ARCH}'. Available macOS binaries: arm64."
        exit 1
    fi

    ensure_install_dir

    print_info "Querying GitHub for latest release..."
    if DYN_URL=$(get_download_url); then
        DOWNLOAD_URL="$DYN_URL"
        print_info "Found release asset for ${TARGET_TRIPLE}: ${DOWNLOAD_URL}"
    else
        print_error "Could not determine latest release asset from GitHub; expected asset '${EXPECTED_ASSET:-${PROGRAM_NAME}-${TARGET_TRIPLE:-unknown}}'."
        exit 1
    fi

    print_info "Installing ${PROGRAM_NAME} ${VERSION:-latest} for ${TARGET_TRIPLE}..."

    TMP_FILE=$(mktemp /tmp/${PROGRAM_NAME}.XXXXXX)
    print_info "Downloading ${PROGRAM_NAME} from ${DOWNLOAD_URL}..."
    if ! curl -s -L -o "$TMP_FILE" "$DOWNLOAD_URL"; then
        print_error "Failed to download ${PROGRAM_NAME}"
        rm -f "$TMP_FILE"
        exit 1
    fi

    chmod +x "$TMP_FILE"
    mv "$TMP_FILE" "${INSTALL_DIR}/${PROGRAM_NAME}"
    print_success "Binary installed to ${INSTALL_DIR}/${PROGRAM_NAME}"

    print_info "Creating launchd agent..."
    create_plist
    load_service

    print_success "${PROGRAM_NAME} installed and started successfully!"
    print_info "Service status:"
    launchctl print "gui/$UID/${LAUNCH_AGENT_LABEL}" 2>/dev/null || print_error "Unable to read service status"
}

uninstall_program() {
    print_info "Uninstalling ${PROGRAM_NAME}..."

    unload_service

    if [ -f "$PLIST_PATH" ]; then
        rm "$PLIST_PATH"
        print_success "LaunchAgent plist removed"
    fi

    if [ -f "${INSTALL_DIR}/${PROGRAM_NAME}" ]; then
        rm "${INSTALL_DIR}/${PROGRAM_NAME}"
        print_success "Binary removed"
    fi

    print_success "${PROGRAM_NAME} uninstalled successfully!"
}

show_status() {
    if [ -f "${INSTALL_DIR}/${PROGRAM_NAME}" ]; then
        print_success "${PROGRAM_NAME} is installed at ${INSTALL_DIR}/${PROGRAM_NAME}"
    else
        print_error "${PROGRAM_NAME} binary not found at ${INSTALL_DIR}/${PROGRAM_NAME}"
    fi

    if launchctl print "gui/$UID/${LAUNCH_AGENT_LABEL}" >/dev/null 2>&1; then
        print_success "Service is loaded"
        launchctl print "gui/$UID/${LAUNCH_AGENT_LABEL}" 2>/dev/null
    else
        print_error "Service is not loaded"
    fi
}

show_usage() {
    cat <<EOF
Usage: $0 [COMMAND]

Commands:
    install     Install ${PROGRAM_NAME} and set up autorun
    uninstall   Remove ${PROGRAM_NAME} and stop autorun
    update      Update ${PROGRAM_NAME} to the latest version
    status      Check installation and service status
    help        Show this help message
EOF
}

case "${1:-}" in
    install)
        install_program
        ;;
    uninstall)
        uninstall_program
        ;;
    update)
        uninstall_program
        install_program
        ;;
    status)
        show_status
        ;;
    help|--help|-h)
        show_usage
        ;;
    *)
        print_error "Invalid command: ${1:-}"
        echo ""
        show_usage
        exit 1
        ;;
esac
