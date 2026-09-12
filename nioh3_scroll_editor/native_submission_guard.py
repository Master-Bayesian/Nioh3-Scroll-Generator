"""Serialize receipt admission across protected workers sharing a data root."""
from contextlib import contextmanager
import os


@contextmanager
def submission_lock(directory):
    handle = (directory / 'admission.lock').open('a+b')
    acquired = False
    try:
        handle.seek(0, 2)
        if not handle.tell():
            handle.write(b'\0')
            handle.flush()
        handle.seek(0)
        try:
            if os.name == 'nt':
                import msvcrt
                msvcrt.locking(handle.fileno(), msvcrt.LK_NBLCK, 1)
            else:
                import fcntl
                fcntl.flock(handle.fileno(), fcntl.LOCK_EX | fcntl.LOCK_NB)
            acquired = True
        except OSError as error:
            raise RuntimeError('Another native executor is admitting an operation') from error
        yield
    finally:
        if acquired:
            handle.seek(0)
            if os.name == 'nt':
                import msvcrt
                msvcrt.locking(handle.fileno(), msvcrt.LK_UNLCK, 1)
            else:
                import fcntl
                fcntl.flock(handle.fileno(), fcntl.LOCK_UN)
        handle.close()
