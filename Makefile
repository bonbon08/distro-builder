PREFIX ?= /usr
DESTDIR ?=
BINDIR = $(PREFIX)/bin
LIBDIR = $(PREFIX)/lib/distro-builder/bin
CARGO_FLAGS ?= --release --locked

build:
	cargo build $(CARGO_FLAGS) --workspace

install: build
	install -dm755 $(DESTDIR)$(BINDIR)
	install -Dm755 target/release/distro-builder $(DESTDIR)$(BINDIR)/distro-builder
	install -dm755 $(DESTDIR)$(LIBDIR)
	install -Dm755 target/release/distro-installer $(DESTDIR)$(LIBDIR)/distro-installer
	install -Dm755 target/release/distro-installer-backend $(DESTDIR)$(LIBDIR)/distro-installer-backend
	install -Dm644 io.github.bonbon08.DistroBuilder.desktop $(DESTDIR)$(PREFIX)/share/applications/io.github.bonbon08.DistroBuilder.desktop
	install -Dm644 LICENSE $(DESTDIR)$(PREFIX)/share/licenses/distro-builder/LICENSE
	install -Dm644 README.md $(DESTDIR)$(PREFIX)/share/doc/distro-builder/README.md
	install -dm755 $(DESTDIR)$(PREFIX)/share/doc/distro-builder/docs
	install -Dm644 docs/*.md $(DESTDIR)$(PREFIX)/share/doc/distro-builder/docs/

uninstall:
	rm -f $(DESTDIR)$(BINDIR)/distro-builder \
	      $(DESTDIR)$(PREFIX)/share/applications/io.github.bonbon08.DistroBuilder.desktop
	rm -rf $(DESTDIR)$(LIBDIR) $(DESTDIR)$(PREFIX)/share/licenses/distro-builder \
	       $(DESTDIR)$(PREFIX)/share/doc/distro-builder

check:
	cargo test $(CARGO_FLAGS) --workspace

.PHONY: build install uninstall check
