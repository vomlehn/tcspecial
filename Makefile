README.html: README.md docs/design.html
	pandoc $^ -o $@

docs/design.html: docs/design.rst
	cd docs && $(MAKE)
