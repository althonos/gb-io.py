from . import lib
from .lib import *

__doc__ = lib.__doc__
__author__ = lib.__author__
__version__ = lib.__version__

__all__ = [
    "Record",
    "Source",
    "Feature",
    "Qualifier",
    "Location",
    "Range",
    "Between",
    "Complement",
    "Join",
    "Order",
    "Bond",
    "OneOf",
    "External",
    "Reference",
    "RecordReader",
    "load",
    "iter",
    "dump"
]

# Small addition to the docstring: we want to show a link redirecting to the
# rendered version of the documentation, but this can only work when Python
# is running with docstrings enabled
if __doc__ is not None:
    __doc__ += """See Also:
    An online rendered version of the documentation for this version
    of the library on
    `Read The Docs <https://gb-io.readthedocs.io/en/v{}/>`_.

    """.format(
        __version__
    )
