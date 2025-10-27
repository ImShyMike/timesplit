#!/bin/bash

set -euo pipefail

PROGRAM_NAME="timesplit"
INSTALL_DIR="/usr/local/bin"
SERVICE_LABEL="com.imshymike.timesplit"
PLIST_FILE="/Library/LaunchDaemons/${SERVICE_LABEL}.plist"
GH_API="https://api.github.com/repos/ImShyMike/timesplit/releases/latest"

MACHINE=$(uname -m)
case "$MACHINE" in
    arm64|aarch64)
        TARGET_TRIPLE="aarch64-apple-darwin"
        ;;
    *)
        TARGET_TRIPLE=""
        ;;
esac

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

print_success() { echo -e "${GREEN}✓${NC} $1"; }
print_error() { echo -e "${RED}✗${NC} $1"; }
print_info() { echo -e "${YELLOW}→${NC} $1"; }

check_root() {
    if [ "$EUID" -ne 0 ]; then
        print_error "This script must be run as root (use sudo)"
        exit 1
    fi
}

choose_install_dir() {
    if [ -d "/usr/local/bin" ]; then
        INSTALL_DIR="/usr/local/bin"
    else
        INSTALL_DIR="/opt/homebrew/bin"
    fi
}

get_download_url() {
    if [ -z "${TARGET_TRIPLE:-}" ]; then
        return 1
    fi

    print_info "Querying GitHub for latest release..."
    if ! RELEASE_JSON=$(curl -sL "${GH_API}"); then
        return 1
    fi

    TAG=$(printf '%s' "$RELEASE_JSON" | grep -m1 '"tag_name"' | sed -E 's/.*"([^\"]+)".*/\1/') || true
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

install_program() {
    check_root
    choose_install_dir

    if [ -z "${TARGET_TRIPLE:-}" ]; then
        print_error "Unsupported architecture '${MACHINE}'. macOS releases are currently available only for Apple Silicon (arm64)."
        exit 1
    fi

    if ! DOWNLOAD_URL=$(get_download_url); then
        print_error "Could not determine latest release asset from GitHub; expected asset '${EXPECTED_ASSET:-${PROGRAM_NAME}-${TARGET_TRIPLE:-unknown}}'."
        exit 1
    fi

    print_info "Found release asset for ${TARGET_TRIPLE}: ${DOWNLOAD_URL}"

    print_info "Installing ${PROGRAM_NAME} ${VERSION:-latest} for ${TARGET_TRIPLE}..."

    TMPDIR=$(mktemp -d /tmp/${PROGRAM_NAME}.XXXXXX)
    TMPFILE="${TMPDIR}/${PROGRAM_NAME}.download"
    print_info "Downloading ${PROGRAM_NAME}..."
    if ! curl -sL -o "$TMPFILE" "$DOWNLOAD_URL"; then
        print_error "Failed to download ${PROGRAM_NAME}"
        rm -rf "$TMPDIR"
        exit 1
    fi

    BIN="$TMPFILE"

    chmod +x "$BIN" || true
    mkdir -p "$INSTALL_DIR"
    mv -f "$BIN" "$INSTALL_DIR/${PROGRAM_NAME}"
    print_success "Binary installed to ${INSTALL_DIR}/${PROGRAM_NAME}"

    rm -rf "$TMPDIR"

    # Create LaunchDaemon plist
    print_info "Creating LaunchDaemon plist at ${PLIST_FILE}"
    cat > "$PLIST_FILE" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
  <dict>
    <key>Label</key>
    <string>${SERVICE_LABEL}</string>
    <key>ProgramArguments</key>
    <array>
      <string>${INSTALL_DIR}/${PROGRAM_NAME}</string>
      <string>run</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>StandardOutPath</key>
    <string>/var/log/${PROGRAM_NAME}.log</string>
    <key>StandardErrorPath</key>
    <string>/var/log/${PROGRAM_NAME}.err</string>
  </dict>
</plist>
EOF

    chown root:wheel "$PLIST_FILE" || true
    chmod 644 "$PLIST_FILE" || true

    # Load the LaunchDaemon
    if launchctl print system/${SERVICE_LABEL} >/dev/null 2>&1; then
        print_info "Service already loaded, unloading first"
        launchctl bootout system "$PLIST_FILE" >/dev/null 2>&1 || true
    fi

    if launchctl bootstrap system "$PLIST_FILE" >/dev/null 2>&1; then
        print_success "LaunchDaemon bootstrapped"
    else
        print_info "bootstrap failed or not supported, trying legacy load"
        launchctl load -w "$PLIST_FILE" || true
    fi

    print_success "${PROGRAM_NAME} installed and scheduled to run at startup. Logs: /var/log/${PROGRAM_NAME}.log"
}

uninstall_program() {
    check_root

    print_info "Removing LaunchDaemon (if present)"
    if launchctl print system/${SERVICE_LABEL} >/dev/null 2>&1; then
        launchctl bootout system "$PLIST_FILE" >/dev/null 2>&1 || true
    else
        launchctl unload "$PLIST_FILE" >/dev/null 2>&1 || true
    fi

    if [ -f "$PLIST_FILE" ]; then
        rm -f "$PLIST_FILE"
        print_success "Removed plist: $PLIST_FILE"
    fi

    choose_install_dir
    if [ -f "${INSTALL_DIR}/${PROGRAM_NAME}" ]; then
        rm -f "${INSTALL_DIR}/${PROGRAM_NAME}"
        print_success "Removed binary: ${INSTALL_DIR}/${PROGRAM_NAME}"
    else
        print_info "Binary not found at ${INSTALL_DIR}/${PROGRAM_NAME}"
    fi

    print_success "${PROGRAM_NAME} uninstalled"
}

show_status() {
    choose_install_dir
    if [ -f "${INSTALL_DIR}/${PROGRAM_NAME}" ]; then
        print_success "${PROGRAM_NAME} is installed at ${INSTALL_DIR}/${PROGRAM_NAME}"
    else
        print_error "${PROGRAM_NAME} is not installed"
    fi

    print_info "LaunchDaemon status:"
    if launchctl print system/${SERVICE_LABEL} >/dev/null 2>&1; then
        launchctl print system/${SERVICE_LABEL} | head -n 40
    else
        if [ -f "$PLIST_FILE" ]; then
            print_info "Plist present at $PLIST_FILE but not loaded"
        else
            print_info "No plist at $PLIST_FILE"
        fi
    fi
}

show_usage() {
    cat <<EOF
Usage: $0 [COMMAND]

Commands:
    install     Install ${PROGRAM_NAME} and set up autorun (requires sudo)
    uninstall   Remove ${PROGRAM_NAME} and stop autorun (requires sudo)
    update      Update ${PROGRAM_NAME} to the latest version (requires sudo)
    status      Check installation and service status
    help        Show this help message
EOF
}

case "${1:-}" in
    install)
        check_root
        install_program
        ;;
    uninstall)
        check_root
        uninstall_program
        ;;
    update)
        check_root
        uninstall_program || true
        install_program
        ;;
    status)
        show_status
        ;;
    help|--help|-h|"")
        show_usage
        ;;
    *)
        print_error "Invalid command: ${1:-}"
        show_usage
        exit 1
        ;;
esac
