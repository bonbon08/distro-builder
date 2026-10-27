# Maintainer: bonbon08 <mail@jan-schmitt.eu>
pkgname=distro-builder
pkgver=0.1.0
pkgrel=1
pkgdesc="Baukasten für eigene Arch-basierte Linux-Distributionen: GUI, archiso-ISO und grafischer Installer"
arch=('any')
url="https://github.com/bonbon08/distro-builder"
license=('MIT')
depends=('python' 'python-gobject' 'gtk4' 'libadwaita' 'gdk-pixbuf2' 'archiso' 'polkit' 'pacman')
optdepends=('skopeo: Container-Images in die ISO packen'
            'docker: Alternative zu skopeo für Container-Images'
            'qemu-desktop: fertige ISO in einer VM testen'
            'edk2-ovmf: VM-Test im UEFI-Modus'
            'xdg-utils: Ausgabeordner öffnen')
checkdepends=('python')
source=("$pkgname-$pkgver.tar.gz::$url/archive/refs/tags/v$pkgver.tar.gz")
sha256sums=('afef027b6c27020ae4756da870e4e13c20c465032773fd1c21fd2f339b0a3af1')

check() {
  cd "$pkgname-$pkgver"
  python -m unittest discover -s tests
}

package() {
  cd "$pkgname-$pkgver"
  make DESTDIR="$pkgdir" PREFIX=/usr install
}
