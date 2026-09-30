# Installing OClock.
#
# These are the freedesktop rules rather than a distribution's: a binary in
# `$PREFIX/bin`, an application entry in `$PREFIX/share/applications`, the
# AppStream metadata in `$PREFIX/share/metainfo`, the licence in
# `$PREFIX/share/licenses/<appid>`, and the icon in the `hicolor` theme under
# `$PREFIX/share/icons`. A distribution package should use the same paths,
# because that is what makes the launcher, the taskbar and a software centre
# find the application without being told where it is.
#
#   make                 build the release binary
#   make install         install it (PREFIX=/usr/local by default)
#   make install PREFIX=~/.local
#   make uninstall       remove everything `install` put there
#   make icons           re-render the PNGs from the SVG
#   make check           validate the desktop entry and the AppStream data
#
# `DESTDIR` is honoured for staged installs, which is what a package build
# uses. Note that the caches are refreshed for the *final* prefix rather than
# the staged one, because that is where the desktop will look.

# `make -C data` runs here, so every path out of this directory is taken from
# the repository root rather than from wherever make happens to be standing.
REPO    := $(abspath $(CURDIR)/..)

PREFIX  ?= /usr/local
DESTDIR ?=

BINDIR    = $(DESTDIR)$(PREFIX)/bin
DATADIR   = $(DESTDIR)$(PREFIX)/share
# The BSD 3-Clause text the application is distributed under, from the
# repository root. A copy of it is installed alongside the binary, because
# clause 2 requires a redistribution in binary form to carry the licence.
LICENSEDIR = $(DESTDIR)$(PREFIX)/share/licenses/$(APPID)
ICONDIR   = $(DATADIR)/icons/hicolor
APPSDIR   = $(DATADIR)/applications
METADIR   = $(DATADIR)/metainfo

BINARY  = oclock
APPID   = org.odsos.OClock
ICON    = $(APPID).png
SYMBOLIC = $(APPID)-symbolic.svg
THEMEDIR = $(CURDIR)/icons/hicolor
SVG     = $(THEMEDIR)/scalable/apps/$(APPID).svg
SYMBOLIC_SVG = $(THEMEDIR)/symbolic/apps/$(SYMBOLIC)
BUILT   = $(REPO)/target/release/$(BINARY)
LICENSE = $(REPO)/LICENSE

SIZES = 16 22 24 32 48 64 128 256 512

.PHONY: all build icons check install uninstall

all: build

build:
	cargo build --release --manifest-path $(REPO)/Cargo.toml

## Re-render the raster icons from the authoritative SVG.
icons:
	python3 $(CURDIR)/icons/render.py

## Validate the desktop integration data with whatever validators are present.
#
# `appstreamcli` reports two diagnostics that are expected here, and neither is
# a mistake to be fixed in the data:
#
#   * `url-homepage-missing` — AppStream wants a homepage, and this project
#     carries no verified one. Nothing in the repository, the Cargo metadata or
#     this directory says where OClock is hosted, and a URL assembled from the
#     application's ID or the ecosystem's name would look authoritative while
#     resolving to somebody else's page. The field is optional; its absence is
#     the honest answer. `tests/packaging.rs` checks that none has been invented.
#   * `cid-contains-uppercase-letter` — the application ID carries an uppercase
#     letter in its last component, which is the freedesktop convention, and all
#     three of the packaging files have to agree on it.
#
# Everything else the validator reports is a real error, and this target fails
# on it.
check:
	@if command -v desktop-file-validate >/dev/null; then \
		desktop-file-validate $(CURDIR)/$(APPID).desktop; \
		echo "desktop-file-validate: ok"; \
	else \
		echo "desktop-file-validate: not installed, skipped"; \
	fi
	@if command -v appstreamcli >/dev/null; then \
		appstreamcli validate --no-net $(CURDIR)/$(APPID).metainfo.xml; \
	else \
		echo "appstreamcli: not installed, skipped"; \
	fi

install: build
	install -Dm0755 $(BUILT) $(BINDIR)/$(BINARY)
	install -Dm0644 $(CURDIR)/$(APPID).desktop $(APPSDIR)/$(APPID).desktop
	install -Dm0644 $(CURDIR)/$(APPID).metainfo.xml $(METADIR)/$(APPID).metainfo.xml
	install -Dm0644 $(LICENSE) $(LICENSEDIR)/LICENSE
	install -Dm0644 $(SVG) $(ICONDIR)/scalable/apps/$(APPID).svg
	install -Dm0644 $(SYMBOLIC_SVG) $(ICONDIR)/symbolic/apps/$(SYMBOLIC)
	for size in $(SIZES); do \
		install -Dm0644 $(THEMEDIR)/$${size}x$${size}/apps/$(ICON) \
			$(ICONDIR)/$${size}x$${size}/apps/$(ICON); \
	done
	# Refresh the caches the desktop reads. Harmless if the tools are absent.
	-command -v update-desktop-database >/dev/null \
		&& update-desktop-database -q "$(APPSDIR)" || true
	-command -v gtk-update-icon-cache >/dev/null \
		&& gtk-update-icon-cache -qtf "$(ICONDIR)" || true

uninstall:
	rm -f $(BINDIR)/$(BINARY)
	rm -f $(APPSDIR)/$(APPID).desktop
	rm -f $(METADIR)/$(APPID).metainfo.xml
	rm -f $(LICENSEDIR)/LICENSE
	rm -f $(ICONDIR)/scalable/apps/$(APPID).svg
	rm -f $(ICONDIR)/symbolic/apps/$(SYMBOLIC)
	for size in $(SIZES); do \
		rm -f $(ICONDIR)/$${size}x$${size}/apps/$(ICON); \
	done
	-command -v gtk-update-icon-cache >/dev/null \
		&& gtk-update-icon-cache -qtf "$(ICONDIR)" || true
