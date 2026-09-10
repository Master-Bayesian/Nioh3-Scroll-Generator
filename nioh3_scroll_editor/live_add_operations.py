"""Durable at-most-once dispatch ownership, independent of the CE transport.

An interrupted dispatched operation is uncertain. Reading its receipt is safe;
replaying it is not. The executor must reconcile inventory and native index
before publishing success. No game addresses or numerical generation live here.
"""
from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path
from uuid import UUID


def canonical(value):
    return json.dumps(value, sort_keys=True, separators=(',', ':'), allow_nan=False).encode('utf-8')


def exclusive_json(path, value):
    with path.open('xb') as stream:
        stream.write(canonical(value))
        stream.flush()
        os.fsync(stream.fileno())


class LiveAddOperations:
    def __init__(self, root):
        self.root = Path(root)
        self.root.mkdir(parents=True, exist_ok=True)

    def directory(self, operation_id):
        if str(UUID(operation_id)) != operation_id:
            raise ValueError('Use a canonical operation UUID')
        return self.root / operation_id

    def prepare(self, operation_id, plan):
        directory = self.directory(operation_id)
        if plan.get('operation_id') != operation_id:
            raise ValueError('Plan operation identity differs')
        directory.mkdir(exist_ok=False)
        digest = hashlib.sha256(canonical(plan)).hexdigest()
        exclusive_json(directory / 'plan.json', {'digest': digest, 'plan': plan})
        return self.snapshot(operation_id)

    def plan(self, operation_id):
        value = json.loads((self.directory(operation_id) / 'plan.json').read_bytes())
        if hashlib.sha256(canonical(value['plan'])).hexdigest() != value['digest']:
            raise ValueError('Stored plan content changed')
        return value

    def claim(self, operation_id, expected_digest):
        value = self.plan(operation_id)
        if value['digest'] != expected_digest:
            raise ValueError('Reviewed plan digest differs')
        # Exclusive create is the cross-process arbiter. Cancellation claims the
        # same file, so it cannot race with dispatch and report false cancellation.
        exclusive_json(self.directory(operation_id) / 'claim.json',
                       {'action': 'dispatch', 'digest': expected_digest})
        return value['plan']

    def cancel(self, operation_id):
        value = self.plan(operation_id)
        exclusive_json(self.directory(operation_id) / 'claim.json',
                       {'action': 'cancel', 'digest': value['digest']})
        return self.snapshot(operation_id)

    def complete(self, operation_id, receipt):
        directory = self.directory(operation_id)
        claim = json.loads((directory / 'claim.json').read_bytes())
        if claim['action'] != 'dispatch' or claim['digest'] != self.plan(operation_id)['digest']:
            raise ValueError('No matching dispatch claim')
        if receipt.get('operation_id') != operation_id or receipt.get('state') not in ('verified', 'rejected_before_dispatch'):
            raise ValueError('Receipt identity or terminal state differs')
        if receipt['state'] == 'verified' and not all(receipt.get(key) is True for key in (
                'full_container_and_native_index_verified', 'dispatch_and_cleanup_verified')):
            raise ValueError('Successful receipt lacks independent verification')
        if receipt['state'] == 'rejected_before_dispatch' and receipt.get('redirect_count') != 0:
            raise ValueError('Rejection does not establish absence of dispatch')
        exclusive_json(directory / 'receipt.json', {'digest': hashlib.sha256(canonical(receipt)).hexdigest(), 'receipt': receipt})
        return self.snapshot(operation_id)

    def snapshot(self, operation_id):
        directory = self.directory(operation_id)
        value = self.plan(operation_id)
        claim_path, receipt_path = directory / 'claim.json', directory / 'receipt.json'
        state, receipt = 'prepared', None
        if claim_path.exists():
            claim = json.loads(claim_path.read_bytes())
            if claim['digest'] != value['digest'] or claim['action'] not in ('cancel', 'dispatch'):
                raise ValueError('Invalid operation claim')
            state = 'cancelled' if claim['action'] == 'cancel' else 'uncertain'
        if receipt_path.exists():
            envelope = json.loads(receipt_path.read_bytes())
            receipt = envelope['receipt']
            if envelope['digest'] != hashlib.sha256(canonical(receipt)).hexdigest():
                raise ValueError('Stored operation receipt changed')
            if state != 'uncertain' or receipt.get('operation_id') != operation_id:
                raise ValueError('Receipt has no matching dispatch')
            state = receipt['state']
        return {'operation_id': operation_id, 'plan_digest': value['digest'], 'state': state,
                'can_dispatch': state == 'prepared', 'can_cancel': state == 'prepared', 'receipt': receipt}
