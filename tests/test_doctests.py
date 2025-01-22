# coding: utf-8
"""Test doctest contained tests in every file of the module.
"""

import os
import sys
import datetime
import doctest
import warnings
import pprint
import textwrap
import types

import gb_io

_cwd = None

def setUp(test):
    global _cwd
    warnings.simplefilter('ignore')
    _path = os.path.realpath(os.path.join(__file__, "..", ".."))
    if _path != os.getcwd():
        _cwd = os.getcwd()
        os.chdir(_path)

def tearDown(test):
    warnings.simplefilter(warnings.defaultaction)
    if _cwd is not None:
        os.chdir(_cwd)


def _load_tests_from_module(tests, module, globs, setUp=None, tearDown=None):
    """Load tests from module, iterating through submodules"""

    module.__test__ = {}
    for attr in (getattr(module, x) for x in dir(module) if not x.startswith('_')):
        if isinstance(attr, types.ModuleType):
            _load_tests_from_module(tests, attr, globs, setUp, tearDown)
        else:
            module.__test__[attr.__name__] = attr

    tests.addTests(doctest.DocTestSuite(
        module,
        globs=globs,
        setUp=setUp,
        tearDown=tearDown,
        optionflags=doctest.ELLIPSIS,
    ))

    return tests


def load_tests(loader, tests, ignore):
    """load_test function used by unittest to find the doctests"""

    globs = {
        "gb_io": gb_io,
    }

    if not sys.argv[0].endswith('green'):
        tests = _load_tests_from_module(tests, gb_io, globs, setUp, tearDown)
    return tests



