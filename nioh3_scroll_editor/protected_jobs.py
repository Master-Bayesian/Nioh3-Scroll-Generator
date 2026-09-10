"""Responsive serialized operations whose owner must not be force-terminated."""
from copy import deepcopy
import threading
import time
import uuid


class ProtectedJobs:
    def __init__(self):
        self.lock = threading.RLock()
        self.cancelled = threading.Event()
        self.thread = None
        self.job = None

    def start(self, kind, action, *, cancellable=False):
        with self.lock:
            if self.thread is not None and self.thread.is_alive():
                raise RuntimeError('BUSY: protected operation is still running')
            self.cancelled.clear()
            self.job = {'job_id': uuid.uuid4().hex, 'kind': kind, 'state': 'running', 'sequence': 0,
                        'cancellable': cancellable, 'progress': None, 'result': None, 'error': None}
            initial = deepcopy(self.job)
            def progress(value):
                with self.lock:
                    self.job.update(progress=value, sequence=self.job['sequence'] + 1)
            def run():
                try:
                    result = action(self.cancelled, progress)
                    with self.lock:
                        self.job.update(state='completed', result=result)
                except Exception as error:
                    with self.lock:
                        self.job.update(state='failed', error={'code': getattr(error, 'code', 'OPERATION_FAILED'), 'message': str(error)})
                finally:
                    with self.lock:
                        self.job['sequence'] += 1
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
        return self.thread is None or not self.thread.is_alive()

    def join(self):
        if self.thread is not None:
            self.thread.join()
