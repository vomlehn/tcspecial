design.html: design.rst table.css
	pandoc -s --css=$(word 2,$^) -o $@ $<

design.txt: design.rst
	pandoc $^ -o $@

.PHONY: clean
clean:
	rm -f design.html design.txt
