# Makefile for the F´ bridge: the TcsBridge component and BridgeTest, the
# deployment it is tested in.
#
# Built with F´'s own tools, fprime-util and the FPP compilers, rather than
# cargo, from an F´ checkout and a Python environment holding F´'s tools,
# both outside this tree. Each is a variable, so that another is one word on
# the command line:
#
#   make test FPRIME_FRAMEWORK=$HOME/fprime FPRIME_VENV=$HOME/fprime-venv
#
# The model -- which channels the bridge has, and their IDs -- is written
# from FPRIME_CONFIG by tcsverify, and the component is built for that
# configuration and no other: see "F´: gateway_fprime" in docs/design.rst.

# The tcspecial tree this directory is in.
TOP := $(abspath $(CURDIR)/..)

# An F´ checkout, v4.4.1 or later, and the virtual environment its tools
# are in: by default, a directory beside this tree holding both.
FPRIME_SCRATCH ?= $(abspath $(TOP)/../fprime-scratch)
FPRIME_FRAMEWORK ?= $(FPRIME_SCRATCH)/fprime
FPRIME_VENV ?= $(FPRIME_SCRATCH)/venv
FPRIME_BIN = $(FPRIME_VENV)/bin
FPRIME_UTIL = $(FPRIME_BIN)/fprime-util

# The configuration the bridge is built for, and the simulator file that
# stands in for its payloads in the test. The fprime example by default.
FPRIME_CONFIG ?= $(TOP)/examples/fprime/fprime.ytcs
FPRIME_SIM ?= $(TOP)/examples/fprime/fprime-sim.yaml

# How many jobs the build runs at once.
JOBS ?= 6

# Where F´ puts what it builds, as fprime-util names them by default, and
# where the model is written: TcsBridge.fpp includes it from there.
BUILD_CACHE = $(CURDIR)/build-fprime-automatic-native
ARTIFACTS = $(CURDIR)/build-artifacts/Linux/BridgeTest
MODEL = $(CURDIR)/Tcs/TcsBridge/model

# Where a test run keeps what it leaves: F´'s ground system's logs, and the
# log of each program the test starts.
RUN = $(CURDIR)/build-test

# The directory the configuration's log node writes in, which must be there:
# the fprime example's. Emptied before each test, as the examples' run
# targets empty theirs.
TELEMETRY ?= /tmp/telemetry

# The lock every run that binds the fixed ports holds, so that two never
# run at once: the same file the other tests that bind them use. The top
# Makefile hands its own PORTS_LOCK in.
TCS_PORTS_LOCK ?= $(HOME)/working/tcs-ports.lock

.PHONY: all
all: build

# Display help
.PHONY: help
help:
	@echo "Makefile for the F´ bridge"
	@echo ""
	@echo "Usage:"
	@echo "  make build    - Write the bridge's model from FPRIME_CONFIG"
	@echo "                  and build the component and its deployment"
	@echo "  make test     - Build, then run the deployment against"
	@echo "                  tcspecial and tcssim and test the bridge"
	@echo "  make model    - Write the model alone"
	@echo "  make clean    - Remove what the build and the test left"
	@echo ""
	@echo "Variables: FPRIME_FRAMEWORK ($(FPRIME_FRAMEWORK)),"
	@echo "  FPRIME_VENV ($(FPRIME_VENV)),"
	@echo "  FPRIME_CONFIG ($(FPRIME_CONFIG)),"
	@echo "  FPRIME_SIM ($(FPRIME_SIM)), JOBS ($(JOBS)),"
	@echo "  TCS_PORTS_LOCK ($(TCS_PORTS_LOCK))"

# Stop, saying what to do, unless the F´ checkout and its tools are there.
.PHONY: check-fprime
check-fprime:
	@if [ ! -f "$(FPRIME_FRAMEWORK)/cmake/FPrime.cmake" ]; then \
		echo "No F´ checkout at $(FPRIME_FRAMEWORK): set" \
			"FPRIME_FRAMEWORK to one, v4.4.1 or later." >&2; \
		exit 1; \
	fi
	@if [ ! -x "$(FPRIME_UTIL)" ]; then \
		echo "No fprime-util in $(FPRIME_BIN): set FPRIME_VENV to" \
			"the virtual environment F´'s tools are in." >&2; \
		exit 1; \
	fi

# tcslib-c, which the bridge is linked with, and tcsverify, which writes its
# model. tcspecial and tcssim too for the test, which runs them.
.PHONY: cargo
cargo:
	cd $(TOP) && cargo build -q -p tcslib-c -p tcsverify -p tcspecial \
		-p tcssim

# The model, written from the configuration once tcsverify has checked it.
# A file that already says what would be written is left alone, so this
# does not make the bridge build again for nothing.
.PHONY: model
model: cargo
	$(TOP)/target/debug/tcsverify --fprime-model=$(MODEL) $(FPRIME_CONFIG)

# settings.ini, which fprime-util reads to find the framework. Written here
# rather than kept, because the framework is wherever FPRIME_FRAMEWORK says,
# and rewritten only when it would change.
.PHONY: settings
settings: check-fprime
	@printf '%s\n' \
		'; Written by fprime/Makefile from FPRIME_FRAMEWORK: set that' \
		'; rather than editing this.' \
		'[fprime]' \
		'framework_path: $(FPRIME_FRAMEWORK)' \
		'project_root: .' \
		'default_cmake_options: FPRIME_ENABLE_FRAMEWORK_UTS=OFF' \
		'    FPRIME_ENABLE_AUTOCODER_UTS=OFF' > settings.ini.new
	@if cmp -s settings.ini.new settings.ini; then \
		rm settings.ini.new; \
	else \
		mv settings.ini.new settings.ini; \
		rm -rf $(BUILD_CACHE); \
	fi

# The build cache, generated once. fprime-util is given the environment's
# own tools, which cmake does not find on PATH by itself. A generation that
# fails leaves a cache half made, which is removed, so that the next build
# generates it again rather than building in it.
$(BUILD_CACHE)/build.ninja:
	cd $(CURDIR) && $(FPRIME_UTIL) generate \
		-DCMAKE_PROGRAM_PATH=$(FPRIME_BIN) \
		-DCMAKE_MAKE_PROGRAM=$(FPRIME_BIN)/ninja \
		-DPYTHON=$(FPRIME_BIN)/python3 \
		|| { rm -rf $(BUILD_CACHE); exit 1; }

# The component and the deployment, built against the model.
.PHONY: build
build: settings model
	$(MAKE) --no-print-directory $(BUILD_CACHE)/build.ninja
	cd $(CURDIR) && PATH=$(FPRIME_BIN):$$PATH $(FPRIME_UTIL) build \
		-j$(JOBS)

# The bridge's test: tcssim standing in for the configuration's payloads,
# tcspecial flying it, BridgeTest running the bridge, and F´'s ground system
# commanding it and reading its channels, holding the ports' lock while it
# runs. See bridge-test.
.PHONY: test
test: build
	rm -rf $(TELEMETRY)
	mkdir $(TELEMETRY)
	FPRIME_BIN=$(FPRIME_BIN) flock $(TCS_PORTS_LOCK) $(CURDIR)/bridge-test $(TOP) $(FPRIME_CONFIG) \
		$(FPRIME_SIM) $(ARTIFACTS) $(RUN)

.PHONY: clean
clean:
	rm -rf $(BUILD_CACHE) $(CURDIR)/build-artifacts $(MODEL) $(RUN) \
		settings.ini settings.ini.new
