# Makefile for the run against cFS's own Linux build: cFS with SBN v7.0.1
# and its UDP module, peered with tcspecial flying examples/cfs.
#
# cFS is built with its own tools, cmake and make, from a cFS checkout and
# an SBN checkout outside this tree, into a scratch directory outside it
# too; nothing is installed. Each is a variable, so that another is one
# word on the command line:
#
#   make test CFS_PRISTINE=$HOME/cFS SBN_SRC=$HOME/sbn
#
# The cFS checkout is copied and never changed, and its submodules are
# fetched at the commits it pins: see prepare. What differs from cFE's
# sample mission is in defs/, and says why in each file. See "What crosses
# the bus" in docs/tcspecial-design.rst for what the run is the test of.

# The tcspecial tree this directory is in, and what sits beside it: see
# beside.mk, which also names TCS_PORTS_LOCK.
TOP := $(abspath $(CURDIR)/..)
include $(TOP)/beside.mk

# A checkout of nasa/cFS, a git checkout of SBN v7.0.1 -- git, so that its
# SBN_IDENT is expanded -- and the directory the build is made in: by
# default, directories beside this tree.
CFS_PRISTINE ?= $(BESIDE)/cfs-pristine
SBN_SRC ?= $(BESIDE)/fprime-scratch/sbn-src/sbn
CFS_SCRATCH ?= $(BESIDE)/cfs-scratch

# The configuration flown.
CFS_CONFIG ?= $(TOP)/examples/cfs/cfs.ytcs

# How many jobs the build runs at once.
JOBS ?= 6

# The tree prepare makes, and where cFS's build puts the one processor's
# executable and its files.
TREE = $(CFS_SCRATCH)/cFS
EXE = $(TREE)/build/exe/cpu1

# Where a test run keeps what it leaves: each program's log, tcspecial's
# telemetry log and state file, and the configuration flown.
RUN = $(CFS_SCRATCH)/run

# cFS's cmake files ask for versions of cmake older than 3.5, which cmake 4
# refuses unless told the oldest policies it may use.
export CMAKE_POLICY_VERSION_MINIMUM = 3.5

.PHONY: all
all: build

# Display help
.PHONY: help
help:
	@echo "Makefile for the run against cFS's own Linux build"
	@echo ""
	@echo "Usage:"
	@echo "  make build    - Make the cFS tree in CFS_SCRATCH and build it"
	@echo "  make test     - Build, then fly CFS_CONFIG against it and"
	@echo "                  test it from the cFS's ground"
	@echo "  make prepare  - Make the cFS tree alone"
	@echo "  make clean    - Remove the tree, the build and the run"
	@echo ""
	@echo "Variables: CFS_PRISTINE ($(CFS_PRISTINE)),"
	@echo "  SBN_SRC ($(SBN_SRC)),"
	@echo "  CFS_SCRATCH ($(CFS_SCRATCH)),"
	@echo "  CFS_CONFIG ($(CFS_CONFIG)), JOBS ($(JOBS)),"
	@echo "  TCS_PORTS_LOCK ($(TCS_PORTS_LOCK))"

# Stop, saying what to do, unless the two checkouts are there.
.PHONY: check-sources
check-sources:
	@if [ ! -f "$(CFS_PRISTINE)/.gitmodules" ]; then \
		echo "No cFS checkout at $(CFS_PRISTINE): set CFS_PRISTINE" \
			"to a clone of https://github.com/nasa/cFS." >&2; \
		exit 1; \
	fi
	@if [ ! -f "$(SBN_SRC)/fsw/platform_inc/sbn_types.h" ]; then \
		echo "No SBN at $(SBN_SRC): set SBN_SRC to a git clone of" \
			"https://github.com/nasa/SBN at v7.0.1." >&2; \
		exit 1; \
	fi

# The cFS tree, made again from the checkouts and defs/ each time.
.PHONY: prepare
prepare: check-sources
	$(CURDIR)/prepare $(CFS_PRISTINE) $(SBN_SRC) $(CFS_SCRATCH) \
		$(CURDIR)/defs

# cFS, built for this machine and put in the build tree's exe/, which is
# where cFS's own "install" puts it. Generated again from nothing, since
# prepare replaced the tree.
.PHONY: build
build: prepare
	cd $(TREE) && $(MAKE) SIMULATION=native prep
	cd $(TREE) && $(MAKE) -j$(JOBS) install

# tcspecial, and the test that is the cFS's ground, built before anything is
# started, so that nothing waits on a compile while the clock runs.
.PHONY: cargo
cargo:
	cd $(TOP) && cargo build -q -p tcspecial
	cd $(TOP) && cargo test -q -p tcspecial --test cfs_flight --no-run

# The run: cFS and tcspecial started, and the test commanding through
# CI_LAB and reading through TO_LAB. See cfs-test.
.PHONY: test
test: build cargo
	flock $(TCS_PORTS_LOCK) $(CURDIR)/cfs-test $(TOP) $(CFS_CONFIG) \
		$(EXE) $(RUN)

.PHONY: clean
clean:
	rm -rf $(TREE) $(RUN)
