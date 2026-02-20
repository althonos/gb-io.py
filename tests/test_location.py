import copy
import unittest
import os

import gb_io


class TestLocation(unittest.TestCase):

    def test_strand(self):
        location = gb_io.Range(1, 2)
        self.assertEqual(location.strand, "+")

        location = gb_io.Complement(location)
        self.assertEqual(location.strand, "-")


class TestJoin(unittest.TestCase):

    def test_copy(self):

        x1 = gb_io.Range(1, 2)
        x2 = gb_io.Range(3, 4)

        loc1 = gb_io.Join([x1, x2])
        self.assertIs(loc1.locations[0], x1)
        self.assertIs(loc1.locations[1], x2)

        loc2 = copy.copy(loc1)
        self.assertIs(loc2.locations[0], x1)
        self.assertIs(loc2.locations[1], x2)
