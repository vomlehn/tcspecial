Name:           tcspecial-test
%include %{_sourcedir}/common.inc
Summary:        tcspecial's test ground station and payload simulator
Requires:       tcspecial = %{version}-%{release}

%description
tcsmoc, a test mission operations center with a window per payload, and
tcssim, which stands in for the payloads a configuration describes. tcsmoc
starts the tcspecial installed beside it and tcssim. Also the test segsend,
which deletes each filled telemetry log segment it is given. Test software,
never flown.

%install
cp -a %{tcs_stage}/. %{buildroot}/

%preun
# The links this package's links script made, taken away while the script
# is still here, and only when the package is going ($1 = 0), not upgraded.
if [ "$1" = 0 ]; then
	for dir in /usr/local/bin /opt/bin; do
		/opt/tcs/sbin/tcspecial-test-links --remove --dir "$dir" \
			>/dev/null 2>&1 || :
	done
fi

%files
/opt/tcs/bin/tcsmoc
/opt/tcs/bin/tcssim
/opt/tcs/bin/segsend
/opt/tcs/share/doc/tcspecial-test
%config(noreplace) /etc/profile.d/tcspecial-test.sh
/opt/tcs/sbin/tcspecial-test-links
