Name:           tcspecial-moc
%include %{_sourcedir}/common.inc
Summary:        Ground libraries and configuration editor for tcspecial
Group:          Development/Libraries
Requires:       python3
Recommends:     python3-pyyaml, python3-tkinter, pkgconf-pkg-config
# This package was tcspecial-ground, whose files it holds now: an upgrade
# takes the old one out. Every tcspecial-ground built was 0.1.0.
Obsoletes:      tcspecial-ground < 0.1.1

# Where the .pth file that puts /opt/tcs/lib/python3/site-packages on
# python3's path goes: the system python3's own site-packages, which on
# Fedora names its minor version (/usr/lib/python3.13/site-packages) because
# Fedora has no directory every python3 reads. Fedora's macro, where there is
# one; elsewhere the same path, asked of python3.
%{!?python3_sitelib: %global python3_sitelib %(python3 -c '
import sysconfig
print(sysconfig.get_path("purelib", "posix_prefix", {"base": "/usr"}))')}

%description
What a ground station for tcspecial is built on: libtcs_moc_c,
tcslib-moc for C and C++, with its header tcslib-moc-c.h and pkg-config file
tcslib-moc-c.pc; the Python packages tcslib_protocol, tcslib_moc and tcsui;
and tcsgui, the graphical configuration editor, with its node descriptions
and schemas.

%install
cp -a %{tcs_stage}/. %{buildroot}/
# The stage puts the .pth where Debian's python3 reads it; this is the
# place Fedora's does.
rm -rf %{buildroot}/usr/lib/python3
install -d %{buildroot}%{python3_sitelib}
echo /opt/tcs/lib/python3/site-packages \
	> %{buildroot}%{python3_sitelib}/tcspecial-moc.pth
# RPM finds what a shared library provides only in an executable file,
# where Debian wants one that is not.
chmod 755 %{buildroot}/opt/tcs/lib/libtcs_moc_c.so.[0-9]*

%post -p /sbin/ldconfig
%postun -p /sbin/ldconfig

%preun
# The links this package's links script made, taken away while the script
# is still here, and only when the package is going ($1 = 0), not upgraded.
if [ "$1" = 0 ]; then
	for dir in /usr/local/bin /opt/bin; do
		/opt/tcs/sbin/tcspecial-moc-links --remove --dir "$dir" \
			>/dev/null 2>&1 || :
	done
fi

%files
%dir /opt/tcs
%dir /opt/tcs/bin
%dir /opt/tcs/sbin
%dir /opt/tcs/include
%dir /opt/tcs/lib
%dir /opt/tcs/lib/pkgconfig
%dir /opt/tcs/lib/python3
%dir /opt/tcs/share
%dir /opt/tcs/share/doc
/opt/tcs/bin/tcsgui
/opt/tcs/include/tcslib-moc-c.h
/opt/tcs/lib/libtcs_moc_c.so*
/opt/tcs/lib/libtcs_moc_c.a
/opt/tcs/lib/pkgconfig/tcslib-moc-c.pc
/opt/tcs/lib/python3/site-packages
/opt/tcs/share/tcsgui
/opt/tcs/share/tcspecial
%doc /opt/tcs/share/doc/tcspecial-moc
%config(noreplace) /etc/ld.so.conf.d/tcspecial-moc.conf
%config(noreplace) /etc/profile.d/tcspecial-moc.sh
%{python3_sitelib}/tcspecial-moc.pth
/opt/tcs/sbin/tcspecial-moc-links

%include %{_sourcedir}/changelog.inc
