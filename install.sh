#!/bin/bash

set -e

PROGRAM_NAME="timesplit"
INSTALL_DIR="/usr/local/bin"
SERVICE_FILE="/etc/systemd/system/${PROGRAM_NAME}.service"
GH_API="https://api.github.com/repos/ImShyMike/timesplit/releases/latest"

# Determine machine architecture
MACHINE=$(uname -m)
case "$MACHINE" in
    aarch64|arm64)
        ARCH="aarch64"
        ;;
    x86_64|amd64)
        ARCH="x86_64"
        ;;
    *)
        ARCH="$MACHINE"
        ;;
esac

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

print_success() {
    echo -e "${GREEN}✓${NC} $1"
}

print_error() {
    echo -e "${RED}✗${NC} $1"
}

print_info() {
    echo -e "${YELLOW}→${NC} $1"
}

get_download_url() {
    # Fetch latest release JSON
    if ! RELEASE_JSON=$(curl -sL "${GH_API}"); then
        return 1
    fi

    # Extract tag_name (e.g. v0.3.0)
    TAG=$(printf '%s' "$RELEASE_JSON" | grep -m1 '"tag_name"' | sed -E 's/.*"([^"]+)".*/\1/')
    # If that doesn't work, fail
    if [ -z "$TAG" ]; then
        return 1
    fi

    VERSION=${TAG#v}

    # Look for an asset whose browser_download_url contains the expected filename
    EXPECTED_NAME="timesplit-${VERSION}-${ARCH}-unknown-linux-musl"
    DOWNLOAD_URL=$(printf '%s' "$RELEASE_JSON" \
        | grep -oE '"browser_download_url"[[:space:]]*:[[:space:]]*"[^"]+' \
        | sed -E 's/"browser_download_url"[[:space:]]*:[[:space:]]*"//' \
        | grep -F "$EXPECTED_NAME" || true)

    # If not found for this arch, try x86_64 as a fallback
    if [ -z "$DOWNLOAD_URL" ] && [ "$ARCH" != "x86_64" ]; then
        print_info "No asset for ${ARCH} found; falling back to x86_64 asset if available"
        EXPECTED_NAME="timesplit-${VERSION}-x86_64-unknown-linux-musl"
        DOWNLOAD_URL=$(printf '%s' "$RELEASE_JSON" \
            | grep -oE '"browser_download_url"[[:space:]]*:[[:space:]]*"[^"]+' \
            | sed -E 's/"browser_download_url"[[:space:]]*:[[:space:]]*"//' \
            | grep -F "$EXPECTED_NAME" || true)
    fi

    # If a URL was found, print it to stdout and succeed
    if [ -n "$DOWNLOAD_URL" ]; then
        printf '%s' "$DOWNLOAD_URL"
        return 0
    fi

    return 1
}

check_root() {
    if [ "$EUID" -ne 0 ]; then
        print_error "This script must be run as root (use sudo)"
        exit 1
    fi
}

install_program() {
    # Try to get a dynamic DOWNLOAD_URL
    print_info "Querying GitHub for latest release..."
    if DYN_URL=$(get_download_url); then
        DOWNLOAD_URL="$DYN_URL"
        print_info "Found release asset: ${DOWNLOAD_URL}"
    else
        print_error "Could not determine latest release asset from GitHub; exiting."
        exit 1
    fi

    print_info "Installing ${PROGRAM_NAME}..."
    
    # Download the binary
    print_info "Downloading ${PROGRAM_NAME} from ${DOWNLOAD_URL}..."
    if ! curl -s -L -o "/tmp/${PROGRAM_NAME}" "${DOWNLOAD_URL}"; then
        print_error "Failed to download ${PROGRAM_NAME}"
        exit 1
    fi
    
    # Make it executable and move to install directory
    chmod +x "/tmp/${PROGRAM_NAME}"
    mv "/tmp/${PROGRAM_NAME}" "${INSTALL_DIR}/${PROGRAM_NAME}"
    print_success "Binary installed to ${INSTALL_DIR}/${PROGRAM_NAME}"
    
    # Create systemd service
    print_info "Creating systemd service..."
    cat > "${SERVICE_FILE}" <<EOF
[Unit]
Description=TimeSplit Background Service
After=network.target

[Service]
Type=simple
ExecStart=${INSTALL_DIR}/${PROGRAM_NAME} run
Restart=always
RestartSec=10
User=${SUDO_USER:-$USER}
Environment="HOME=/home/${SUDO_USER:-$USER}"

[Install]
WantedBy=multi-user.target
EOF
    
    print_success "Systemd service created"
    
    # Reload systemd, enable and start the service
    print_info "Enabling and starting service..."
    systemctl daemon-reload
    systemctl enable "${PROGRAM_NAME}.service"
    systemctl start "${PROGRAM_NAME}.service"
    
    print_success "${PROGRAM_NAME} installed and started successfully!"
    print_info "Service status:"
    systemctl status "${PROGRAM_NAME}.service" --no-pager
}

uninstall_program() {
    print_info "Uninstalling ${PROGRAM_NAME}..."
    
    # Stop and disable service
    if systemctl is-active --quiet "${PROGRAM_NAME}.service"; then
        print_info "Stopping service..."
        systemctl stop "${PROGRAM_NAME}.service"
    fi
    
    if systemctl is-enabled --quiet "${PROGRAM_NAME}.service" 2>/dev/null; then
        print_info "Disabling service..."
        systemctl disable "${PROGRAM_NAME}.service"
    fi
    
    # Remove service file
    if [ -f "${SERVICE_FILE}" ]; then
        rm "${SERVICE_FILE}"
        print_success "Service file removed"
    fi
    
    # Remove binary
    if [ -f "${INSTALL_DIR}/${PROGRAM_NAME}" ]; then
        rm "${INSTALL_DIR}/${PROGRAM_NAME}"
        print_success "Binary removed"
    fi
    
    # Reload systemd
    systemctl daemon-reload
    
    print_success "${PROGRAM_NAME} uninstalled successfully!"
}

show_status() {
    if [ -f "${INSTALL_DIR}/${PROGRAM_NAME}" ]; then
        print_success "${PROGRAM_NAME} is installed at ${INSTALL_DIR}/${PROGRAM_NAME}"
        
        if systemctl is-active --quiet "${PROGRAM_NAME}.service"; then
            print_success "Service is running"
            systemctl status "${PROGRAM_NAME}.service" --no-pager
        else
            print_error "Service is not running"
        fi
    else
        print_info "${PROGRAM_NAME} is not installed"
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

Examples:
    sudo $0 install
    sudo $0 uninstall
    sudo $0 update
    $0 status
    $0 help
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