#!/usr/bin/env bash
# Builds a universal (arm64 + x86_64) macros_ffi static library for macOS,
# matching macros-gd/CMakeLists.txt's `CMAKE_OSX_ARCHITECTURES "arm64;x86_64"`.
#
# Must run on an actual Mac (or macOS CI runner) — cross-compiling for
# Apple targets needs the Apple SDK sysroot, which can't be redistributed
# and isn't available on Linux dev boxes.
#
# Usage: macros-ffi/build-macos.sh
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")"

for target in x86_64-apple-darwin aarch64-apple-darwin; do
    rustup target add "$target" >/dev/null
    cargo build -p macros-ffi --release --target "$target"
done

mkdir -p ../target/universal-macos
lipo -create -output ../target/universal-macos/libmacros_ffi.a \
    ../target/x86_64-apple-darwin/release/libmacros_ffi.a \
    ../target/aarch64-apple-darwin/release/libmacros_ffi.a

echo
echo "Built: macros-ffi/../target/universal-macos/libmacros_ffi.a"
echo "Header: macros-ffi/include/macros_ffi.h"
echo
echo "Pass to macros-gd's CMake configure, e.g.:"
echo "  -DMACROS_FFI_LIB=$(cd .. && pwd)/target/universal-macos/libmacros_ffi.a"
echo "  -DMACROS_FFI_INCLUDE_DIR=$(pwd)/include"
echo
echo "If the link step reports missing symbols, macros-core's macOS backend"
echo "(backend/macos.rs) may need another framework linked beyond"
echo "CoreFoundation/ApplicationServices already in CMakeLists.txt — Security"
echo "is the most likely candidate (core-foundation/core-graphics sometimes"
echo "pull it in transitively). Verify against the actual linker error rather"
echo "than adding it speculatively."
