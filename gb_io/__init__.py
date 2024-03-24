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