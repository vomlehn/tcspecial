# What the three TCS specs share, read with %include.
#
# The packages are cut from what `make stage` installed, one directory per
# package, so a spec's %install copies its stage and its %files says which
# of those files it owns. `make rpm` defines tcs_version, tcs_stage, and
# tcs_date, the date of the commit built, for the %changelog that each spec
# ends with by reading changelog.inc.

%{!?tcs_version: %{error: tcs_version is not defined: build with make rpm}}
%{!?tcs_stage: %{error: tcs_stage is not defined: build with make rpm}}
%{!?tcs_date: %{error: tcs_date is not defined: build with make rpm}}

# What the stage holds is shipped as it is: stripped there, or not, as
# `make` was told with STRIP, so that the RPMs hold what the .debs hold. So
# rpmbuild neither strips again nor splits out debug information, and runs
# none of its install-time post-processing.
%global debug_package %{nil}
%global __strip /bin/true
%global __os_install_post %{nil}
# The Python under /opt/tcs is installed without bytecode, as the .deb is:
# the path it is in names no Python version, and bytecode would.
%global _python_bytecompile_extra 0
%global __brp_python_bytecompile %{nil}

Version:        %{tcs_version}
Release:        1%{?dist}
License:        MIT OR Apache-2.0
URL:            https://github.com/vomlehn/tcspecial
%global tcs_maintainer David VomLehn <dvomlehn@gmail.com>
Packager:       %{tcs_maintainer}
