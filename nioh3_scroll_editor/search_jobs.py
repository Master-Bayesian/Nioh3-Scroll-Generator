"""Single-owner offline jobs with bounded storage and page-boundary checkpoints."""
from __future__ import annotations

import base64
from copy import deepcopy
from dataclasses import asdict
import hashlib
import hmac
import json
import secrets
import threading
import time
import uuid

from .core_services import CandidateApplicationService
from .grace_map import load_grace_output_map, grace_map_from_cache_payload
from .search_application import collect_offline_ng3_search_batch, collect_offline_rarity5_search_batch, require_search_candidate_ready
from .seed_accelerator import seed_acceleration_execution_policy
from .scroll_input_metadata import initial_challenge_capacity
from .worker_contracts import RequestError, SearchQuery, candidate_payload, validate_request

TERMINAL = frozenset(('completed', 'cancelled', 'failed'))


class SearchJobs:
    def __init__(self, service=None, collector=collect_offline_ng3_search_batch):
        self.service = service or CandidateApplicationService()
        self.collector = collector
        self.lock = threading.RLock()
        self.cancel_event = threading.Event()
        self.thread = None
        self.job = None
        self.secret = secrets.token_bytes(32)
        self.candidate_records = {}
        self.level = 180
        self.maps = {}

    def register_cache(self, cache_json):
        payload = json.loads(cache_json)
        mapping = grace_map_from_cache_payload(payload, expected_generation_context_digest=self.service.context.context_digest)
        cache_id = hashlib.sha256(cache_json.encode()).hexdigest()
        if cache_id not in self.maps and len(self.maps) >= 16:
            raise ValueError('Measured map registry is full; restart the idle search worker')
        self.maps[cache_id] = mapping
        return {'cache_id': cache_id}

    def _token(self, binding, cursor):
        body = json.dumps({'binding': binding, 'cursor': cursor}, sort_keys=True).encode()
        return base64.urlsafe_b64encode(body).decode() + '.' + hmac.new(self.secret, body, hashlib.sha256).hexdigest()

    def _cursor(self, token, binding):
        if token is None:
            return 0
        try:
            encoded, signature = token.split('.')
            body = base64.urlsafe_b64decode(encoded)
            if not hmac.compare_digest(signature, hmac.new(self.secret, body, hashlib.sha256).hexdigest()):
                raise ValueError('signature')
            payload = json.loads(body)
            if payload['binding'] != binding or type(payload['cursor']) is not int or payload['cursor'] < 0:
                raise ValueError('binding')
            return payload['cursor']
        except (ValueError, KeyError, TypeError) as error:
            raise RequestError('INVALID_RESUME_TOKEN', 'Resume token belongs to a different query, context, execution policy, or worker session') from error

    def start(self, params):
        validate_request({'protocol': 1, 'id': 'application', 'method': 'search.start', 'params': params})
        with self.lock:
            if self.thread is not None and self.thread.is_alive():
                raise RequestError('BUSY', 'One search can run per worker')
            if params['context_digest'] != self.service.context.context_digest:
                raise RequestError('CONTEXT_MISMATCH', 'Refresh the worker handshake before searching')
            query = SearchQuery.from_payload(params['query'])
            cache_id = params.get('cache_id')
            if query.request.playthrough in (4, 5):
                from emaki_exchange import CATEGORY_TO_TYPE
                mapping = self.maps.get(cache_id)
                if query.request.rarity != 5 or mapping is None or mapping.record_type != CATEGORY_TO_TYPE[query.request.playthrough] or mapping.rarity != 5:
                    raise ValueError('NG4/5 offline search requires an exact save-bound rarity-5 map')
            elif cache_id is not None:
                raise ValueError('NG3 uses its certified bundled map')
            binding = f'{query.digest}:{params["context_digest"]}:{params["allow_cpu_fallback"]}:{cache_id}'
            cursor = self._cursor(params['resume_token'], binding)
            self.cancel_event.clear()
            self.candidate_records = {}
            self.level = query.level
            self.job = {
                'job_id': str(uuid.uuid4()), 'state': 'queued', 'sequence': 0,
                'query_digest': query.digest, 'context_digest': params['context_digest'],
                'cursor': cursor, 'start_cursor': cursor, 'candidates': [],
                'progress': None, 'stop_reason': None, 'error': None,
                'resume_token': None, 'elapsed_ms': 0,
            }
            result = deepcopy(self.job)
            self.thread = threading.Thread(target=self._run, args=(query, dict(params), binding), name='offline-search', daemon=False)
            self.thread.start()
            return result

    def snapshot(self, job_id):
        with self.lock:
            if self.job is None or self.job['job_id'] != job_id:
                raise RequestError('JOB_NOT_FOUND', 'Job is unavailable; only the latest job is retained')
            return deepcopy(self.job)

    def current(self):
        with self.lock:
            return {'job': deepcopy(self.job)}

    def export(self, job_id, candidate_id):
        from .candidate_transfer import export_candidate
        with self.lock:
            self.snapshot(job_id)
            if candidate_id not in self.candidate_records:
                raise ValueError('Candidate is no longer retained by this search job')
            return export_candidate(self.candidate_records[candidate_id], self.service.context.context_digest, self.level)

    def cancel(self, job_id):
        with self.lock:
            self.snapshot(job_id)
            if self.job['state'] not in TERMINAL:
                self.cancel_event.set()
                self.job.update(state='cancel_requested', sequence=self.job['sequence'] + 1)
            return deepcopy(self.job)

    def shutdown(self):
        with self.lock:
            if self.job is not None and self.job['state'] not in TERMINAL:
                self.cancel(self.job['job_id'])
        if self.thread is not None:
            self.thread.join()

    def _run(self, query, params, binding):
        started = time.monotonic()
        def progress(report):
            with self.lock:
                self.job.update(progress=asdict(report), sequence=self.job['sequence'] + 1,
                                elapsed_ms=round((time.monotonic() - started) * 1000))
        try:
            with self.lock:
                if not self.cancel_event.is_set():
                    self.job.update(state='running', sequence=self.job['sequence'] + 1)
                cursor = self.job['cursor']
            stop = cursor + params['job_trials']
            mapping = self.maps[params['cache_id']] if query.request.playthrough in (4, 5) else (load_grace_output_map(rarity=query.request.rarity) if query.request.rarity in (4, 5) else None)
            collector = collect_offline_rarity5_search_batch if query.request.playthrough in (4, 5) else self.collector
            reason = 'budget_reached'
            with seed_acceleration_execution_policy(allow_bulk_cpu=params['allow_cpu_fallback']):
                while cursor < stop:
                    if self.cancel_event.is_set():
                        reason = 'cancelled'
                        break
                    page = collector(
                        query.request, grace_mapping=mapping, level=query.level,
                        result_count=params['result_count'] - len(self.job['candidates']),
                        max_trials_per_batch=min(params['page_trials'], stop - cursor),
                        start_after_trial=cursor, intersection_progress=progress,
                        cancelled=self.cancel_event.is_set,
                        allow_cpu_fallback=params['allow_cpu_fallback'],
                    )
                    remaining = params['result_count'] - len(self.job['candidates'])
                    if len(page.candidates) > remaining:
                        raise RequestError('RESULT_OVERFLOW', 'Solver exceeded the requested result limit')
                    # Filter only the completed candidates of this bounded page. The
                    # collector checkpoint still covers rejected candidates, so a
                    # selective filter cannot replay or skip solver trials.
                    ready = [require_search_candidate_ready(c) for c in page.candidates]
                    accepted = [c for c in ready if not query.initial_challenge_counts or
                                initial_challenge_capacity(c.seed) in query.initial_challenge_counts]
                    if query.grace_effect_ids:
                        accepted = [c for c in accepted if c.grace is not None and c.grace.effect_id in query.grace_effect_ids]
                    if query.grouped_rolls:
                        thresholds = dict(query.grouped_rolls)
                        accepted = [c for c in accepted if all(
                            any(e.effect_id in group and e.roll_percent is not None and
                                e.roll_percent >= thresholds.get(e.effect_id, 0)
                                for e in c.effects[0 if not query.request.primary_effect_ids else 1:])
                            for group in query.request.required_secondary_id_groups)]
                    if query.effect_occurrences:
                        from .effect_occurrences import matches_occurrences
                        accepted = [c for c in accepted if matches_occurrences(c.effects, query.effect_occurrences)]
                    payloads = [candidate_payload(c, self.service) for c in accepted]
                    next_cursor = page.next_start_after_trial
                    if next_cursor is None or next_cursor < cursor or next_cursor > min(stop, cursor + params['page_trials']):
                        raise RequestError('INVALID_CHECKPOINT', 'Solver returned an invalid page cursor')
                    with self.lock:
                        if len(self.job['candidates']) + len(payloads) > params['result_count']:
                            raise RequestError('RESULT_OVERFLOW', 'Solver exceeded the requested result limit')
                        self.job['candidates'].extend(payloads)
                        self.candidate_records.update({payload['candidate_id']: candidate for payload, candidate in zip(payloads, accepted)})
                        self.job.update(cursor=next_cursor, sequence=self.job['sequence'] + 1)
                    previous, cursor = cursor, next_cursor
                    if self.cancel_event.is_set():
                        reason = 'cancelled'
                        break
                    if len(self.job['candidates']) >= params['result_count']:
                        reason = 'result_limit'
                        break
                    if page.intersection_report is not None and page.intersection_report.exhausted_family:
                        reason = 'family_exhausted'
                        break
                    if cursor == previous:
                        raise RequestError('NO_PROGRESS', 'Solver stopped without an exhaustion checkpoint')
            with self.lock:
                if self.cancel_event.is_set():
                    reason = 'cancelled'
                self.job.update(state='cancelled' if reason == 'cancelled' else 'completed', stop_reason=reason,
                                resume_token=None if reason == 'family_exhausted' else self._token(binding, cursor))
        except Exception as error:
            with self.lock:
                self.job.update(state='failed', stop_reason='error', resume_token=None,
                                error={'code': getattr(error, 'code', 'SEARCH_FAILED'), 'message': str(error)})
        finally:
            with self.lock:
                self.job.update(sequence=self.job['sequence'] + 1, elapsed_ms=round((time.monotonic() - started) * 1000))
