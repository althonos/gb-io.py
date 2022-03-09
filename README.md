# 🧬🏦 `gb-io.py` [![Stars](https://img.shields.io/github/stars/althonos/gb-io.py.svg?style=social&maxAge=3600&label=Star)](https://github.com/althonos/gb-io.py/stargazers)

*A Python interface to [`gb-io`], a fast [GenBank] parser written in [Rust].*

[`gb-io`]: https://crates.io/crates/gb-io
[GenBank]: https://www.ncbi.nlm.nih.gov/genbank/
[Rust]: https://www.rust-lang.org/

[![Actions](https://img.shields.io/github/workflow/status/althonos/gb-io.py/Test/main?logo=github&style=flat-square&maxAge=300)](https://github.com/althonos/gb-io.py/actions)
[![Coverage](https://img.shields.io/codecov/c/gh/althonos/gb-io.py?style=flat-square&maxAge=3600)](https://codecov.io/gh/althonos/gb-io.py/)
[![License](https://img.shields.io/badge/license-MIT-blue.svg?style=flat-square&maxAge=2678400)](https://choosealicense.com/licenses/mit/)
[![PyPI](https://img.shields.io/pypi/v/gb-io.svg?style=flat-square&maxAge=3600)](https://pypi.org/project/gb-io)
[![Wheel](https://img.shields.io/pypi/wheel/gb-io.svg?style=flat-square&maxAge=3600)](https://pypi.org/project/gb-io/#files)
[![Python Versions](https://img.shields.io/pypi/pyversions/gb-io.svg?style=flat-square&maxAge=3600)](https://pypi.org/project/gb-io/#files)
[![Python Implementations](https://img.shields.io/pypi/implementation/gb-io?style=flat-square&maxAge=3600&label=impl)](https://pypi.org/project/gb-io/#files)
[![Source](https://img.shields.io/badge/source-GitHub-303030.svg?maxAge=2678400&style=flat-square)](https://github.com/althonos/gb-io.py/)
[![Mirror](https://img.shields.io/badge/mirror-EMBL-009f4d?style=flat-square&maxAge=2678400)](https://git.embl.de/larralde/gb-io.py/)
[![GitHub issues](https://img.shields.io/github/issues/althonos/gb-io.py.svg?style=flat-square&maxAge=600)](https://github.com/althonos/gb-io.py/issues)
<!-- [![Docs](https://img.shields.io/readthedocs/gb-io/latest?style=flat-square&maxAge=600)](https://gb-io.readthedocs.io) -->
<!-- [![Changelog](https://img.shields.io/badge/keep%20a-changelog-8A0707.svg?maxAge=2678400&style=flat-square)](https://github.com/althonos/gb-io.py/blob/master/CHANGELOG.md) -->
<!-- [![Downloads](https://img.shields.io/badge/dynamic/json?style=flat-square&color=303f9f&maxAge=86400&label=downloads&query=%24.total_downloads&url=https%3A%2F%2Fapi.pepy.tech%2Fapi%2Fprojects%2Fgb-io)](https://pepy.tech/project/gb-io) -->

## 🗺️ Overview

`gb-io.py` is a Python package that provides an interface to `gb-io`, a very
fast GenBank format parser implemented in Rust. It can reach much higher
speed than the [Biopython](http://biopython.org/) or
the [scikit-bio](http://scikit-bio.org/) parsers.

This library has no external dependency and is available for all modern Python
versions (3.7+).

## 🔧 Installing

Install the `gb-io` package directly from [PyPi](https://pypi.org/project/gb-io)
which hosts pre-compiled wheels that can be installed with `pip`:
```console
$ pip install gb-io
```

Wheels are provided for the following platforms:
- Linux, CPython, x86-64
- Linux, PyPy, x86-64
- Linux, CPython, Aarch64
- MacOS, CPython, x86-64
- MacOS, PyPy, x86-64

Otherwise, the source distribution will be downloaded, and a local copy of
the Rust compiler will be downloaded to build the package, unless it is
already installed on the host machine.

<!-- ## 📖 Documentation

A complete [API reference](https://gb-io.readthedocs.io/en/stable/api.html)
can be found in the [online documentation](https://gb-io.readthedocs.io/),
or directly from the command line using
[`pydoc`](https://docs.python.org/3/library/pydoc.html):
```console
$ pydoc gb_io
``` -->

## 💡 Usage

Use the `gb_io.load` function to obtain a list of all GenBank records in a file:
```python
records = gb_io.load("tests/data/AY048670.1.gb")
```

Reading from a file-like object is supported as well, both in text and
binary mode:
```python
with open("tests/data/AY048670.1.gb") as file:
    records = gb_io.load(file)
```

It is also possible to iterate over each record in the file without having
to load the entirety of the file contents to memory with the `gb_io.iter`
method, which returns an iterator instead of a list:
```python
for record in gb_io.iter("tests/data/AY048670.1.gb"):
    print(record.name, record.sequence[:10])
```

## 📝 Example

The following small script will extract all the CDS features from a GenBank
file, and write them in FASTA format to an output file:
```python
import gb_io

with open("tests/data/AY048670.1.faa", "w") as dst:
    for record in gb_io.iter("tests/data/AY048670.1.gb"):
        for feature in filter(lambda feat: feat.type == "CDS", record.features):
            qualifiers = feature.qualifiers.to_dict()
            print(qualifiers)
            dst.write(">{}\n".format(qualifiers["locus_tag"][0]))
            dst.write("{}\n".format(qualifiers["translation"][0]))
```

Compared to similar implementations using `Bio.SeqIO.parse`, `Bio.GenBank.parse`
and `Bio.GenBank.Scanner.GenBankScanner.parse_cds_features`, the performance is
the following:

|               | `gb_io.iter`  | `GenBankScanner` | `GenBank.parse` | `SeqIO.parse` |
| ------------- | ------------- | ---------------- | --------------- | ------------- |
| Time (s)      | **2.264**     | 7.982            | 15.259          | 19.351        |
| Speed (MiB/s) | **136.5**     | 37.1             | 20.5            | 16.2          |
| Speedup       | **x8.55**     | x2.42            | x1.27           | -             |



## 💭 Feedback

### ⚠️ Issue Tracker

Found a bug ? Have an enhancement request ? Head over to the [GitHub issue
tracker](https://github.com/althonos/gb-io.py/issues) if you need to report
or ask something. If you are filing in on a bug, please include as much
information as you can about the issue, and try to recreate the same bug
in a simple, easily reproducible situation.

### 🏗️ Contributing

Contributions are more than welcome! See
[`CONTRIBUTING.md`](https://github.com/althonos/gb-io.py/blob/main/CONTRIBUTING.md)
for more details.

## ⚖️ License

This library is provided under the [MIT License](https://choosealicense.com/licenses/mit/).
The `gb-io` Rust crate package was written by [David Leslie](https://github.com/dlesl)
and is licensed under the terms of the [MIT License](https://choosealicense.com/licenses/mit/).
This package vendors the source of several additional packages that are
licensed under the [Apache-2.0](https://choosealicense.com/licenses/apache-2.0/),
[MIT](https://choosealicense.com/licenses/mit/) or
[BSD-3-Clause](https://choosealicense.com/licenses/bsd-3-clause/) licenses;
see the license file distributed with the source copy of each vendored
dependency for more information.

*This project is in no way not affiliated, sponsored, or otherwise endorsed
by the [original `gb-io` authors](https://github.com/dlesl). It was developed
by [Martin Larralde](https://github.com/althonos/) during his PhD project
at the [European Molecular Biology Laboratory](https://www.embl.de/) in
the [Zeller team](https://github.com/zellerlab).*
