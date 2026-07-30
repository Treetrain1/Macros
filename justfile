name := "macros"
appid := "dev.ethanstokes.Macros"

# Variables
TARGET := "target/release/macros"
CEF_DIR := "target/release"
LIBDIR := "/usr/lib/macros"

# Default target
default: build

# Build the project
build *args:
    cargo build --release {{args}}

# Run the project
run: build
    {{TARGET}}

# Clean the project
clean:
    cargo clean

# Install the project
install:
    # Binary's RUNPATH is `$ORIGIN`, so the CEF runtime payload (libcef.so,
    # GL/Vulkan shims, *.pak, icudtl.dat, locales/, ...) has to live alongside
    # it in a private libdir, not /usr/bin.
    sudo install -Dm0755 {{TARGET}} {{LIBDIR}}/macros
    sudo install -Dm0755 {{CEF_DIR}}/libcef.so {{LIBDIR}}/libcef.so
    sudo install -Dm0755 {{CEF_DIR}}/libEGL.so {{LIBDIR}}/libEGL.so
    sudo install -Dm0755 {{CEF_DIR}}/libGLESv2.so {{LIBDIR}}/libGLESv2.so
    sudo install -Dm0755 {{CEF_DIR}}/libvk_swiftshader.so {{LIBDIR}}/libvk_swiftshader.so
    sudo install -Dm0755 {{CEF_DIR}}/libvulkan.so.1 {{LIBDIR}}/libvulkan.so.1
    sudo install -Dm0755 {{CEF_DIR}}/chrome-sandbox {{LIBDIR}}/chrome-sandbox
    sudo install -Dm0644 {{CEF_DIR}}/vk_swiftshader_icd.json {{LIBDIR}}/vk_swiftshader_icd.json
    sudo install -Dm0644 {{CEF_DIR}}/icudtl.dat {{LIBDIR}}/icudtl.dat
    sudo install -Dm0644 {{CEF_DIR}}/v8_context_snapshot.bin {{LIBDIR}}/v8_context_snapshot.bin
    sudo install -Dm0644 {{CEF_DIR}}/chrome_100_percent.pak {{LIBDIR}}/chrome_100_percent.pak
    sudo install -Dm0644 {{CEF_DIR}}/chrome_200_percent.pak {{LIBDIR}}/chrome_200_percent.pak
    sudo install -Dm0644 {{CEF_DIR}}/resources.pak {{LIBDIR}}/resources.pak
    sudo rm -rf {{LIBDIR}}/locales
    sudo cp -r {{CEF_DIR}}/locales {{LIBDIR}}/locales
    sudo ln -sf {{LIBDIR}}/macros /usr/bin/macros
    sudo install -Dm0644 res/macros.desktop /usr/share/applications/macros.desktop
    sudo install -Dm0644 res/icons/macros.png /usr/share/icons/hicolor/256x256/apps/macros.png

# Uninstall the project
uninstall:
    sudo rm -rf {{LIBDIR}}
    sudo rm -f /usr/bin/macros /usr/share/applications/macros.desktop /usr/share/icons/hicolor/256x256/apps/macros.png

replace: build uninstall install

# Regenerate the vendored dependency manifests the Flatpak build needs to build
# fully offline (Flathub disallows network access during the actual build step).
# Re-run whenever Cargo.lock or ui/pnpm-lock.yaml changes. Requires Python 3 and
# https://github.com/flatpak/flatpak-builder-tools checked out somewhere; check
# that repo's READMEs for current generator flags before running.
flatpak-sources fbt_path:
    python3 {{fbt_path}}/cargo/flatpak-cargo-generator.py Cargo.lock -o packaging/flatpak/cargo-sources.json
    python3 {{fbt_path}}/node/flatpak-node-generator.py pnpm ui/pnpm-lock.yaml -o packaging/flatpak/node-sources.json