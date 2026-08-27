#!/usr/bin/env bash
# Cross-builds blockwork-ffi as a Windows staticlib from Linux, reusing the same
# xwin splat (MSVC CRT + Windows SDK) blockwork-gd's own C++ build already uses
# (see blockwork-gd/build.sh) rather than pulling a second copy via cargo-xwin.
# Also builds blockwork-linux-bridge (a plain native build — no cross-compile
# involved, it's the host target) and copies it into blockwork-gd's resources/
# folder, so it ends up bundled in the .geode package. That's the process
# macros_init launches when it detects it's running under Wine on a Linux
# host (Proton) — see blockwork-ffi/src/wine_bridge.rs.
#
# Usage: SPLAT_DIR=/path/to/splat ./build-windows.sh [blockwork-gd-repo-path]
set -euo pipefail

: "${SPLAT_DIR:?Set SPLAT_DIR to the xwin splat directory (crt/ and sdk/ subfolders, see blockwork-gd/build.sh)}"

TARGET=x86_64-pc-windows-msvc
cd "$(dirname "${BASH_SOURCE[0]}")"

BLOCKWORK_GD_REPO="${1:-/home/treetrain1/Documents/git/blockwork-gd}"

rustup target add "$TARGET" >/dev/null

# Scoped per-command (not `export`) so these don't leak into the native
# blockwork-linux-bridge build below.
env \
  CC_x86_64_pc_windows_msvc=clang-cl \
  CXX_x86_64_pc_windows_msvc=clang-cl \
  AR_x86_64_pc_windows_msvc=llvm-lib \
  CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER=lld-link \
  RUSTFLAGS="-Lnative=${SPLAT_DIR}/crt/lib/x86_64 -Lnative=${SPLAT_DIR}/sdk/lib/um/x86_64 -Lnative=${SPLAT_DIR}/sdk/lib/ucrt/x86_64 ${RUSTFLAGS:-}" \
  cargo build -p blockwork-ffi --release --target "$TARGET"
  # Rust's *-pc-windows-msvc target links the dynamic UCRT by default (no
  # +crt-static above) — matches what GD's own binary and blockwork-gd's
  # existing clang-cl toolchain expect. Mixing static- and dynamic-CRT
  # objects in one binary is a classic MSVC pitfall, so don't add it.

cargo build -p blockwork-linux-bridge --release

if [ -d "$BLOCKWORK_GD_REPO" ]; then
    mkdir -p "$BLOCKWORK_GD_REPO/resources"
    cp ../target/release/blockwork-linux-bridge "$BLOCKWORK_GD_REPO/resources/linux-input.so"
    echo "Copied blockwork-linux-bridge to $BLOCKWORK_GD_REPO/resources/linux-input.so"
else
    echo "warning: $BLOCKWORK_GD_REPO not found, skipping resource copy (pass its path as \$1 if it's elsewhere)"
fi

LIB_PATH="../target/${TARGET}/release/blockwork_ffi.lib"
echo
echo "Built: blockwork-ffi/${LIB_PATH}"
echo "Header: blockwork-ffi/include/blockwork_ffi.h"
echo
echo "Native import libs this build needs beyond the CRT/SDK above (feed into blockwork-gd/CMakeLists.txt's target_link_libraries):"
env \
  CC_x86_64_pc_windows_msvc=clang-cl \
  CXX_x86_64_pc_windows_msvc=clang-cl \
  AR_x86_64_pc_windows_msvc=llvm-lib \
  CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER=lld-link \
  RUSTFLAGS="-Lnative=${SPLAT_DIR}/crt/lib/x86_64 -Lnative=${SPLAT_DIR}/sdk/lib/um/x86_64 -Lnative=${SPLAT_DIR}/sdk/lib/ucrt/x86_64 ${RUSTFLAGS:-}" \
  cargo rustc -p blockwork-ffi --release --target "$TARGET" -- --print native-static-libs 2>&1 | grep "native-static-libs:" || echo "(none printed — check build output above)"
