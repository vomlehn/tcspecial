Name:           tcspecial-doc
%include %{_sourcedir}/common.inc
Summary:        Manuals and designs for tcspecial
Group:          Documentation
# The manuals were tcspecial's, and the examples tcspecial's, before 0.1.1.
Conflicts:      tcspecial < 0.1.1
# The manuals' figures are the examples' diagrams, which it installs.
Requires:       tcspecial-examples = %{version}-%{release}

%description
The TCS manuals and design documents, TCS Design and Documentation at their
head, as HTML with their figures and screenshots. Their figures are the
examples' diagrams, which tcspecial-examples installs. Nothing a vehicle or
a ground station needs to run.

%install
cp -a %{tcs_stage}/. %{buildroot}/

%files
%dir /opt/tcs
%dir /opt/tcs/share
%dir /opt/tcs/share/doc
%doc /opt/tcs/share/doc/tcspecial-doc

%include %{_sourcedir}/changelog.inc
