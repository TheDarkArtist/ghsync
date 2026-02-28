pkgname=ghsync
pkgver=0.1.0
pkgrel=1
pkgdesc="Back up all GitHub repos (personal + org) via gh CLI"
arch=('x86_64' 'aarch64')
url="https://github.com/TheDarkArtist/ghsync"
license=('MIT')
depends=('github-cli')
makedepends=('cargo')
source=("${pkgname}-${pkgver}.tar.gz::${url}/archive/v${pkgver}.tar.gz")
sha256sums=('SKIP')

build() {
  cd "${pkgname}-${pkgver}"
  cargo build --release --locked
}

package() {
  cd "${pkgname}-${pkgver}"
  install -Dm755 "target/release/${pkgname}" "${pkgdir}/usr/bin/${pkgname}"
  install -Dm644 LICENSE "${pkgdir}/usr/share/licenses/${pkgname}/LICENSE"
}
