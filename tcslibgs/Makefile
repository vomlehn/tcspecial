# Makefile for the tcslibgs crate's README, and the beacon's packing code
#
# Cargo builds the crate itself, from the repository's root: see README.md.
# This builds the page, and regenerates the code that packs and unpacks a
# beacon from the XTCE that describes it.

.PHONY: all
all:	docs

# Display help
.PHONY: help
help:
	@echo "Makefile for the tcslibgs README"
	@echo ""
	@echo "Usage:"
	@echo "  make README.html     - Build README.html from README.md"
	@echo "  make beacon-packet   - Regenerate the beacon's Rust and" \
		"Python codecs"
	@echo "                         from xtce/beacon.xml"
	@echo "  make clean           - Remove README.html"

.PHONY: docs
docs: README.html

# A page of its own, so pandoc's --standalone: it carries its own head and
# styling, and reads the same wherever it is copied to.
README.html: README.md
	pandoc --standalone --metadata pagetitle=tcslibgs README.md \
		-o README.html

# The beacon's layout is stated once, in xtce/beacon.xml, and the code that
# packs and unpacks it is generated from that by tcs-xtce, vendored in
# vendor/tcs-xtce: Rust for tcspecial and tcslib, and Python for the Python
# ground station. Both are committed, so that building needs no Python; run
# this after changing the XML, and commit what it writes.
TCS_XTCE := cd ../vendor/tcs-xtce/generator && python3 -m tcs_xtce
BEACON_XML := xtce/beacon.xml
BEACON_RS := src/beacon_packet.rs
BEACON_PY := ../python/tcslibgs/beacon_packet.py

.PHONY: beacon-packet
beacon-packet: $(BEACON_RS) $(BEACON_PY)

# The Rust is formatted after it is generated: the generator writes lines
# wider than the 80 columns ../rustfmt.toml allows, and what is committed is
# formatted, so without this a regenerated file would differ from it and
# fail cargo fmt --check. rustfmt finds ../rustfmt.toml from the file's own
# directory; the edition is the crate's, from Cargo.toml.
$(BEACON_RS): $(BEACON_XML)
	$(TCS_XTCE) ../../../tcslibgs/$(BEACON_XML) \
		../../../tcslibgs/$(BEACON_RS) \
		--lang rust --source-name tcslibgs/$(BEACON_XML)
	rustfmt --edition 2021 $(BEACON_RS)

$(BEACON_PY): $(BEACON_XML)
	$(TCS_XTCE) ../../../tcslibgs/$(BEACON_XML) \
		../../../tcslibgs/$(BEACON_PY) \
		--lang python --source-name tcslibgs/$(BEACON_XML)

.PHONY: clean
clean:
	rm -f README.html
