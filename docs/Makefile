O = ../OUT

# manual.rst is the short page that says what each manual covers;
# tcspecial.rst, tcs-cfge.rst, tcs-moc.rst, tcs-sim.rst, tcs-verify.rst and
# tcs-conflict.rst are the programs' manuals. The pages are committed beside
# their sources: the
# pre-commit hook remakes the page of each document a commit changes and adds
# it to the commit, and make html-check in the top directory fails where one
# is behind (see make hooks there).
MANUALS = manual.html tcspecial.html tcs-cfge.html tcs-moc.html tcs-sim.html \
	tcs-verify.html tcs-conflict.html

# design.rst says the same of the design documents, one per program as the
# manuals are, and one for the lander example, which is planned.
# tcspecial-design.html, which draws the restart figures, has a rule of its
# own.
DESIGNS = design.html tcs-cfge-design.html tcs-moc-design.html \
	tcs-sim-design.html tcs-verify-design.html tcs-conflict-design.html \
	lander-design.html

.PHONY: all
all:	index.html tcspecial-design.html $(DESIGNS) $(MANUALS) \
	third-party.html node-types.html building-a-vehicle.html \
	desk-to-flight.html getting-started.html

# The page the site gives at docs/: what each document here is, made from
# README.rst, which GitHub shows when the directory is browsed.
index.html: README.rst
	rst2html README.rst index.html

# The first page to read: the released packages installed, and an example
# run, in about fifteen minutes.
getting-started.html: getting-started.rst
	rst2html getting-started.rst getting-started.html

# The message-flow figures of "In-flight data across a restart" and "The
# restart and the beacon, in time", drawn by
# restart_flows.py from the lists of messages it holds.
FLOWS = restart-commanded.svg restart-crash.svg restart-ground.svg \
	restart-beacons.svg restart-lost-up.svg restart-lost-answer.svg \
	restart-lost-beacons.svg

tcspecial-design.html: tcspecial-design.rst $(FLOWS)
	rst2html tcspecial-design.rst tcspecial-design.html

# The message-flow figures of the tcs-moc manual, drawn by the same script:
# what goes up and what comes down behind what an operator presses.
MOC_FLOWS = tcs-moc-connect.svg tcs-moc-command.svg tcs-moc-ping.svg

$(FLOWS) $(MOC_FLOWS) &: restart_flows.py
	python3 restart_flows.py

$(MANUALS) $(DESIGNS): %.html: %.rst
	rst2html $< $@

# The figures of the overview of the TCS software, drawn by hand.
manual.html: tcs-flight.svg tcs-desktop.svg

# The figures of the tcs-sim manual: where tcs-sim sits, drawn by hand, and the
# screenshots of its window, which make screenshots-tcs-sim in the top
# directory makes again.
tcs-sim.html: tcs-sim-between.svg $(wildcard shots/tcs-sim-*.png)

# The figures of the tcs-cfge manual: how a crash is recovered from, drawn by
# hand, and the screenshots of its windows, which make screenshots-tcs-cfge in
# the top directory makes again.
tcs-cfge.html: tcs-cfge-recovery.svg $(wildcard shots/tcs-cfge-*.png)

# The figures of the tcs-moc manual: its message flows, the restart's from
# the design, and the screenshots of its windows, which make
# screenshots-tcs-moc in the top directory makes again.
tcs-moc.html: $(MOC_FLOWS) restart-beacons.svg $(wildcard shots/tcs-moc-*.png)

third-party.html: third-party.rst
	rst2html third-party.rst third-party.html

node-types.html: node-types.rst
	rst2html node-types.rst node-types.html

# The guide from the examples to a whole vehicle. Its excerpts are read from
# the stage files as the page is made, so a stage changed is a page made
# again; its figures are tcs-cfge's drawings of the stages, which
# building-a-vehicle/screenshots.sh makes again, and the vehicle-*.svg
# figures drawn by hand.
building-a-vehicle.html: building-a-vehicle.rst \
		$(wildcard building-a-vehicle/*/*.ytcs) \
		$(wildcard shots/vehicle-stage-*.png) \
		$(wildcard vehicle-*.svg)
	rst2html building-a-vehicle.rst building-a-vehicle.html

# What changes between a configuration run on a desk and one that flies.
desk-to-flight.html: desk-to-flight.rst
	rst2html desk-to-flight.rst desk-to-flight.html

.PHONY: clean
clean:
	rm -f tcspecial-design.html $(DESIGNS) $(MANUALS) third-party.html \
		node-types.html building-a-vehicle.html desk-to-flight.html
