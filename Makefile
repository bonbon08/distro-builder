PREFIX ?= /usr
DESTDIR ?=
BINDIR = $(PREFIX)/bin
LIBDIR = $(PREFIX)/share/distro-builder

install:
	install -dm755 $(DESTDIR)$(LIBDIR)/distrobuilder/templates
	install -Dm644 distrobuilder/*.py $(DESTDIR)$(LIBDIR)/distrobuilder/
	install -Dm644 distrobuilder/templates/* $(DESTDIR)$(LIBDIR)/distrobuilder/templates/
	install -dm755 $(DESTDIR)$(BINDIR)
	sed 's#/usr/share/distro-builder#$(LIBDIR)#' distro-builder > $(DESTDIR)$(BINDIR)/distro-builder
	chmod 755 $(DESTDIR)$(BINDIR)/distro-builder
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
	python3 -m unittest discover -s tests -v

.PHONY: install uninstall check
