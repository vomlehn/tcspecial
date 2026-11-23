# Makefile for the tcsmoc-c crate's README
#
# Cargo builds the crate itself, from the repository's root: see README.md.
# This builds only the page.

.PHONY: all
all:	docs

# Display help
.PHONY: help
help:
	@echo "Makefile for the tcsmoc-c README"
	@echo ""
	@echo "Usage:"
	@echo "  make index.html     - Build index.html from README.md"
	@echo "  make clean           - Remove index.html"

.PHONY: docs
docs: index.html

# A page of its own, so pandoc's --standalone: it carries its own head and
# styling, and reads the same wherever it is copied to. README.md's first
# heading is made the page's title, with a table of contents under it.
index.html: README.md
	pandoc --standalone --shift-heading-level-by=-1 --toc --toc-depth=3 \
		--metadata toc-title="Table of Contents" \
		--metadata pagetitle=tcsmoc-c README.md -o index.html

.PHONY: clean
clean:
	rm -f index.html
