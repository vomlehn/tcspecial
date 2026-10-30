O = ../OUT

.PHONY: all
all:	design.html manual.html third-party.html

# The message-flow figures of "In-flight data across a restart", drawn by
# restart_flows.py from the lists of messages it holds.
FLOWS = restart-commanded.svg restart-crash.svg restart-ground.svg

design.html: design.rst $(FLOWS)
	rst2html design.rst design.html

$(FLOWS) &: restart_flows.py
	python3 restart_flows.py

manual.html: manual.rst
	rst2html manual.rst manual.html

third-party.html: third-party.rst
	rst2html third-party.rst third-party.html

.PHONY: clean
clean:
	rm -f design.html manual.html third-party.html
