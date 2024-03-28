import unittest
import os

import gb_io


class TestLocation(unittest.TestCase):

    def test_strand(self):
        location = gb_io.Range(1, 2)
        self.assertEqual(location.strand, "+")

        location = gb_io.Complement(location)
        self.assertEqual(location.strand, "-")