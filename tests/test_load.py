import unittest
import os

import gb_io

DATA_FOLDER = os.path.realpath(os.path.join(__file__, os.path.pardir, "data"))


class TestLoad(unittest.TestCase):

    def test_load_binary_file(self):
        path = os.path.join(DATA_FOLDER, "AY048670.1.gb")
        with open(path, "rb") as f:
            records = gb_io.load(f)

    def test_load_text_file(self):
        path = os.path.join(DATA_FOLDER, "AY048670.1.gb")
        with open(path, "rb") as f:
            records = gb_io.load(f)

    def test_load_path(self):
        path = os.path.join(DATA_FOLDER, "AY048670.1.gb")
        records = gb_io.load(path)


class TestLoadError(unittest.TestCase):

    def test_load_directory(self):
        dirname = os.path.dirname(__file__)
        self.assertRaises(OSError, gb_io.load, dirname)

    def test_load_file_not_found(self):
        path = "really/not/a/file/in/there"
        self.assertRaises(OSError, gb_io.load, path)

    def test_load_type_error(self):
        self.assertRaises(TypeError, gb_io.load, 1)
        self.assertRaises(TypeError, gb_io.load, [])

    def test_load_file_syntax_error(self):

        class Reader(object):
            def read(self, n):
                return b"LOCUS"

        r = Reader()
        self.assertRaises(ValueError, gb_io.load, r)

    def test_load_error_propagation(self):

        class MyError(ValueError):
            pass

        class Reader(object):
            def __init__(self):
                self.called = 0
            def read(self, n):
                if self.called == 0:
                    self.called += 1
                    return b"LOCUS"
                raise MyError("my error")

        r = Reader()
        self.assertRaises(MyError, gb_io.load, r)
