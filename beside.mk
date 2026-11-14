# Where the checkouts and scratch directories this tree is used with are:
# the directory the main checkout is in, which every Makefile here includes
# this to know. F´, cFS and SBN, tcs-helpers and the ports' lock are found
# beside it.
#
# Found through git, so that a worktree of this tree -- which lies deeper,
# under .claude/worktrees -- finds the same ones the main checkout does,
# rather than looking beside itself and finding nothing. Where this is not a
# git checkout, a release unpacked, it is the directory this tree is in.
#
# tcs-xtce is found here too. python/tcsprotocol/_xtce's links to its
# runtime are relative, and so point beside the tree itself; from a worktree,
# where there is none, the package looks beside the main checkout instead, by
# the same rule as this: see its __init__.py.

BESIDE_TREE := $(abspath $(dir $(lastword $(MAKEFILE_LIST))))
BESIDE_GIT := $(shell git -C $(BESIDE_TREE) rev-parse \
	--path-format=absolute --git-common-dir 2>/dev/null)
BESIDE := $(abspath $(if $(BESIDE_GIT),$(BESIDE_GIT)/../..,$(BESIDE_TREE)/..))

# The lock every run that binds the fixed ports holds -- the simulated-devices
# test, the F´ bridge's, the cFS run -- so that two never run at once, from
# this checkout or one beside it.
TCS_PORTS_LOCK ?= $(BESIDE)/tcs-ports.lock
