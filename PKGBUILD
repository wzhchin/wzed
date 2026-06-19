# Maintainer: wangzh <wzhchin@gmail.com>
#
# Packages the tagged release (v$pkgver) from GitHub. The build is Wayland-only:
# upstream's Cargo.toml disables gpui_linux's x11 backend, so the binary links no
# libxcb / libxkbcommon-x11. GPU (EGL/GL) and the Wayland client are dlopen'd at
# runtime like the upstream editor.
#
# Prerequisite: the v$pkgver tag must already contain the no-x11 Cargo.toml
# change. After tagging, refresh the source checksum with:
#   updpkgsums && makepkg --printsrcinfo > .SRCINFO

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

source=("$pkgname-$pkgver.tar.gz::$url/archive/refs/tags/v$pkgver.tar.gz")
sha256sums=('SKIP')

# Keep Cargo's downloads and the Zed-crate git checkouts inside $srcdir so the
# build is hermetic and nothing leaks into the maintainer's home.
export CARGO_HOME="$srcdir/cargo-home"

prepare() {
    default_prepare
    # Honor the committed Cargo.lock — fail loudly if it's drifted from
    # Cargo.toml instead of silently rewriting it.
    cargo fetch --locked --target "$CARCH-unknown-linux-gnu"
}

build() {
    cd "$pkgname-$pkgver"
    cargo build --release --frozen
}

check() {
    cd "$pkgname-$pkgver"
    cargo test --frozen
}

package() {
    cd "$pkgname-$pkgver"

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
