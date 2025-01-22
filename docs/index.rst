`gb-io` |Stars|
===============

.. |Stars| image:: https://img.shields.io/github/stars/althonos/gb-io.py.svg?style=social&maxAge=3600&label=Star
   :target: https://github.com/althonos/gb-io.py/stargazers

*A Python interface to* ``gb-io``, *a fast GenBank parser and serializer written in Rust.*

|Actions| |Coverage| |PyPI| |Bioconda| |AUR| |Wheel| |Versions| |Implementations| |License| |Source| |Mirror| |Issues| |Docs| |Changelog| |Downloads|

.. |Actions| image:: https://img.shields.io/github/actions/workflow/status/althonos/gb-io.py/test.yml?branch=main&logo=github&style=flat-square&maxAge=300
   :target: https://github.com/althonos/gb-io.py/actions

.. |Coverage| image:: https://img.shields.io/codecov/c/gh/althonos/gb-io.py?style=flat-square&maxAge=600
   :target: https://codecov.io/gh/althonos/gb-io.py/

.. |PyPI| image:: https://img.shields.io/pypi/v/gb-io.svg?style=flat-square&maxAge=3600
   :target: https://pypi.python.org/pypi/gb-io

.. |Bioconda| image:: https://img.shields.io/conda/vn/bioconda/gb-io?style=flat-square&maxAge=3600
   :target: https://anaconda.org/bioconda/gb-io

.. |AUR| image:: https://img.shields.io/aur/version/python-gb-io?logo=archlinux&style=flat-square&maxAge=3600
   :target: https://aur.archlinux.org/packages/python-gb-io

.. |Wheel| image:: https://img.shields.io/pypi/wheel/gb-io?style=flat-square&maxAge=3600
   :target: https://pypi.org/project/gb-io/#files

.. |Versions| image:: https://img.shields.io/pypi/pyversions/gb-io.svg?style=flat-square&maxAge=3600
   :target: https://pypi.org/project/gb-io/#files

.. |Implementations| image:: https://img.shields.io/pypi/implementation/gb-io.svg?style=flat-square&maxAge=3600&label=impl
   :target: https://pypi.org/project/gb-io/#files

.. |License| image:: https://img.shields.io/badge/license-MIT-blue.svg?style=flat-square&maxAge=3600
   :target: https://choosealicense.com/licenses/mit/

.. |Source| image:: https://img.shields.io/badge/source-GitHub-303030.svg?maxAge=2678400&style=flat-square
   :target: https://github.com/althonos/gb-io.py/

.. |Mirror| image:: https://img.shields.io/badge/mirror-EMBL-009f4d?style=flat-square&maxAge=2678400
   :target: https://git.embl.de/larralde/gb-io.py/

.. |Issues| image:: https://img.shields.io/github/issues/althonos/gb-io.py.svg?style=flat-square&maxAge=600
   :target: https://github.com/althonos/gb-io.py/issues

.. |Docs| image:: https://img.shields.io/readthedocs/gb-io?style=flat-square&maxAge=3600
   :target: http://gb-io.readthedocs.io/en/stable/?badge=stable

.. |Changelog| image:: https://img.shields.io/badge/keep%20a-changelog-8A0707.svg?maxAge=2678400&style=flat-square
   :target: https://github.com/althonos/gb-io.py/blob/main/CHANGELOG.md

.. |Downloads| image:: https://img.shields.io/pypi/dm/gb-io?style=flat-square&color=303f9f&maxAge=86400&label=downloads
   :target: https://pepy.tech/project/gb-io


Overview
--------

``gb-io.py`` is a Python package that provides an interface to `gb-io`, a very
fast GenBank format parser and serializer implemented in Rust by `David Leslie <https://github.com/dlesl>`_. 
It can reach much higher speed than the `Biopython <http://biopython.org/>`_ or
the `scikit-bio <http://scikit-bio.org/>`_ parsers.

This library has no external dependency and is available for all modern Python
versions (3.7+).


Setup
-----

Run ``pip install gb-io`` in a shell to download the latest release from PyPI,
or have a look at the :doc:`Installation page <guide/install>` to find other ways 
to install ``gb-io``.


Library
-------

.. toctree::
   :maxdepth: 2

   User Guide <guide/index>
   API Reference <api/index>


Related Projects
----------------

The following Python libraries may be of interest for bioinformaticians.

.. include:: related.rst


License
-------

This library is provided under the `MIT License <https://choosealicense.com/licenses/mit/>`_.
The ``gb-io`` Rust crate package was written by `David Leslie <https://github.com/dlesl>`_
and is licensed under the terms of the `MIT License <https://choosealicense.com/licenses/mit/>`_.
This package may vendor the source of several additional packages that are
licensed under the `Apache-2.0 <https://choosealicense.com/licenses/apache-2.0/>`_,
`MIT <https://choosealicense.com/licenses/mit/>`_ or
`BSD-3-Clause <https://choosealicense.com/licenses/bsd-3-clause/>`_ licenses;
see the license file distributed with the source copy of each vendored
dependency for more information.

*This project is in no way not affiliated, sponsored, or otherwise endorsed
by the* `original authors <https://github.com/dlesl>`_. *It was developed
by* `Martin Larralde <https://github.com/althonos/>`_ *during his PhD project
at the* `European Molecular Biology Laboratory <https://www.embl.de/>`_ *in
the* `Zeller team <https://github.com/zellerlab>`_.
