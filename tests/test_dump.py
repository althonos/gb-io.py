import datetime
import unittest
import os
import tempfile
import io

import gb_io

DATA_FOLDER = os.path.realpath(os.path.join(__file__, os.path.pardir, "data"))


class TestDump(unittest.TestCase):

    @classmethod
    def setUpClass(cls):
        path = os.path.join(DATA_FOLDER, "AY048670.1.gb")
        cls.records = gb_io.load(path)
        with open(path) as src:
             cls.contents = src.read()

    def test_dump_single_record(self):
        buffer = io.BytesIO()
        gb_io.dump(self.records[0], buffer)
        lines_actual = buffer.getvalue().strip().decode().splitlines()
        lines_expected = self.contents.strip().splitlines()
        self.assertMultiLineEqual("\n".join(lines_actual[1:]), "\n".join(lines_expected[1:]))

    def test_dump_binary_file(self):
        buffer = io.BytesIO()
        gb_io.dump(self.records, buffer)
        lines_actual = buffer.getvalue().strip().decode().splitlines()
        lines_expected = self.contents.strip().splitlines()
        self.assertMultiLineEqual("\n".join(lines_actual[1:]), "\n".join(lines_expected[1:]))

    def test_dump_text_file(self):
        buffer = io.StringIO()
        gb_io.dump(self.records, buffer)
        lines_actual = buffer.getvalue().strip().splitlines()
        lines_expected = self.contents.strip().splitlines()
        self.assertMultiLineEqual("\n".join(lines_actual[1:]), "\n".join(lines_expected[1:]))

    def test_dump_path(self):
        with tempfile.NamedTemporaryFile(suffix=".gbk", mode="w+") as f:
            gb_io.dump(self.records, f.name)
            lines_actual = f.read().strip().splitlines()
        lines_expected = self.contents.strip().splitlines()
        self.assertMultiLineEqual("\n".join(lines_actual[1:]), "\n".join(lines_expected[1:]))

    def test_python_record(self):
        record = gb_io.Record(
            sequence=b"ATGC",
            name="Test sequence",
            source=gb_io.Source(name="Testus organismae"),
            date=datetime.date(2024, 4, 1),
            features=[
                gb_io.Feature("CDS", gb_io.Range(0, 3), [gb_io.Qualifier("translation", "M")]),
            ]
        )
        with tempfile.NamedTemporaryFile(suffix=".gbk", mode="w+") as f:
            gb_io.dump(record, f.name)
            lines_actual = f.read().strip().splitlines()
        self.assertEqual(
            lines_actual,
            [
                'LOCUS       Test sequence              4 bp            linear UNK 01-APR-2024',
                'SOURCE      Testus organismae',
                'FEATURES             Location/Qualifiers',
                '     CDS             1..3',
                '                     /translation="M"',
                'ORIGIN      ',
                '        1 ATGC',
                '//'
            ]
        )

class TestDumpError(unittest.TestCase):

    @classmethod
    def setUpClass(cls):
        path = os.path.join(DATA_FOLDER, "AY048670.1.gb")
        cls.records = gb_io.load(path)

    def test_dump_directory(self):
        dirname = os.path.dirname(__file__)
        self.assertRaises(OSError, gb_io.dump, self.records, dirname)

    def test_dump_bad_file(self):
        self.assertRaises(TypeError, gb_io.dump, self.records, None)

    def test_dump_none(self):
        buffer = io.BytesIO()
        dirname = os.path.dirname(__file__)
        self.assertRaises(TypeError, gb_io.dump, None, buffer)

    def test_dump_none_list(self):
        buffer = io.BytesIO()
        dirname = os.path.dirname(__file__)
        self.assertRaises(TypeError, gb_io.dump, [None], buffer)
