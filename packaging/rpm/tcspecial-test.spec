Name:           tcspecial-test
%include %{_sourcedir}/common.inc
Summary:        Test ground station and payload simulator for tcspecial
Group:          Applications/Engineering
Requires:       tcspecial = %{version}-%{release}
# The test apps were tcspecial's before 0.1.1.
Conflicts:      tcspecial < 0.1.1

%description
tcs-moc, a test mission operations center with a window per payload, and
tcs-sim, which simulates the payloads a configuration describes. tcs-moc
starts the tcspecial installed beside it and tcs-sim. Also the test segsend,
which deletes each filled telemetry log segment it is given, and the five
test app programs, under /opt/tcs/libexec, that the app node types are tested
with. Verification tools, never flown.

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
%dir /opt/tcs
%dir /opt/tcs/bin
%dir /opt/tcs/sbin
%dir /opt/tcs/libexec
%dir /opt/tcs/share
%dir /opt/tcs/share/doc
/opt/tcs/bin/tcs-moc
/opt/tcs/bin/tcs-sim
/opt/tcs/bin/segsend
/opt/tcs/libexec/app_pl
/opt/tcs/libexec/app_all_pl
/opt/tcs/libexec/app_gateway_veh
/opt/tcs/libexec/app_payload_veh
/opt/tcs/libexec/app_all_veh
%doc /opt/tcs/share/doc/tcspecial-test
%config(noreplace) /etc/profile.d/tcspecial-test.sh
/opt/tcs/sbin/tcspecial-test-links

%include %{_sourcedir}/changelog.inc
