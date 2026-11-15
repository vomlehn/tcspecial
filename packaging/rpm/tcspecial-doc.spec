Name:           tcspecial-doc
%include %{_sourcedir}/common.inc
Summary:        Manuals, designs and example configurations for tcspecial
Group:          Documentation
# The manuals were tcspecial's, and the examples tcspecial's, before 0.1.1.
Conflicts:      tcspecial < 0.1.1
# The battery and descent examples' programs are Python 3.
Requires:       python3

%description
The TCS manuals and design documents, TCS Design and Documentation at their
head, as HTML with their figures and screenshots, and the example
configurations, in both spellings with their simulator files and diagrams,
which the manuals' figures come from, and the two Python programs the battery
and descent examples run. Nothing a vehicle or a ground station needs to run.

%install
cp -a %{tcs_stage}/. %{buildroot}/

%files
%dir /opt/tcs
%dir /opt/tcs/share
%dir /opt/tcs/share/doc
%dir /opt/tcs/share/examples
%doc /opt/tcs/share/doc/tcspecial-doc
/opt/tcs/share/examples/tcspecial

%include %{_sourcedir}/changelog.inc
