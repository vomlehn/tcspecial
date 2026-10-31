"""The part of tcs-xtce's Python runtime this package uses: the digest, and
the packing the fixed commands' generated code is built on.

tcs-xtce's Python runtime is not published, so this tree vendors it, in
``vendor/tcs-xtce/python/tcs_xtce``: see vendor/tcs-xtce/README.md. The four
modules here are links to the vendored ones, unchanged, so there is one copy
of them in the tree; but they are inside this package, so that installing
the package installs them and an installed ground station needs nothing from
the source tree. Only the digest, the packing that
``tcslibgs/fixed_packet.py`` imports, and what those two import: the rest of
the runtime is nothing a ground station reads.

When tcs-xtce's Python runtime is published, this goes, and the distribution
depends on it instead.
"""
