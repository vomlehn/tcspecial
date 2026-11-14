Name:           tcspecial
%include %{_sourcedir}/common.inc
Summary:        Telemetry and control, special: spacecraft flight software
Group:          Applications/System

%description
tcspecial, the TCSpecial Flight Executive, runs a spacecraft's payloads as
nodes in a configured graph and relays their data and commands to and from
the mission operations center over UDP. This package is what flies,
operational code alone: tcspecial itself, with its README, licences and
notices. The configuration checker and the test apps are the tcs-tools and
tcspecial-test packages, and the manuals and examples tcspecial-doc. A
systemd unit, tcspecial.service, is installed but not enabled.

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
%dir /opt/tcs
%dir /opt/tcs/bin
%dir /opt/tcs/sbin
%dir /opt/tcs/share
%dir /opt/tcs/share/doc
%dir /etc/opt/tcs
%dir /var/opt/tcs
/opt/tcs/bin/tcspecial
%doc /opt/tcs/share/doc/tcspecial
%dir /etc/opt/tcs/tcspecial
%dir /var/opt/tcs/tcspecial
%config(noreplace) /etc/profile.d/tcspecial.sh
/usr/lib/systemd/system/tcspecial.service
/opt/tcs/sbin/tcspecial-links

%include %{_sourcedir}/changelog.inc
