O = ../OUT

all:	design.d requirements.d

.PHONY: design.d
design.d:
	cd design && $(MAKE) SOURCEDIR=. BUILDDIR=../$O html

.PHONY: requirements.d
requirements.d:
	cd requirements && $(MAKE) SOURCEDIR=. BUILDDIR=../$O html

.PHONY: clean
clean:
	rm -rf $O
