from __future__ import annotations

from collections import OrderedDict, deque
from threading import Lock


MAX_PREVIEW_CANDIDATES = 200


class SearchEventBuffer:
    """Bound worker-to-UI search traffic and coalesce noisy progress updates."""

    def __init__(self, *, hard_candidate_limit: int = MAX_PREVIEW_CANDIDATES) -> None:
        if hard_candidate_limit <= 0:
            raise ValueError("hard_candidate_limit must be positive")
        self._hard_candidate_limit = hard_candidate_limit
        self._lock = Lock()
        self._run_id = 0
        self._candidate_limit = 0
        self._accepted_candidates = 0
        self._cancelled = False
        self._candidates: deque[tuple[str, object]] = deque()
        self._progress: OrderedDict[str, object] = OrderedDict()
        self._terminal: deque[tuple[str, object]] = deque()

    def begin(self, requested_candidate_limit: int) -> int:
        if requested_candidate_limit <= 0:
            raise ValueError("requested_candidate_limit must be positive")
        with self._lock:
            self._run_id += 1
            self._candidate_limit = min(
                requested_candidate_limit,
                self._hard_candidate_limit,
            )
            self._accepted_candidates = 0
            self._cancelled = False
            self._candidates.clear()
            self._progress.clear()
            self._terminal.clear()
            return self._run_id

    def publish_candidate(self, run_id: int, candidate: object) -> bool:
        with self._lock:
            if (
                run_id != self._run_id
                or self._cancelled
                or self._accepted_candidates >= self._candidate_limit
            ):
                return False
            self._accepted_candidates += 1
            self._candidates.append(("candidate_found", candidate))
            return True

    def publish_progress(self, run_id: int, event: str, payload: object) -> bool:
        with self._lock:
            if run_id != self._run_id or self._cancelled:
                return False
            self._progress[event] = payload
            return True

    def publish_terminal(self, run_id: int, event: str, payload: object) -> bool:
        with self._lock:
            if run_id != self._run_id:
                return False
            self._terminal.append((event, payload))
            return True

    def cancel(self, run_id: int) -> bool:
        """Cancel one run and discard only its not-yet-rendered UI traffic."""

        with self._lock:
            if run_id != self._run_id:
                return False
            self._cancelled = True
            self._candidates.clear()
            self._progress.clear()
            return True

    def is_cancelled(self, run_id: int) -> bool:
        with self._lock:
            return run_id != self._run_id or self._cancelled

    def drain(self, *, max_events: int) -> tuple[tuple[str, object], ...]:
        if max_events <= 0:
            raise ValueError("max_events must be positive")
        drained: list[tuple[str, object]] = []
        with self._lock:
            while self._candidates and len(drained) < max_events:
                drained.append(self._candidates.popleft())

            while (
                not self._candidates
                and self._progress
                and len(drained) < max_events
            ):
                event, payload = self._progress.popitem(last=False)
                drained.append((event, payload))

            while (
                not self._candidates
                and not self._progress
                and self._terminal
                and len(drained) < max_events
            ):
                drained.append(self._terminal.popleft())
        return tuple(drained)

    def has_pending(self) -> bool:
        with self._lock:
            return bool(self._candidates or self._progress or self._terminal)

    @property
    def accepted_candidate_count(self) -> int:
        with self._lock:
            return self._accepted_candidates
