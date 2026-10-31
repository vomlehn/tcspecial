Name:           tcspecial
%include %{_sourcedir}/common.inc
Summary:        Telemetry and control, special: spacecraft flight software
Group:          Applications/System

%description
tcspecial runs a spacecraft's payloads as nodes in a configured graph and
relays their data and commands to and from the mission operations center
over UDP. This package is what flies: tcspecial itself, tcsverify,
the five test app programs under /opt/tcs/libexec, the manual, and the
example configurations. A systemd unit, tcspecial.service, is installed but
not enabled.

%install
cp -a %{tcs_stage}/. %{buildroot}/

%post
if [ -d /run/systemd/system ]; then systemctl daemon-reload || :; fi

%postun
if [ -d /run/systemd/system ]; then systemctl daemon-reload || :; fi

%preun
# The links this package's links script made, taken away while the script
# is still here, and only when the package is going ($1 = 0), not upgraded.
if [ "$1" = 0 ]; then
	for dir in /usr/local/bin /opt/bin; do
		/opt/tcs/sbin/tcspecial-links --remove --dir "$dir" \
			>/dev/null 2>&1 || :
	done
fi

%files
/opt/tcs/bin/tcspecial
/opt/tcs/bin/tcsverify
/opt/tcs/libexec/app_pl
/opt/tcs/libexec/app_all_pl
/opt/tcs/libexec/app_gateway_veh
/opt/tcs/libexec/app_payload_veh
/opt/tcs/libexec/app_all_veh
%doc /opt/tcs/share/doc/tcspecial
/opt/tcs/share/examples/tcspecial
%dir /etc/opt/tcs/tcspecial
%dir /var/opt/tcs/tcspecial
%config(noreplace) /etc/profile.d/tcspecial.sh
/usr/lib/systemd/system/tcspecial.service
/opt/tcs/sbin/tcspecial-links

%include %{_sourcedir}/changelog.inc
