Name:           tcspecial-ground
%include %{_sourcedir}/common.inc
Summary:        tcspecial's ground libraries and configuration editor
Requires:       python3
Recommends:     python3-pyyaml, python3-tkinter, pkgconf-pkg-config

# Where the .pth file that puts /opt/tcs/lib/python3/site-packages on
# python3's path goes: the system python3's own site-packages, which on
# Fedora names its minor version (/usr/lib/python3.13/site-packages) because
# Fedora has no directory every python3 reads. Fedora's macro, where there is
# one; elsewhere the same path, asked of python3.
%{!?python3_sitelib: %global python3_sitelib %(python3 -c '
import sysconfig
print(sysconfig.get_path("purelib", "posix_prefix", {"base": "/usr"}))')}

%description
What a ground station for tcspecial is built on: libtcs, tcslib for C and
C++, with its header tcs.h and pkg-config file tcs.pc; the Python
packages tcslibgs, tcslib and tcsui; and tcsgui, the graphical configuration
editor, with its node descriptions and schemas.

%install
cp -a %{tcs_stage}/. %{buildroot}/
# The stage puts the .pth where Debian's python3 reads it; this is the
# place Fedora's does.
rm -rf %{buildroot}/usr/lib/python3
install -d %{buildroot}%{python3_sitelib}
echo /opt/tcs/lib/python3/site-packages \
	> %{buildroot}%{python3_sitelib}/tcspecial-ground.pth

%post -p /sbin/ldconfig
%postun -p /sbin/ldconfig

%preun
# The links this package's links script made, taken away while the script
# is still here, and only when the package is going ($1 = 0), not upgraded.
if [ "$1" = 0 ]; then
	for dir in /usr/local/bin /opt/bin; do
		/opt/tcs/sbin/tcspecial-ground-links --remove --dir "$dir" \
			>/dev/null 2>&1 || :
	done
fi

%files
/opt/tcs/bin/tcsgui
/opt/tcs/include/tcs.h
/opt/tcs/lib/libtcs.so*
/opt/tcs/lib/libtcs.a
/opt/tcs/lib/pkgconfig/tcs.pc
/opt/tcs/lib/python3/site-packages
/opt/tcs/share/tcsgui
/opt/tcs/share/doc/tcspecial-ground
%config(noreplace) /etc/ld.so.conf.d/tcspecial-ground.conf
%config(noreplace) /etc/profile.d/tcspecial-ground.sh
%{python3_sitelib}/tcspecial-ground.pth
/opt/tcs/sbin/tcspecial-ground-links
