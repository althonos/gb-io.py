Installation
============

.. note::

    Wheels are provided for Windows, Linux and OSX x86-64 platforms, as well as 
    Linux and OSX Aarch64 platforms. Other machines will have to build the wheel 
    from the source distribution. Building ``gb-io`` involves compiling 
    Rust code, which requires a Rust compiler to be available.



PyPi
^^^^

``gb-io.py`` is hosted on GitHub, but the easiest way to install it is to download
the latest release from its `PyPi repository <https://pypi.python.org/pypi/gb-io>`_.
It will install all dependencies then install ``gb-io`` either from a wheel if
one is available, or from source after compiling the Rust code :

.. code:: console

	$ pip install --user gb-io


.. Conda
.. ^^^^^

.. Pronto is also available as a `recipe <https://anaconda.org/bioconda/gb-io>`_
.. in the `bioconda <https://bioconda.github.io/>`_ channel. To install, simply
.. use the ``conda`` installer:

.. .. code:: console

..    $ conda install -c bioconda gb-io


.. Arch User Repository
.. ^^^^^^^^^^^^^^^^^^^^

.. A package recipe for Arch Linux can be found in the Arch User Repository
.. under the name `python-gb-io <https://aur.archlinux.org/packages/python-gb-io>`_.
.. It will always match the latest release from PyPI.

.. Steps to install on ArchLinux depend on your `AUR helper <https://wiki.archlinux.org/title/AUR_helpers>`_
.. (``yaourt``, ``aura``, ``yay``, etc.). For ``aura``, you'll need to run:

.. .. code:: console

..     $ aura -A python-gb-io


.. BioArchLinux
.. ^^^^^^^^^^^^

.. The `BioArchLinux <https://bioarchlinux.org>`_ project provides pre-compiled packages
.. based on the AUR recipe. Add the BioArchLinux package repository to ``/etc/pacman.conf``:

.. .. code:: ini

..     \[bioarchlinux\]
..     Server = https://repo.bioarchlinux.org/$arch

.. Then install the latest version of the package and its dependencies with ``pacman``:

.. .. code:: console

..     $ pacman -S python-gb-io


Piwheels
^^^^^^^^

``gb-io`` works on Raspberry Pi computers, and pre-built wheels are compiled 
for `armv7l` platforms on piwheels. Run the following command to install these 
instead of compiling from source:

.. code:: console

   $ pip3 install gb-io --extra-index-url https://www.piwheels.org/simple

Check the `piwheels documentation <https://www.piwheels.org/faq.html>`_ for 
more information.


GitHub + ``pip``
^^^^^^^^^^^^^^^^

If, for any reason, you prefer to download the library from GitHub, you can clone
the repository and install the repository by running (with the admin rights):

.. code:: console

   $ git clone --recursive https://github.com/althonos/gb-io.py
   $ pip install --user ./gb-io.py

.. caution::

    Keep in mind this will install always try to install the latest commit,
    which may not even build, so consider using a versioned release instead.


GitHub + ``setuptools``
^^^^^^^^^^^^^^^^^^^^^^^

If you do not want to use ``pip``, you can still clone the repository and
run the ``setup.py`` file manually, although you will need to install the
build dependencies (mainly `Cython <https://pypi.org/project/cython>`_):

.. code:: console

   $ git clone --recursive https://github.com/althonos/gb-io.py
   $ cd gb-io.py
   $ python setup.py build_ext
   # python setup.py install

.. Danger::

    Installing packages without ``pip`` is strongly discouraged, as they can
    only be uninstalled manually, and may damage your system.
