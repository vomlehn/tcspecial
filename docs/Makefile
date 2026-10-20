O = ../OUT

.PHONY: all
all:	design.html manual.html third-party.html

design.html: design.rst
	rst2html design.rst design.html

manual.html: manual.rst
	rst2html manual.rst manual.html

third-party.html: third-party.rst
	rst2html third-party.rst third-party.html

.PHONY: clean
clean:
	rm -f design.html manual.html third-party.html
