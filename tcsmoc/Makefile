# Makefile for the tcsmoc crate's README
#
# Cargo builds the crate itself, from the repository's root: see README.md.
# This builds only the page.

.PHONY: all
all:	docs

# Display help
.PHONY: help
help:
	@echo "Makefile for the tcsmoc README"
	@echo ""
	@echo "Usage:"
	@echo "  make README.html     - Build README.html from README.md"
	@echo "  make clean           - Remove README.html"

.PHONY: docs
docs: README.html

# A page of its own, so pandoc's --standalone: it carries its own head and
# styling, and reads the same wherever it is copied to. README.md's first
# heading is made the page's title, with a table of contents under it.
README.html: README.md
	pandoc --standalone --shift-heading-level-by=-1 --toc --toc-depth=3 \
		--metadata toc-title="Table of Contents" \
		--metadata pagetitle=tcsmoc README.md -o README.html

.PHONY: clean
clean:
	rm -f README.html
