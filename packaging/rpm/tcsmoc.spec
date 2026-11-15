Name:           tcsmoc
%include %{_sourcedir}/common.inc
Summary:        TCSpecial MOC Correspondent: what a ground station is built on
Group:          Development/Libraries
Requires:       python3, python3-pyyaml
Recommends:     pkgconf-pkg-config
# This package was tcspecial-moc, and before that tcspecial-ground, whose
# operational code it holds now: an upgrade takes the old one out. Every
# one of them built was 0.1.0.
Obsoletes:      tcspecial-ground < 0.1.1
Obsoletes:      tcspecial-moc < 0.1.1

%description
What a ground station for tcspecial is built on: libtcsmoc-c, the TCSpecial
MOC Correspondent (tcsmoc) for C and C++, with its header tcsmoc-c.h
and pkg-config file tcsmoc-c.pc; the Python packages tcsprotocol (TCS
Link Support) and tcsmoc (the Correspondent); and the schemas of the
configuration files they read. Operational code alone: the configuration
editor and checker are the tcs-tools package.

%install
cp -a %{tcs_stage}/. %{buildroot}/
# The stage puts the .pth where Debian's python3 reads it; this one's is
# written when it is installed, by the trigger below.
rm -rf %{buildroot}/usr/lib/python3
# RPM finds what a shared library provides only in an executable file,
# where Debian wants one that is not.
chmod 755 %{buildroot}/opt/tcs/lib/libtcsmoc-c.so.[0-9]*

# The .pth file that puts /opt/tcs/lib/python3/site-packages on python3's
# path. It goes in python3's own site-packages, which on Fedora names its
# minor version (/usr/lib/python3.15/site-packages), so the python3 that is
# installed says where, rather than the one that built the package. This
# runs when the package is installed with a python3 there, and again when
# python3 is installed or upgraded, moving the file to the new version's
# directory; it never fails the transaction.
%triggerin -- python3
rm -f /usr/lib/python3*/site-packages/tcsmoc.pth
site=$(python3 -c 'import sysconfig
print(sysconfig.get_path("purelib", "posix_prefix", {"base": "/usr"}))') \
	&& mkdir -p "$site" \
	&& echo /opt/tcs/lib/python3/site-packages > "$site/tcsmoc.pth" || :

# python3 removed ($2 is 0) takes the .pth with it.
%triggerun -- python3
if [ "$2" = 0 ]; then
	rm -f /usr/lib/python3*/site-packages/tcsmoc.pth
fi

%post -p /sbin/ldconfig
%postun -p /sbin/ldconfig

%preun
# The .pth file goes when the package does ($1 = 0), not on an upgrade.
if [ "$1" = 0 ]; then
	rm -f /usr/lib/python3*/site-packages/tcsmoc.pth
fi

%files
%dir /opt/tcs
%dir /opt/tcs/include
%dir /opt/tcs/lib
%dir /opt/tcs/lib/pkgconfig
%dir /opt/tcs/lib/python3
%dir /opt/tcs/share
%dir /opt/tcs/share/doc
/opt/tcs/include/tcsmoc-c.h
/opt/tcs/lib/libtcsmoc-c.so*
/opt/tcs/lib/libtcsmoc-c.a
/opt/tcs/lib/pkgconfig/tcsmoc-c.pc
/opt/tcs/lib/python3/site-packages
/opt/tcs/share/tcspecial
%doc /opt/tcs/share/doc/tcsmoc
%config(noreplace) /etc/ld.so.conf.d/tcsmoc.conf
%config(noreplace) /etc/profile.d/tcsmoc.sh

%include %{_sourcedir}/changelog.inc
