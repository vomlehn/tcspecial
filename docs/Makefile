O = ../OUT

all:	design.html manual.html tcslog.html requirements.html

design.html: design.rst
	rst2html design.rst design.html

tcslog.html: tcslog.rst
	rst2html tcslog.rst tcslog.html

manual.html: manual.rst
	rst2html manual.rst manual.html

requirements.html: requirements.rst
	rst2html requirements.rst requirements.html

.PHONY: clean
clean:
	rm -f design.html manual.html tcslog.html requirements.html
