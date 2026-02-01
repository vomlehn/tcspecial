O = ../OUT

all:	design.html requirements.html

design.html: design.rst
	rst2html design.rst design.html

manual.html: manual.rst
	rst2html manual.rst manual.html

requirements.html: requirements.rst
	rst2html requirements.rst requirements.html

.PHONY: clean
clean:
	rm -f design.html requirements.html
