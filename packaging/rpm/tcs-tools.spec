Name:           tcs-tools
%include %{_sourcedir}/common.inc
Summary:        Configuration editor and checker for tcspecial
Group:          Applications/Engineering
# tcs_ui shows what tcsmoc's beacon code works out, and imports it.
Requires:       tcsmoc = %{version}-%{release}
# The windows open through libraries loaded when tcs-gui runs, which
# RPM cannot see that it needs: the keyboard, X11 or Wayland, EGL and
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
Recommends:     python3-tkinter
# tcs-gui was tcspecial-moc's, which tcsmoc obsoletes; tcs-verify, the test
# apps and the examples were tcspecial's. Every one of them built was 0.1.0.
Obsoletes:      tcspecial-moc < 0.1.1
Conflicts:      tcspecial < 0.1.1

%description
The software tools that write and check tcspecial's configurations:
tcs-gui, the graphical configuration editor, with its node descriptions;
tcs-verify, which says what is wrong with a configuration without running
any of it; tcs-conflict, which says whether two configurations can be run
at once; and tcs_ui, the Python window pieces the examples' ground
stations show. Never flown. The manuals and the example configurations are
the tcspecial-doc package.

%install
cp -a %{tcs_stage}/. %{buildroot}/
# The stage puts the .pth where Debian's python3 reads it; this one's is
# written when it is installed, by the trigger below.
rm -rf %{buildroot}/usr/lib/python3

# The .pth file that puts /opt/tcs/lib/python3/site-packages on python3's
# path. It goes in python3's own site-packages, which on Fedora names its
# minor version (/usr/lib/python3.15/site-packages), so the python3 that is
# installed says where, rather than the one that built the package. This
# runs when the package is installed with a python3 there, and again when
# python3 is installed or upgraded, moving the file to the new version's
# directory; it never fails the transaction.
%triggerin -- python3
rm -f /usr/lib/python3*/site-packages/tcs-tools.pth
site=$(python3 -c 'import sysconfig
print(sysconfig.get_path("purelib", "posix_prefix", {"base": "/usr"}))') \
	&& mkdir -p "$site" \
	&& echo /opt/tcs/lib/python3/site-packages > "$site/tcs-tools.pth" || :

# python3 removed ($2 is 0) takes the .pth with it.
%triggerun -- python3
if [ "$2" = 0 ]; then
	rm -f /usr/lib/python3*/site-packages/tcs-tools.pth
fi

%preun
# The links this package's links script made, and its .pth file, taken
# away while the script is still here, and only when the package is going
# ($1 = 0), not upgraded.
if [ "$1" = 0 ]; then
	rm -f /usr/lib/python3*/site-packages/tcs-tools.pth
	for dir in /usr/local/bin /opt/bin; do
		/opt/tcs/sbin/tcs-tools-links --remove --dir "$dir" \
			>/dev/null 2>&1 || :
	done
fi

%files
%dir /opt/tcs
%dir /opt/tcs/bin
%dir /opt/tcs/sbin
%dir /opt/tcs/lib
%dir /opt/tcs/lib/python3
%dir /opt/tcs/lib/python3/site-packages
%dir /opt/tcs/share
%dir /opt/tcs/share/doc
/opt/tcs/bin/tcs-gui
/opt/tcs/bin/tcs-verify
/opt/tcs/bin/tcs-conflict
/opt/tcs/lib/python3/site-packages/tcs_ui
/opt/tcs/lib/python3/site-packages/tcs_ui-%{version}.dist-info
/opt/tcs/share/tcs-gui
%doc /opt/tcs/share/doc/tcs-tools
%config(noreplace) /etc/profile.d/tcs-tools.sh
/opt/tcs/sbin/tcs-tools-links

%include %{_sourcedir}/changelog.inc
