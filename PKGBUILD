# Maintainer: bonbon08 <mail@jan-schmitt.eu>
pkgname=distro-builder
pkgver=0.1.0
pkgrel=1
pkgdesc="Baukasten für eigene Arch-basierte Linux-Distributionen: GUI, archiso-ISO und grafischer Installer"
arch=('x86_64')
url="https://github.com/bonbon08/distro-builder"
license=('MIT')
depends=('gtk4' 'libadwaita' 'archiso' 'polkit' 'pacman' 'gcc-libs' 'glibc')
makedepends=('cargo')
optdepends=('skopeo: Container-Images in die ISO packen'
            'docker: Alternative zu skopeo für Container-Images'
            'qemu-desktop: fertige ISO in einer VM testen'
            'edk2-ovmf: VM-Test im UEFI-Modus'
            'xdg-utils: Ausgabeordner öffnen')
source=("$pkgname-$pkgver.tar.gz::$url/archive/refs/tags/v$pkgver.tar.gz")
# Regenerate with `updpkgsums` once the release tarball for this version exists.
sha256sums=('SKIP')

prepare() {
  cd "$pkgname-$pkgver"
  cargo fetch --locked
}

build() {
  cd "$pkgname-$pkgver"
  cargo build --release --locked --workspace
}

check() {
  cd "$pkgname-$pkgver"
  cargo test --release --locked --workspace
}

package() {
  cd "$pkgname-$pkgver"
  make DESTDIR="$pkgdir" PREFIX=/usr install
}
