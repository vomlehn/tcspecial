# The test container, in which the TCS projects' packages are installed and
# removed as an administrator would, without touching this machine: see
# README.md, bin/tcs-container and the scripts beside it.
#
#   make container   build both images, kubuntu's and fedora's
#   make kubuntu     build the kubuntu image, for the .debs
#   make fedora      build the fedora image, for the RPMs
#   make container-install [PROJECT=DIR] [PACKAGES=...]
#                    build the packages of the project checked out in DIR,
#                    this tcspecial checkout by default, and install
#                    PACKAGES, by default all of them, in the container
#   make container-remove PACKAGES=...
#                    remove the packages from it
#   make container-installed, container-fresh, container-shell
#                    list what it has installed, start it afresh, and open
#                    a shell in it
#   make container-example [EXAMPLE=NAME] [HIDDEN=1] [ARGS=...]
#                    run one of tcspecial's examples in it, or list them
#   make check       shellcheck the scripts, and fail where index.html is
#                    behind README.md
#   make html        write index.html from README.md
#
# DISTRO is kubuntu, for the .debs, or fedora, for the RPMs.

DISTRO   ?= kubuntu
PROJECT  ?= ..
PACKAGES ?=
TCS_HELPERS ?= ../../tcs-helpers

TCS_CONTAINER = bin/tcs-container --distro $(DISTRO)
FORMAT = $(if $(filter fedora,$(DISTRO)),rpm,deb)

# Prints the package files the project's make deb or make rpm built, named
# exactly by its packaging.mk, so that a stale package left beside them is
# never installed: those of PACKAGES alone when it is given.
PACKAGE_FILES = $(MAKE) -s --no-print-directory -C $(PROJECT) \
		$(if $(PACKAGES),PACKAGES='$(PACKAGES)') \
		--eval 'tcs-package-files: ; @echo $$($(if \
			$(filter rpm,$(FORMAT)),RPM,DEB)_FILES)' \
		tcs-package-files

.PHONY: help
help:
	@sed -n '/^#   make/,/^#$$/p' Makefile | sed 's/^# \{0,1\}//'

.PHONY: container
container: kubuntu fedora

.PHONY: kubuntu
kubuntu:
	bin/tcs-container --distro kubuntu build

.PHONY: fedora
fedora:
	bin/tcs-container --distro fedora build

.PHONY: container-install
container-install:
	$(MAKE) --no-print-directory -C $(PROJECT) $(FORMAT)
	$(TCS_CONTAINER) install $$($(PACKAGE_FILES))

.PHONY: container-remove
container-remove:
	@test -n '$(PACKAGES)' || { echo 'container-remove needs' \
		'PACKAGES=..., the packages to remove' >&2; exit 2; }
	$(TCS_CONTAINER) remove $(PACKAGES)

.PHONY: container-installed
container-installed:
	$(TCS_CONTAINER) installed

.PHONY: container-fresh
container-fresh:
	$(TCS_CONTAINER) fresh

.PHONY: container-shell
container-shell:
	$(TCS_CONTAINER) shell

# Runs one of tcspecial's examples in the container, or with no EXAMPLE
# lists them: HIDDEN=1, for testing, draws its window with Xvfb, where
# nobody sees it, and ARGS go to tcsx, which runs it.
.PHONY: container-example
container-example:
	$(TCS_CONTAINER) example $(if $(HIDDEN),--hidden) $(EXAMPLE) $(ARGS)

.PHONY: check
check:
	shellcheck bin/* image/xtcee image/cfge image/tcs-profile.sh
	$(TCS_HELPERS)/bin/html-pages check .

# README.md is the documentation; index.html is it as a page of its own,
# which tcspecial's manual links, with pandoc's --standalone giving it a
# head, README.md's first heading made its title with a table of contents
# under it. tcs-helpers' pre-commit hook remakes it whenever a commit
# changes README.md, and make check fails where it is behind.
index.html: README.md
	pandoc --standalone --shift-heading-level-by=-1 --toc --toc-depth=3 \
		--metadata toc-title="Table of Contents" \
		--metadata pagetitle="The TCS test container" \
		README.md -o index.html

.PHONY: html
html: index.html
