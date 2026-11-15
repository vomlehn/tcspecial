Name:           tcspecial-examples
%include %{_sourcedir}/common.inc
Summary:        Examples for tcspecial, and tcsx, which runs them
Group:          Applications/Engineering
# tcsx runs the examples by tcs-moc and tcs-sim, checks them with
# tcs-verify, and their ground stations are tcsmoc's Python and tcs_ui's.
Requires:       tcspecial-test = %{version}-%{release}
Requires:       tcs-tools = %{version}-%{release}
Requires:       tcsmoc = %{version}-%{release}
Requires:       python3
Requires:       python3-tkinter
# The examples were tcspecial-doc's before 0.2.0.
Conflicts:      tcspecial-doc < 0.2.0
# The windows open through libraries loaded when beacon runs, which RPM
# cannot see that it needs: the keyboard, X11 or Wayland, EGL and
# OpenGL. The .deb names the same ones.
Requires:       libxkbcommon.so.0()(64bit)
Requires:       libxkbcommon-x11.so.0()(64bit)
Requires:       libX11.so.6()(64bit)
Requires:       libX11-xcb.so.1()(64bit)
Requires:       libXcursor.so.1()(64bit)
Requires:       libXi.so.6()(64bit)
Requires:       libXrender.so.1()(64bit)
Requires:       libEGL.so.1()(64bit)
Requires:       libGL.so.1()(64bit)
Requires:       libwayland-client.so.0()(64bit)
Requires:       libwayland-egl.so.1()(64bit)

%description
Every tcspecial example, under /opt/tcs/share/examples/tcspecial, laid out
as in the source tree: its configurations, in both spellings with their
XTCE, simulator files and diagrams, and the programs it runs -- payloads,
compute nodes and ground stations. With them the examples' Rust ground
stations, beacon and lorem_gateway, and tcsx: tcsx lists the examples, and
tcsx NAME runs one, by tcs-moc or by its own ground station. Examples,
never flown.

%install
cp -a %{tcs_stage}/. %{buildroot}/

%preun
# The links this package's links script made, taken away while the script
# is still here, and only when the package is going ($1 = 0), not upgraded.
if [ "$1" = 0 ]; then
	for dir in /usr/local/bin /opt/bin; do
		/opt/tcs/sbin/tcspecial-examples-links --remove --dir "$dir" \
			>/dev/null 2>&1 || :
	done
fi

%files
%dir /opt/tcs
%dir /opt/tcs/bin
%dir /opt/tcs/sbin
%dir /opt/tcs/share
%dir /opt/tcs/share/doc
%dir /opt/tcs/share/examples
/opt/tcs/bin/beacon
/opt/tcs/bin/lorem_gateway
/opt/tcs/bin/tcsx
/opt/tcs/share/examples/tcspecial
%doc /opt/tcs/share/doc/tcspecial-examples
%config(noreplace) /etc/profile.d/tcspecial-examples.sh
/opt/tcs/sbin/tcspecial-examples-links

%include %{_sourcedir}/changelog.inc
