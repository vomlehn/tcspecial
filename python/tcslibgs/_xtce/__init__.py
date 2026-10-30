"""The part of tcs-xtce's Python runtime this package uses: the digest.

tcs-xtce's Python runtime is not published, so this tree vendors it, in
``vendor/tcs-xtce/python/tcs_xtce``: see vendor/tcs-xtce/README.md. The three
modules here are links to the vendored ones, unchanged, so there is one copy
of them in the tree; but they are inside this package, so that installing
the package installs them and an installed ground station needs nothing from
the source tree. Only the digest and what it imports: the rest of the
runtime, its generated classes most of all, is nothing a ground station
reads.

When tcs-xtce's Python runtime is published, this goes, and the distribution
depends on it instead.
"""
