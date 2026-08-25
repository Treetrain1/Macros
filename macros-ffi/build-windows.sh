#!/usr/bin/env bash
# Cross-builds macros-ffi as a Windows staticlib from Linux, reusing the same
# xwin splat (MSVC CRT + Windows SDK) macros-gd's own C++ build already uses
# (see macros-gd/build.sh) rather than pulling a second copy via cargo-xwin.
# Also builds macros-linux-bridge (a plain native build — no cross-compile
# involved, it's the host target) and copies it into macros-gd's resources/
# folder, so it ends up bundled in the .geode package. That's the process
# macros_init launches when it detects it's running under Wine on a Linux
# host (Proton) — see macros-ffi/src/wine_bridge.rs.
#
# Usage: SPLAT_DIR=/path/to/splat ./build-windows.sh [macros-gd-repo-path]
set -euo pipefail

: "${SPLAT_DIR:?Set SPLAT_DIR to the xwin splat directory (crt/ and sdk/ subfolders, see macros-gd/build.sh)}"

TARGET=x86_64-pc-windows-msvc
cd "$(dirname "${BASH_SOURCE[0]}")"

MACROS_GD_REPO="${1:-/home/treetrain1/Documents/git/macros-gd}"

rustup target add "$TARGET" >/dev/null

# Scoped per-command (not `export`) so these don't leak into the native
# macros-linux-bridge build below.
env \
  CC_x86_64_pc_windows_msvc=clang-cl \
  CXX_x86_64_pc_windows_msvc=clang-cl \
  AR_x86_64_pc_windows_msvc=llvm-lib \
  CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER=lld-link \
  RUSTFLAGS="-Lnative=${SPLAT_DIR}/crt/lib/x86_64 -Lnative=${SPLAT_DIR}/sdk/lib/um/x86_64 -Lnative=${SPLAT_DIR}/sdk/lib/ucrt/x86_64 ${RUSTFLAGS:-}" \
  cargo build -p macros-ffi --release --target "$TARGET"
  # Rust's *-pc-windows-msvc target links the dynamic UCRT by default (no
  # +crt-static above) — matches what GD's own binary and macros-gd's
  # existing clang-cl toolchain expect. Mixing static- and dynamic-CRT
  # objects in one binary is a classic MSVC pitfall, so don't add it.

cargo build -p macros-linux-bridge --release

if [ -d "$MACROS_GD_REPO" ]; then
    mkdir -p "$MACROS_GD_REPO/resources"
    cp ../target/release/macros-linux-bridge "$MACROS_GD_REPO/resources/linux-input.so"
    echo "Copied macros-linux-bridge to $MACROS_GD_REPO/resources/linux-input.so"
else
    echo "warning: $MACROS_GD_REPO not found, skipping resource copy (pass its path as \$1 if it's elsewhere)"
fi

LIB_PATH="../target/${TARGET}/release/macros_ffi.lib"
echo
echo "Built: macros-ffi/${LIB_PATH}"
echo "Header: macros-ffi/include/macros_ffi.h"
echo
echo "Native import libs this build needs beyond the CRT/SDK above (feed into macros-gd/CMakeLists.txt's target_link_libraries):"
env \
  CC_x86_64_pc_windows_msvc=clang-cl \
  CXX_x86_64_pc_windows_msvc=clang-cl \
  AR_x86_64_pc_windows_msvc=llvm-lib \
  CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER=lld-link \
  RUSTFLAGS="-Lnative=${SPLAT_DIR}/crt/lib/x86_64 -Lnative=${SPLAT_DIR}/sdk/lib/um/x86_64 -Lnative=${SPLAT_DIR}/sdk/lib/ucrt/x86_64 ${RUSTFLAGS:-}" \
  cargo rustc -p macros-ffi --release --target "$TARGET" -- --print native-static-libs 2>&1 | grep "native-static-libs:" || echo "(none printed — check build output above)"
