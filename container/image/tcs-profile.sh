# shellcheck shell=sh
# The test containers' login shells: where Yamcs is, for tcspecial's
# examples through it, and, in an interactive one, how to run tcspecial's
# examples once tcspecial-examples is installed, and to edit them and
# tcs-xtce's once their editors are. tcsx is given --no-memlock there,
# since a rootless container's limit on locked memory is the host's, too
# small for tcspecial to lock its memory into RAM as it does by default.
export YAMCS_HOME=/opt/yamcs
case $- in
*i*)
	if [ -x /opt/tcs/bin/tcsx ]; then
		alias tcsx='tcsx --no-memlock'
		echo "Run tcsx to see a list of tcspecial's examples," \
			"tcsx EXAMPLE to run one."
		echo "Run cfge FILE to edit a configuration, cfge -e" \
			"EXAMPLE to edit an example's."
	fi
	if [ -x /opt/tcs/bin/tcs-xtcee ]; then
		echo "Run xtcee FILE to edit an XTCE file, xtcee -e to list" \
			"tcs-xtce's examples, xtcee -e EXAMPLE to edit one."
	fi
	;;
esac
