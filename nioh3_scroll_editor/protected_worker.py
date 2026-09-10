"""Role-scoped save/runtime host. EOF never discards in-flight ownership."""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import sys
import time

from jsonschema import Draft7Validator
from .app_settings import load_app_settings
from .core_services import CandidateApplicationService
from .protected_jobs import ProtectedJobs
from .worker_transport import read_frame, write_frame

ROOT = Path(getattr(sys, '_MEIPASS', Path(__file__).resolve().parents[1]))
REQUEST_PATH = ROOT / 'packages/contracts/protected-request.schema.json'
RESPONSE_PATH = ROOT / 'packages/contracts/protected-response.schema.json'
VALIDATOR = Draft7Validator(json.loads(REQUEST_PATH.read_text(encoding='utf-8')))
RESPONSE_VALIDATOR = Draft7Validator(json.loads(RESPONSE_PATH.read_text(encoding='utf-8')))
CONTRACT_DIGEST = hashlib.sha256(REQUEST_PATH.read_bytes() + RESPONSE_PATH.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--role', choices=('save', 'runtime'), required=True)
    args = parser.parse_args()
    source, sink = sys.stdin.buffer, sys.stdout.buffer
    sys.stdout = sys.stderr
    service = CandidateApplicationService()
    if args.role == 'save':
        from .save_application import SaveApplication
        application = SaveApplication(Path(os.environ.get('NIOH3_STATE_ROOT') or load_app_settings(fallback_root=ROOT).data_root), service=service)
    else:
        from .runtime_application import RuntimeApplication
        application = RuntimeApplication(service=service)
    jobs = ProtectedJobs()
    negotiated = False
    try:
        while True:
            payload = read_frame(source)
            if payload is None:
                break
            request_id = payload.get('id') if isinstance(payload, dict) else None
            stop = False
            try:
                if not VALIDATOR.is_valid(payload):
                    raise ValueError('INVALID_REQUEST: protected contract validation failed')
                method, params = payload['method'], dict(payload['params'])
                if method == 'handshake':
                    result = {'role': args.role, 'protocol': 1, 'contract_digest': CONTRACT_DIGEST,
                              'context': service.context.to_payload(), 'kill_safe': False}
                    negotiated = True
                elif not negotiated:
                    raise ValueError('HANDSHAKE_REQUIRED')
                elif method == 'shutdown':
                    result = {'safe_to_shutdown': False, 'error': 'Operation still running'}
                    if jobs.idle():
                        result = application.shutdown() if args.role == 'runtime' else {'safe_to_shutdown': True}
                    stop = result['safe_to_shutdown']
                elif method == 'job.snapshot':
                    result = jobs.snapshot(params['job_id'])
                elif method == 'job.current':
                    result = jobs.current()
                elif method == 'job.cancel':
                    result = jobs.cancel(params['job_id'])
                elif not method.startswith(args.role + '.'):
                    raise ValueError('ROLE_MISMATCH')
                elif method == 'runtime.status':
                    result = application.status()
                else:
                    operation = method.split('.', 1)[1]
                    params.pop('title_screen_confirmed', None)
                    def execute(cancelled, progress, operation=operation, params=params):
                        if args.role == 'runtime' and operation in ('generate', 'search', 'capture_grace', 'live_batch_execute'):
                            return getattr(application, operation)(**params, cancelled=cancelled, progress=progress)
                        return getattr(application, operation)(**params)
                    result = jobs.start(method, execute, cancellable=method in ('runtime.generate', 'runtime.search', 'runtime.capture_grace', 'runtime.live_batch_execute'))
                response = {'protocol': 1, 'id': request_id, 'ok': True, 'result': result}
                if not RESPONSE_VALIDATOR.is_valid(response):
                    raise RuntimeError('INVALID_RESULT: operation output does not match the protected contract; query the operation receipt before retrying a write')
            except Exception as error:
                response = {'protocol': 1, 'id': request_id, 'ok': False, 'error': {'code': getattr(error, 'code', 'OPERATION_REJECTED'), 'message': str(error)}}
            write_frame(sink, response)
            if stop:
                break
    finally:
        jobs.cancelled.set()
        jobs.join()
        # A broker crash or closed pipe is not permission to abandon a hook.
        if args.role == 'runtime':
            while not application.shutdown()['safe_to_shutdown']:
                time.sleep(1)
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
