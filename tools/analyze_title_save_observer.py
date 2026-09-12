"""Read bounded v2.01 CE observations; never infer a product commit from I/O alone."""
from __future__ import annotations
import argparse
from collections import Counter, defaultdict
import json
from pathlib import Path
from typing import Any

MAX_BYTES = 12 * 1024 * 1024
MAX_LINE = 65536
MAX_EVENTS = 4096
SCHEMA = 'nioh3.title-save-observer/v1'


def _list(value: Any) -> list:
    # The CE JSON writer predates an empty-array marker; {} is permitted only
    # for these explicitly declared empty-vector fields, not arbitrary objects.
    if value == {}:
        return []
    if not isinstance(value, list):
        raise ValueError('Expected event vector')
    return value


def read_capture(path: Path) -> tuple[dict, list[dict], dict]:
    if path.stat().st_size > MAX_BYTES:
        raise ValueError('Capture byte limit exceeded')
    rows = []
    with path.open('r', encoding='utf-8') as stream:
        while line := stream.readline(MAX_LINE + 1):
            if len(line.encode('utf-8')) > MAX_LINE or not line.endswith('\n'):
                raise ValueError('Overlong or incomplete capture line')
            value = json.loads(line)
            if not isinstance(value, dict):
                raise ValueError('Capture records must be objects')
            rows.append(value)
            if len(rows) > MAX_EVENTS + 2:
                raise ValueError('Event limit exceeded')
    if len(rows) < 2 or rows[0].get('kind') != 'header' or rows[-1].get('kind') != 'status':
        raise ValueError('Missing header/final status; this is not a complete capture')
    header, events, status = rows[0], rows[1:-1], rows[-1]
    if header.get('schema') != SCHEMA:
        raise ValueError('Unknown observer schema')
    identity = header.get('process', {})
    if not isinstance(identity.get('creation_filetime'), str) or not identity['creation_filetime'].isdigit():
        raise ValueError('Process creation identity missing')
    if type(identity.get('pid')) is not int or identity['pid'] <= 0:
        raise ValueError('Process ID missing')
    if not isinstance(header.get('run_id'), str) or not isinstance(header.get('profile'), str):
        raise ValueError('Run/profile identity missing')
    required = {'active', 'dropped', 'cleanup_pending', 'owned_breakpoints', 'errors', 'sequence', 'epoch'}
    if not required.issubset(status):
        raise ValueError('Final status is incomplete')
    if any(type(status[k]) is not bool for k in ('active', 'cleanup_pending')):
        raise ValueError('Invalid lifecycle flags')
    if any(type(status[k]) is not int or status[k] < 0 for k in ('dropped', 'sequence', 'epoch')):
        raise ValueError('Invalid lifecycle counters')
    _list(status['errors'])
    _list(status['owned_breakpoints'])
    previous = 0
    for event in events:
        if event.get('schema') != SCHEMA or event.get('run_id') != header.get('run_id'):
            raise ValueError('Mixed run/schema')
        process = event.get('process', {})
        if any(process.get(k) != identity.get(k) for k in ('pid', 'creation_filetime')):
            raise ValueError('Process instance changed within capture')
        sequence = event.get('sequence')
        if type(sequence) is not int or sequence <= previous:
            raise ValueError('Event sequence is not strictly increasing')
        previous = sequence
        if type(event.get('thread_id')) is not int or event['thread_id'] <= 0:
            raise ValueError('Missing native thread identity')
        if event.get('profile') != header['profile'] or type(event.get('site')) is not str:
            raise ValueError('Mixed profile or invalid observation site')
        if type(event.get('epoch')) is not int or not 0 <= event['epoch'] <= status['epoch']:
            raise ValueError('Invalid capture epoch')
    if status['sequence'] < previous:
        raise ValueError('Final sequence precedes an event')
    return header, events, status


def analyze(path: Path) -> dict:
    header, events, status = read_capture(path)
    counts: Counter = Counter()
    threads: dict[str, set[int]] = defaultdict(set)
    native_errors, rejected, empty_pending, io = [], [], [], []
    for event in events:
        site = event.get('site', 'unknown')
        counts[site] += 1
        threads[site].add(event['thread_id'])
        if site == 'request_exit' and event.get('disposition') == 'busy_rejected':
            rejected.append(event['sequence'])
        g = event.get('globals', {})
        if g.get('queue_count') == 0 and g.get('task', {}).get('address'):
            empty_pending.append(event['sequence'])
        if site == 'serializer_exit' and event.get('error_code', 0) != 0:
            native_errors.append({'sequence': event['sequence'], 'site': site, 'error': event['error_code']})
        if site in ('coordinator_exit', 'writer_exit', 'read_exit', 'apply_exit') and event.get('return_al') == 0:
            native_errors.append({'sequence': event['sequence'], 'site': site, 'error': event.get('error')})
        if site.startswith('writer_'):
            io.append({key: event[key] for key in ('sequence','site','thread_id','tick_ms','directory',
                                                  'filename','handle','bytes','bytes_written',
                                                  'return_al','return_u32','error','writer_start_sequence','writer_start_origin',
                                                  'previously_observed_handle','handle_state','unpaired') if key in event})
    errors = _list(status.get('errors', []))
    owned = _list(status.get('owned_breakpoints', []))
    cleanup_errors = _list(status.get('cleanup_errors', []))
    complete = bool(events) and not (status.get('active') or status.get('dropped') or
                                     status.get('cleanup_pending') or status.get('continue_failed') or owned or errors or cleanup_errors)
    return {'schema': 'nioh3.title-save-analysis/v1', 'run_id': header['run_id'],
            'profile': header['profile'], 'process': header['process'],
            'capture_complete_within_selected_profile': complete,
            'capture_integrity_only': True, 'native_path_completion_proven': False,
            'site_counts': dict(counts), 'threads_by_site': {k: sorted(v) for k,v in threads.items()},
            'busy_rejections': rejected, 'native_errors': native_errors,
            'observer_errors': errors, 'cleanup_errors': cleanup_errors,
            'empty_queue_with_active_task_events': empty_pending, 'file_chronology': io,
            'native_generation_ack': 'not_observed_or_defined',
            'title_state_binding': 'requires_controlled_run_annotation',
            'product_commit_proven': False, 'release': 'BLOCK',
            'warning': 'A successful file rename or drained snapshot queue is not save ownership.'}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('capture', type=Path)
    parser.add_argument('--output', type=Path)
    args = parser.parse_args()
    result = analyze(args.capture)
    text = json.dumps(result, indent=2) + '\n'
    if args.output:
        with args.output.open('x', encoding='utf-8') as stream:
            stream.write(text)
    else:
        print(text, end='')
    return 0 if result['capture_complete_within_selected_profile'] else 2


if __name__ == '__main__':
    raise SystemExit(main())
