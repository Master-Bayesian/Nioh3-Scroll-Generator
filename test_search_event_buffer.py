from __future__ import annotations

import unittest

from nioh3_scroll_editor.search_event_buffer import SearchEventBuffer


class SearchEventBufferTests(unittest.TestCase):
    def test_candidate_stream_is_hard_bounded(self) -> None:
        events = SearchEventBuffer(hard_candidate_limit=3)
        run_id = events.begin(10)

        self.assertTrue(events.publish_candidate(run_id, "a"))
        self.assertTrue(events.publish_candidate(run_id, "b"))
        self.assertTrue(events.publish_candidate(run_id, "c"))
        self.assertFalse(events.publish_candidate(run_id, "d"))
        self.assertEqual(events.accepted_candidate_count, 3)

    def test_progress_is_coalesced_and_candidates_are_rendered_first(self) -> None:
        events = SearchEventBuffer()
        run_id = events.begin(20)
        events.publish_progress(run_id, "progress", 1)
        events.publish_progress(run_id, "progress", 2)
        events.publish_candidate(run_id, "seed")
        events.publish_terminal(run_id, "search_complete", "done")

        self.assertEqual(
            events.drain(max_events=1),
            (("candidate_found", "seed"),),
        )
        self.assertEqual(events.drain(max_events=1), (("progress", 2),))
        self.assertEqual(
            events.drain(max_events=1),
            (("search_complete", "done"),),
        )

    def test_large_progress_burst_retains_only_the_latest_update(self) -> None:
        events = SearchEventBuffer()
        run_id = events.begin(20)

        for value in range(100_000):
            events.publish_progress(run_id, "intersection_progress", value)

        self.assertEqual(
            events.drain(max_events=10),
            (("intersection_progress", 99_999),),
        )
        self.assertFalse(events.has_pending())

    def test_cancel_discards_pending_visual_events_but_accepts_completion(self) -> None:
        events = SearchEventBuffer()
        run_id = events.begin(20)
        events.publish_candidate(run_id, "seed")
        events.publish_progress(run_id, "progress", 1)

        self.assertTrue(events.cancel(run_id))
        self.assertTrue(events.is_cancelled(run_id))
        self.assertFalse(events.publish_candidate(run_id, "late"))
        self.assertTrue(events.publish_terminal(run_id, "search_complete", "done"))
        self.assertEqual(
            events.drain(max_events=10),
            (("search_complete", "done"),),
        )

    def test_stale_run_events_are_rejected(self) -> None:
        events = SearchEventBuffer()
        stale_run_id = events.begin(20)
        current_run_id = events.begin(20)

        self.assertFalse(events.publish_candidate(stale_run_id, "stale"))
        self.assertFalse(events.publish_terminal(stale_run_id, "error", "stale"))
        self.assertTrue(events.publish_candidate(current_run_id, "current"))
        self.assertEqual(
            events.drain(max_events=10),
            (("candidate_found", "current"),),
        )


if __name__ == "__main__":
    unittest.main()
