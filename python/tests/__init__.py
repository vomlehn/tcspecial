"""Tests of the Python spellings: see examples/ping/tests/python.rs, which
runs them under cargo test."""

import unittest

from tcslib_protocol import configuration


def loaded(path):
    """Loads a configuration a test has written, with its own XTCE written
    beside it first, by :func:`tcslib_protocol.configuration.write_own_xtce`, as
    ``make xtce`` writes a shipped one's: a ground station reads what each
    node sends from there.

    A configuration tcsverify cannot write one for is loaded without it, so
    that a test of what is wrong with it still sees the Python say so.

    Args:
        path: The configuration's file.

    Returns:
        The :class:`tcslib_protocol.configuration.Configuration`.

    Raises:
        unittest.SkipTest: tcsverify is not built.
    """
    if configuration.find_tcsverify() is None:
        raise unittest.SkipTest("tcsverify is not built")
    configuration.write_own_xtce(path)
    return configuration.load(path)
