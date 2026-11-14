"""Tests of the Python spellings: see examples/ping/tests/python.rs, which
runs them under cargo test."""

import unittest

from tcsprotocol import configuration


def loaded(path):
    """Loads a configuration a test has written, with its own XTCE written
    beside it first, by :func:`tcsprotocol.configuration.write_own_xtce`, as
    ``make xtce`` writes a shipped one's: a ground station reads what each
    node sends from there.

    A configuration tcs-verify cannot write one for is loaded without it, so
    that a test of what is wrong with it still sees the Python say so.

    Args:
        path: The configuration's file.

    Returns:
        The :class:`tcsprotocol.configuration.Configuration`.

    Raises:
        unittest.SkipTest: tcs-verify is not built.
    """
    if configuration.find_tcs_verify() is None:
        raise unittest.SkipTest("tcs-verify is not built")
    configuration.write_own_xtce(path)
    return configuration.load(path)
