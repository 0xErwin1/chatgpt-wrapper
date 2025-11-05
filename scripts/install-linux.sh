#!/usr/bin/env bash
set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TARGET_BIN="${PROJECT_ROOT}/src-tauri/target/release/chatgpt-wrapper"
INSTALL_DIR="${HOME}/.local/opt/chatgpt-wrapper"
LAUNCHER="${INSTALL_DIR}/chatgpt-desktop"
DESKTOP_FILE="${HOME}/.local/share/applications/chatgpt-wrapper.desktop"
ICON_TARGET_DIR="${HOME}/.local/share/icons/hicolor/128x128/apps"
ICON_SOURCE="${PROJECT_ROOT}/src-tauri/icons/128x128.png"

printf '→ Building ChatGPT Desktop (release binary)...\n'
cargo build --release --manifest-path "${PROJECT_ROOT}/src-tauri/Cargo.toml"

mkdir -p "${INSTALL_DIR}"
cp "${TARGET_BIN}" "${INSTALL_DIR}/chatgpt-wrapper"
chmod +x "${INSTALL_DIR}/chatgpt-wrapper"

cat > "${LAUNCHER}" <<'LAUNCH'
#!/usr/bin/env bash
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
exec "${DIR}/chatgpt-wrapper" "$@"
LAUNCH
chmod +x "${LAUNCHER}"

mkdir -p "${ICON_TARGET_DIR}"
cp "${ICON_SOURCE}" "${ICON_TARGET_DIR}/chatgpt-wrapper.png"

cat > "${DESKTOP_FILE}" <<DESKTOP
[Desktop Entry]
Type=Application
Name=ChatGPT Desktop
Comment=Minimal desktop wrapper for chat.openai.com
Exec=${LAUNCHER}
Icon=chatgpt-wrapper
Terminal=false
Categories=Utility;Network;
StartupNotify=true
DESKTOP

update-desktop-database "${HOME}/.local/share/applications" >/dev/null 2>&1 || true

printf '\n✔ ChatGPT Desktop installed locally.\n'
printf '  • Binary: %s\n' "${INSTALL_DIR}/chatgpt-wrapper"
printf '  • Launcher: %s\n' "${LAUNCHER}"
printf '  • Desktop entry: %s\n' "${DESKTOP_FILE}"
printf '  • Icon: %s/chatgpt-wrapper.png\n' "${ICON_TARGET_DIR}"
