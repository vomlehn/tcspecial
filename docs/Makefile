design.html: design.rst
	pandoc $^ -o $@

design.txt: design.rst
	pandoc $^ -o $@

.PHONY: clean
clean:
	rm -f design.html design.txt
