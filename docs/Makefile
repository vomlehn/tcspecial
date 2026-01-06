design.html: design.rst table.css
	pandoc -s --css=$(word 2,$^) -o $@ $< --toc

design.txt: design.rst
	pandoc $^ -o $@ --toc

.PHONY: clean
clean:
	rm -f design.html design.txt
