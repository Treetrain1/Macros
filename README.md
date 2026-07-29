# Macros

A Tauri app to visually create and run macros on Windows, Linux, and macOS.

## Building

### Build
```bash
git clone https://github.com/EthanRStokes/macros.git macros && cd macros
just
```

### Installation

AUR: `macros-git`

```bash
just install
```

### Windows installer

Release builds of the Windows installer are published automatically on each
GitHub release. To build one locally:

1. Install [Inno Setup 6](https://jrsoftware.org/isdl.php) (one-time).
2. Run:
   ```powershell
   pwsh -File scripts/build-installer.ps1
   ```
   This produces `dist\macros-windows-x86_64-setup.exe`.