O = ../OUT

# manual.rst is the short page that says what each manual covers;
# tcspecial.rst, tcsgui.rst, tcsmoc.rst, tcssim.rst and tcsverify.rst are
# the programs' manuals. The pages are committed beside their sources: the
# pre-commit hook remakes the page of each document a commit changes and adds
# it to the commit, and make html-check in the top directory fails where one
# is behind (see make hooks there).
MANUALS = manual.html tcspecial.html tcsgui.html tcsmoc.html tcssim.html \
	tcsverify.html

# design.rst says the same of the design documents, one per program as the
# manuals are. tcspecial-design.html, which draws the restart figures, has a
# rule of its own.
DESIGNS = design.html tcsgui-design.html tcsmoc-design.html \
	tcssim-design.html tcsverify-design.html

.PHONY: all
all:	tcspecial-design.html $(DESIGNS) $(MANUALS) third-party.html \
	node-types.html

# The message-flow figures of "In-flight data across a restart" and "The
# restart and the beacon, in time", drawn by
# restart_flows.py from the lists of messages it holds.
FLOWS = restart-commanded.svg restart-crash.svg restart-ground.svg \
	restart-beacons.svg restart-lost-up.svg restart-lost-answer.svg \
	restart-lost-beacons.svg

tcspecial-design.html: tcspecial-design.rst $(FLOWS)
	rst2html tcspecial-design.rst tcspecial-design.html

# The message-flow figures of the tcsmoc manual, drawn by the same script:
# what goes up and what comes down behind what an operator presses.
MOC_FLOWS = tcsmoc-connect.svg tcsmoc-command.svg tcsmoc-ping.svg

$(FLOWS) $(MOC_FLOWS) &: restart_flows.py
	python3 restart_flows.py

$(MANUALS) $(DESIGNS): %.html: %.rst
	rst2html $< $@

# The figures of the overview of the TCS software, drawn by hand.
manual.html: tcs-flight.svg tcs-desktop.svg

# The figures of the tcssim manual: where tcssim sits, drawn by hand, and the
# screenshots of its window, which make screenshots-tcssim in the top
# directory makes again.
tcssim.html: tcssim-between.svg $(wildcard shots/tcssim-*.png)

# The figures of the tcsgui manual: how a crash is recovered from, drawn by
# hand, and the screenshots of its windows, which make screenshots-tcsgui in
# the top directory makes again.
tcsgui.html: tcsgui-recovery.svg $(wildcard shots/tcsgui-*.png)

# The figures of the tcsmoc manual: its message flows, the restart's from
# the design, and the screenshots of its windows, which make
# screenshots-tcsmoc in the top directory makes again.
tcsmoc.html: $(MOC_FLOWS) restart-beacons.svg $(wildcard shots/tcsmoc-*.png)

third-party.html: third-party.rst
	rst2html third-party.rst third-party.html

node-types.html: node-types.rst
	rst2html node-types.rst node-types.html

.PHONY: clean
clean:
	rm -f tcspecial-design.html $(DESIGNS) $(MANUALS) third-party.html \
		node-types.html
