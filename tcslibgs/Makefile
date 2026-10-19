# Makefile for the tcslibgs crate's README
#
# Cargo builds the crate itself, from the repository's root: see README.md.
# This builds only the page.

.PHONY: all
all:	docs

# Display help
.PHONY: help
help:
	@echo "Makefile for the tcslibgs README"
	@echo ""
	@echo "Usage:"
	@echo "  make README.html     - Build README.html from README.md"
	@echo "  make clean           - Remove README.html"

.PHONY: docs
docs: README.html

# A page of its own, so pandoc's --standalone: it carries its own head and
# styling, and reads the same wherever it is copied to.
README.html: README.md
	pandoc --standalone --metadata pagetitle=tcslibgs README.md -o README.html

.PHONY: clean
clean:
	rm -f README.html
