name := "blockwork"
appid := "dev.ethanstokes.Blockwork"

# Variables
TARGET := "target/release/blockwork"
CEF_DIR := "target/release"
LIBDIR := "/usr/lib/blockwork"

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
    sudo install -Dm0755 {{TARGET}} {{LIBDIR}}/blockwork
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
    sudo ln -sf {{LIBDIR}}/blockwork /usr/bin/blockwork
    sudo install -Dm0644 res/blockwork.desktop /usr/share/applications/blockwork.desktop
    sudo install -Dm0644 res/icons/blockwork.png /usr/share/icons/hicolor/256x256/apps/blockwork.png

# Uninstall the project
uninstall:
    sudo rm -rf {{LIBDIR}}
    sudo rm -f /usr/bin/blockwork /usr/share/applications/blockwork.desktop /usr/share/icons/hicolor/256x256/apps/blockwork.png

replace: build uninstall install

# Regenerate packaging/flatpak/{cargo,node}-sources.json from the current
# lockfiles (needs org.flatpak.Builder installed: flatpak install flathub
# org.flatpak.Builder). Re-run whenever Cargo.lock or ui/pnpm-lock.yaml changes.
flatpak-sources:
    flatpak run --command=flatpak-cargo-generator org.flatpak.Builder -o packaging/flatpak/cargo-sources.json Cargo.lock
    flatpak run --command=flatpak-node-generator org.flatpak.Builder pnpm ui/pnpm-lock.yaml --pnpm-store-version v11 -o packaging/flatpak/node-sources.json

# Build the Flatpak entirely from source inside the sandbox (Rust + frontend),
# into ./flatpak-build, exporting to a local ./flatpak-repo (both gitignored).
# Building in-sandbox -- rather than reusing a host `cargo build` -- keeps the
# binary linked against the runtime's own glibc instead of the host's, which
# matters on rolling-release distros with a newer glibc than the runtime ships.
# If flatpak-builder errors with "Failed to spawn rofiles-fuse", add
# --disable-rofiles-fuse (needed in some sandboxed/containerized dev environments
# where FUSE isn't available).
flatpak-build *args:
    flatpak-builder --force-clean --user --repo=flatpak-repo flatpak-build packaging/flatpak/{{appid}}.yml {{args}}

# Install the just-built Flatpak from the local repo, adding it as a remote
# first if needed. Re-run after every flatpak-build to pick up changes.
flatpak-install:
    flatpak remote-add --user --if-not-exists --no-gpg-verify blockwork-local flatpak-repo
    flatpak install --user -y --reinstall blockwork-local {{appid}}

# Build, install, and launch in one go -- the normal "does it still work" loop.
flatpak-test *args: (flatpak-build args) flatpak-install
    flatpak run {{appid}}

# Remove the local test install (leaves flatpak-repo/flatpak-build in place).
flatpak-uninstall:
    flatpak uninstall --user -y {{appid}}
