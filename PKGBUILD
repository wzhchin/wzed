# Maintainer: wangzh <wzhchin@gmail.com>
#
# Local-source PKGBUILD: builds wzed straight from this checkout (the directory
# holding this PKGBUILD), no remote download. Wayland-only — Cargo.toml disables
# gpui_linux's x11 backend, so the binary links no libxcb / libxkbcommon-x11.
# GPU (EGL/GL) and the Wayland client are dlopen'd at runtime.
#
# makepkg runs the functions from an empty $srcdir, so each one cd's back to
# $startdir (this directory) to build in place and reuse the existing target/.
#
# Usage, from the repo root:
#   makepkg -si

pkgname=wzed
pkgver=0.1.1
pkgrel=1
pkgdesc="Lightweight text editor based on Zed's GPUI editor (Wayland-only)"
arch=('x86_64' 'aarch64')
url="https://github.com/wzhchin/wzed"
license=('GPL-3.0-or-later')

# Runtime: libxkbcommon is linked directly; wayland/fontconfig/libglvnd are
# dlopen'd by GPUI/wgpu at startup. No X11 libraries are needed.
depends=(
    'libxkbcommon'
    'wayland'
    'fontconfig'
    'libglvnd'
)
makedepends=(
    'cargo'
    'git'
    'wayland-protocols'
)
checkdepends=('cargo')

# No source array: the package is built from this very checkout. $startdir is
# where makepkg found the PKGBUILD, i.e. the repo root.
#
# !lto: makepkg's default CFLAGS/LDFLAGS add -flto, which compiles the bundled
# libgit2-sys (a C crate) to GCC LTO bitcode. The Rust linker (rust-lld/LLVM)
# then can't resolve those symbols — `undefined symbol: git_libgit2_init`.
# Rust/LLVM and GCC LTO are incompatible; disable LTO for the whole build.
options=('!strip' '!lto')

pkgver() {
    cd "$startdir"
    # Derive from Cargo.toml so this never drifts from the actual package version.
    grep -m1 '^version' Cargo.toml | sed 's/.*"\(.*\)".*/\1/'
}

prepare() {
    cd "$startdir"
    # Honor the committed Cargo.lock — fail loudly if it's drifted from
    # Cargo.toml instead of silently rewriting it.
    cargo fetch --locked --target "$CARCH-unknown-linux-gnu"
}

build() {
    cd "$startdir"
    cargo build --release --frozen
}

check() {
    cd "$startdir"
    cargo test --frozen
}

package() {
    cd "$startdir"

    install -Dm755 "target/release/$pkgname" "$pkgdir/usr/bin/$pkgname"

    local app_id="dev.wzed.editor"
    install -Dm644 "dist/$app_id.desktop" \
        "$pkgdir/usr/share/applications/$app_id.desktop"

    local size
    for size in 16 22 32 48 64 128 256; do
        install -Dm644 "dist/icons/$size.png" \
            "$pkgdir/usr/share/icons/hicolor/${size}x${size}/apps/$app_id.png"
    done

    install -Dm644 LICENSE "$pkgdir/usr/share/licenses/$pkgname/LICENSE"
}
