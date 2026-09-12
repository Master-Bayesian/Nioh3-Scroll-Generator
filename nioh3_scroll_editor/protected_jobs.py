"""Responsive serialized operations whose owner must not be force-terminated."""
from copy import deepcopy
import threading
import sys
import traceback
import uuid


class ProtectedJobs:
    def __init__(self):
        self.lock = threading.RLock()
        self.cancelled = threading.Event()
        self.thread = None
        self.job = None

    def _join_terminal_owner(self):
        with self.lock:
            previous = self.thread if self.job and self.job['state'] in ('completed', 'failed') else None
        # Completion can be observed before Thread.is_alive() becomes false.
        # Join outside the lock, without retrying or replacing an active action.
        if previous is not None:
            previous.join()

    def start(self, kind, action, *, cancellable=False):
        self._join_terminal_owner()
        with self.lock:
            if self.thread is not None and self.thread.is_alive():
                raise RuntimeError('BUSY: protected operation is still running')
            self.cancelled.clear()
            self.job = {'job_id': uuid.uuid4().hex, 'kind': kind, 'state': 'running', 'sequence': 0,
                        'cancellable': cancellable, 'progress': None, 'result': None, 'error': None}
            job = self.job
            initial = deepcopy(job)
            def progress(value):
                with self.lock:
                    job.update(progress=value, sequence=job['sequence'] + 1)
            def run():
                try:
                    result = action(self.cancelled, progress)
                    outcome = {'state': 'completed', 'result': result}
                except Exception as error:
                    try:
                        print(f"[protected-failure] job_id={job['job_id']} kind={kind} error={error}", file=sys.stderr)
                        traceback.print_exc(file=sys.stderr)
                    except Exception:
                        pass
                    outcome = {'state': 'failed', 'error': {'code': getattr(error, 'code', 'OPERATION_FAILED'), 'message': str(error)}}
                with self.lock:
                    # Publish all terminal fields together, after action cleanup.
                    job.update(**outcome, sequence=job['sequence'] + 1)
            self.thread = threading.Thread(target=run, name=f'protected-{kind}', daemon=False)
            self.thread.start()
            return initial

    def snapshot(self, job_id):
        with self.lock:
            if self.job is None or self.job['job_id'] != job_id:
                raise ValueError('Unknown operation job')
            return deepcopy(self.job)

    def current(self):
        """Private broker recovery; the broker must hide private job payloads."""
        with self.lock:
            return {'job': deepcopy(self.job)}

    def cancel(self, job_id):
        with self.lock:
            self.snapshot(job_id)
            if not self.job['cancellable']:
                raise ValueError('This operation cannot be cancelled; wait for its commit outcome')
            self.cancelled.set()
            if self.job['state'] == 'running':
                self.job.update(state='cancel_requested', sequence=self.job['sequence'] + 1)
            return deepcopy(self.job)

    def idle(self):
        self._join_terminal_owner()
        with self.lock:
            return self.thread is None or not self.thread.is_alive()

    def join(self):
        if self.thread is not None:
            self.thread.join()
