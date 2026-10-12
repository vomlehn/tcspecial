O = ../OUT

all:	design.html manual.html

design.html: design.rst
	rst2html design.rst design.html

manual.html: manual.rst
	rst2html manual.rst manual.html

.PHONY: clean
clean:
	rm -f design.html manual.html
