"""Sequential reviewed batches; each native insertion keeps its own durable receipt.

No batch rollback is claimed. A claimed batch is never replayed, even after an
exception. Reconcile child operation receipts before preparing remaining items.
"""
import hashlib
import json
from pathlib import Path
from uuid import uuid4, UUID

from .live_add_operations import canonical, exclusive_json


class LiveAddBatch:
    def __init__(self, application):
        self.application = application
        self.root = application.operations.root / 'batches'
        self.root.mkdir(exist_ok=True)

    def directory(self, batch_id):
        if str(UUID(batch_id)) != batch_id:
            raise ValueError('Expected canonical batch UUID')
        return self.root / batch_id

    def prepare(self, candidates, save_path):
        candidates = json.loads(canonical(candidates))
        if not 1 <= len(candidates) <= 200:
            raise ValueError('Batch size must be 1-200')
        ids = [c['candidate_id'] for c in candidates]
        if len(set(ids)) != len(ids):
            raise ValueError('Duplicate candidate identity')
        with self.application.lock:
            for candidate in candidates:
                self.application.validate_candidate(candidate)
            first = self.application.prepare(candidates[0], save_path)
            if first['count_before'] + len(candidates) > 400:
                self.application.cancel(first['operation_id'])
                raise ValueError('Insufficient scroll capacity')
            batch_id = str(uuid4())
            plan = {'batch_id': batch_id, 'candidates': candidates,
                    'save_path': str(Path(save_path).resolve()), 'first': first}
            digest = hashlib.sha256(canonical(plan)).hexdigest()
            directory = self.directory(batch_id)
            directory.mkdir(exist_ok=False)
            exclusive_json(directory / 'plan.json', {'plan': plan, 'digest': digest})
            return {'batch_id': batch_id, 'plan_digest': digest, 'count': len(candidates), 'state': 'prepared'}

    def execute(self, batch_id, plan_digest, *, cancelled=lambda: False, progress=lambda value: None):
        directory = self.directory(batch_id)
        value = json.loads((directory / 'plan.json').read_bytes())
        if value['digest'] != plan_digest or hashlib.sha256(canonical(value['plan'])).hexdigest() != plan_digest:
            raise ValueError('Batch review digest differs')
        with self.application.lock:
            # Exclusive durable claim is the cross-process replay guard.
            exclusive_json(directory / 'claim.json', {'digest': plan_digest})
            results, previous = [], None
            progress({'completed': 0, 'total': len(value['plan']['candidates'])})
            for index, candidate in enumerate(value['plan']['candidates']):
                if cancelled():
                    if index == 0:
                        self.application.cancel(value['plan']['first']['operation_id'])
                    break
                child = value['plan']['first'] if index == 0 else self.application.prepare(
                    candidate, value['plan']['save_path'], previous_operation_id=previous)
                exclusive_json(directory / f'child-{index:03}.json', child)
                result = self.application.execute(child['operation_id'], child['plan_digest'])
                results.append(result)
                progress({'completed': sum(r['state'] == 'verified' for r in results), 'total': len(value['plan']['candidates'])})
                if result['state'] != 'verified':
                    break
                previous = child['operation_id']
            complete = len(results) == len(value['plan']['candidates']) and all(r['state'] == 'verified' for r in results)
            receipt = {'batch_id': batch_id, 'state': 'complete' if complete else 'partial',
                       'verified_count': sum(r['state'] == 'verified' for r in results), 'results': results}
            exclusive_json(directory / 'receipt.json', receipt)
            return receipt

    def cancel(self, batch_id):
        directory = self.directory(batch_id)
        with self.application.lock:
            value = json.loads((directory / 'plan.json').read_bytes())
            exclusive_json(directory / 'claim.json', {'digest': value['digest'], 'cancelled': True})
            self.application.cancel(value['plan']['first']['operation_id'])
            exclusive_json(directory / 'receipt.json', {'batch_id': batch_id, 'state': 'cancelled', 'verified_count': 0, 'results': []})
            return self.status(batch_id)

    def status(self, batch_id):
        # Execute holds this same lock. A claimed batch without a final receipt
        # is settled only from durable child receipts after execution has ended.
        # Reading status never replays an insertion or cancels a child.
        with self.application.lock:
            directory = self.directory(batch_id)
            plan = json.loads((directory / 'plan.json').read_bytes())
            children = [json.loads(path.read_bytes()) for path in sorted(directory.glob('child-*.json'))]
            states = [self.application.status(child['operation_id']) for child in children]
            receipt = json.loads((directory / 'receipt.json').read_bytes()) if (directory / 'receipt.json').exists() else {}
            claimed = (directory / 'claim.json').exists()
            state = receipt.get('state', 'prepared')
            if claimed and not receipt:
                # A child plan can exist before its journal entry is written.
                # No recorded child is insufficient evidence to clear uncertainty.
                if not states or any(child['state'] == 'uncertain' for child in states):
                    state = 'uncertain'
                elif len(states) == len(plan['plan']['candidates']) and all(child['state'] == 'verified' for child in states):
                    state = 'complete'
                else:
                    state = 'partial'
            return {'batch_id': batch_id, 'plan_digest': plan['digest'], 'state': state, 'claimed': claimed,
                    'requested_count': len(plan['plan']['candidates']), 'children': states}
