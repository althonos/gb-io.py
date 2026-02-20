import copy
import unittest
import os

import gb_io

DATA_FOLDER = os.path.realpath(os.path.join(__file__, os.path.pardir, "data"))


class TestRecord(unittest.TestCase):

    def test_copy(self):

        path = os.path.join(DATA_FOLDER, "AY048670.1.gb")
        with open(path, "rb") as f:
            records = gb_io.load(f)

        record1 = records[0]
        record2 = copy.copy(record1)

        # shallow copy -- new object, but should keep same references
        self.assertIsNot(record2, record1)
        self.assertIs(record2.features, record1.features)
        # self.assertIs(record2.source, record1.source)
        self.assertIs(record2.references, record1.references)
        self.assertIs(record2.sequence, record1.sequence)