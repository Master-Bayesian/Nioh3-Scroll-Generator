import unittest
from types import SimpleNamespace
from nioh3_scroll_editor.effect_occurrences import matches_occurrences

def effect(key, roll):
    return SimpleNamespace(effect_id=key, roll_percent=roll)

def requirement(*keys, roll=0, scope='secondary'):
    return {'scope': scope, 'alternatives': [{'effect_id': key, 'minimum_roll_percent': roll} for key in keys]}

class OccurrenceTests(unittest.TestCase):
    def test_duplicate_requirements_need_distinct_slots_and_individual_thresholds(self):
        requirements = [requirement(10, roll=90), requirement(10, roll=80)]
        self.assertFalse(matches_occurrences([effect(1,100),effect(10,100)],requirements))
        self.assertTrue(matches_occurrences([effect(1,100),effect(10,80),effect(10,95)],requirements))
        self.assertFalse(matches_occurrences([effect(1,100),effect(10,80),effect(10,85)],requirements))

    def test_assignment_backtracks_for_overlapping_alternatives(self):
        self.assertTrue(matches_occurrences([effect(1,100),effect(10,90),effect(11,90)],
            [requirement(10,11),requirement(10)]))
        self.assertFalse(matches_occurrences([effect(10,100),effect(11,90)],
            [requirement(10,scope='primary'),requirement(10)]))
