"""Private stdio worker: uint32-LE byte length followed by UTF-8 JSON.

Only offline search is exposed. This process owns no save writer or game handle.

Identity: a production launch requires the exact installed game file version
(``--game-file-version <A.B.C.D>``) and binds the worker's generation context
and generation tables to that one version. There is no default, no discovery,
and no ``CURRENT`` fallback: a missing, malformed, or unregistered version
fails closed before any job is accepted. ``--legacy-test-context`` is an
explicit, visibly non-production opt-in that reproduces the pre-version identity
for tests and diagnostics only; it never authorizes candidate, cache, or resume
reuse.
"""
from __future__ import annotations

import sys

from .core_services import CoreErrorCode, CoreServiceError
from .worker_transport import read_exact, read_frame, write_frame
from .search_jobs import SearchJobs
from .worker_contracts import CONTRACT_DIGEST, MAX_FRAME_BYTES, RequestError, validate_request


USAGE = (
    "usage: python -m nioh3_scroll_editor.search_worker "
    "(--game-file-version <A.B.C.D> | --legacy-test-context)\n"
    "\n"
    "Serves the offline search surface (handshake, search.catalog,\n"
    "recommended_level.resolve, cache.register, candidate.preview, search.start,\n"
    "job.current, job.snapshot, job.cancel, candidate.export and shutdown).\n"
    "\n"
    "Identity: a production launch requires --game-file-version, the exact\n"
    "installed game executable version (for example 2.0.2.0). There is no default;\n"
    "a missing or unregistered version fails closed with RESOURCE_MISMATCH.\n"
    "--legacy-test-context is an explicit, visibly non-production opt-in that\n"
    "reproduces the pre-version identity for tests and diagnostics only; it never\n"
    "authorizes candidate, cache, or resume reuse."
)


class WorkerStartupError(RuntimeError):
    """A refusal that must stop the process before any frame is served."""








def handshake(jobs):
    from .seed_accelerator import cuda_seed_acceleration_available
    from .effect_preimage_accelerator import d3d11_effect_acceleration_available
    context = jobs.service.context
    return {
        'protocol': 1, 'contract_digest': CONTRACT_DIGEST, 'role': 'offline_search',
        'context': context.to_payload(),
        'capabilities': {
            'playthroughs': [3], 'rarities': [3, 4, 5],
            'cached_rarity5_playthroughs': [4, 5],
            'cuda_pivot_and_auxiliary': cuda_seed_acceleration_available(),
            'directcompute_effect_filter': d3d11_effect_acceleration_available(),
            'cpu_exact_replay': True, 'bulk_cpu_requires_opt_in': True,
            'save_write': False, 'runtime_calls': False,
        },
    }


def resolve_generation_tables():
    """Generation tables for the verified installed build, else ``None``.

    The four part version of the single discovered Nioh 3 executable is the
    offline identity.  When that build owns a versioned resource its tables are
    returned; an unknown or ambiguous installation returns ``None`` so the
    worker keeps the shipped baseline instead of claiming a newer version.

    Discovery is a diagnostics convenience only.  It is never the production
    identity: :func:`main` resolves the version from its explicit argument and
    never calls this entry.
    """

    try:
        from .game_compatibility import _file_version, discover_game_executables
        from .r4_finalizer_resource import VERSION_RESOURCE_ROOTS

        candidates = discover_game_executables()
        if len(candidates) != 1:
            return None
        version = tuple(int(part) for part in _file_version(candidates[0]))
    except Exception:
        return None
    if version not in VERSION_RESOURCE_ROOTS:
        return None
    from .effect_generation_tables import effect_generation_tables_for_game_version

    return effect_generation_tables_for_game_version(version)


def parse_file_version_argument(raw):
    """Parse one exact four-part version, refusing every other spelling.

    Mirrors ``parse_game_file_version`` in ``crates/nioh3-worker/src/main.rs``:
    a value that passes here is never refused downstream for its spelling, and
    anything without exactly four 16-bit parts is refused as a start-up error.
    """

    parts = raw.split(".")
    if len(parts) != 4:
        raise WorkerStartupError(
            f"--game-file-version must be a four-part version such as 2.0.2.0, not {raw}"
        )
    numbers = []
    for part in parts:
        if not part.isdigit():
            raise WorkerStartupError(
                f"--game-file-version part {part} is not a number in {raw}"
            )
        value = int(part)
        if value > 0xFFFF:
            raise WorkerStartupError(
                f"--game-file-version part {part} is not a 16-bit number in {raw}"
            )
        numbers.append(value)
    return (numbers[0], numbers[1], numbers[2], numbers[3])


def parse_options(args):
    """Resolve the explicit identity selection from ``args``, fail closed.

    ``None`` means the caller asked for help.  Exactly one of
    ``--game-file-version`` and ``--legacy-test-context`` is required; neither is
    selected silently and there is no discovered or default version.
    """

    game_file_version = None
    legacy_test_context = False
    index = 0
    while index < len(args):
        argument = args[index]
        if argument == "--help" or argument == "-h":
            return None
        if argument == "--game-file-version":
            index += 1
            if index >= len(args):
                raise WorkerStartupError("--game-file-version needs a value")
            game_file_version = args[index]
        elif argument == "--legacy-test-context":
            legacy_test_context = True
        else:
            raise WorkerStartupError(f"unknown argument: {argument}")
        index += 1

    if game_file_version is not None and legacy_test_context:
        raise WorkerStartupError(
            "refusing to start: pass either --game-file-version or "
            "--legacy-test-context, not both"
        )
    if game_file_version is not None:
        return parse_file_version_argument(game_file_version)
    if legacy_test_context:
        return "legacy"
    raise WorkerStartupError(
        "refusing to start: a production launch requires "
        "--game-file-version <A.B.C.D>, the exact installed game executable "
        "version; --legacy-test-context is the explicit non-production test opt-in"
    )


def resolve_worker_context(file_version):
    """Production identity and generation tables for one explicit version.

    The context and the tables are derived from the same explicit version in one
    pass, so a handshake can never publish tables the identity does not describe.
    An unregistered version fails closed with ``RESOURCE_MISMATCH`` before the
    data root is read.
    """

    from .effect_generation_tables import effect_generation_tables_for_game_version
    from .resolved_context import ResolvedGenerationContext

    if file_version == "legacy":
        from .resolved_context import capture_legacy_context

        return capture_legacy_context(), None
    context = ResolvedGenerationContext.capture(file_version=file_version)
    return context, effect_generation_tables_for_game_version(file_version)


def main(argv=None):
    args = list(sys.argv[1:] if argv is None else argv)
    try:
        selection = parse_options(args)
    except WorkerStartupError as error:
        print(f"{error}\n\n{USAGE}", file=sys.stderr)
        return 2
    if selection is None:
        print(USAGE)
        return 0

    source, sink = sys.stdin.buffer, sys.stdout.buffer
    # Libraries' Python prints are diagnostics, never protocol bytes.
    sys.stdout = sys.stderr
    try:
        context, generation_tables = resolve_worker_context(selection)
    except CoreServiceError as error:
        print(
            f"search worker startup failed: {error.code.value}: {error.message}",
            file=sys.stderr,
        )
        return 1
    jobs = SearchJobs(context=context, generation_tables=generation_tables)
    negotiated = False
    try:
        while True:
            payload = read_frame(source)
            if payload is None:
                break
            request_id = payload.get('id') if isinstance(payload, dict) else None
            shutdown = False
            try:
                validate_request(payload)
                method, params = payload['method'], payload['params']
                if method == 'handshake':
                    result = handshake(jobs)
                    negotiated = True
                elif not negotiated:
                    raise RequestError('HANDSHAKE_REQUIRED', 'Negotiate before sending commands')
                elif method == 'search.catalog':
                    from .catalog import searchable_scroll_effect_definitions, native_effect_name, R4_FINAL_GRACE_IDS
                    from .grace_map import load_grace_output_map
                    rarity, locale = params['rarity'], params['locale']
                    ordinary = searchable_scroll_effect_definitions(3, rarity)
                    mapping = load_grace_output_map(rarity=rarity) if rarity in (4, 5) else None
                    grace_ids = sorted({entry.grace_id for entry in mapping.ranges
                                        if rarity != 4 or entry.grace_id in R4_FINAL_GRACE_IDS}) if mapping is not None else []
                    result = {
                        'context_digest': jobs.service.context.context_digest,
                        'ordinary_effects': [{'effect_id': effect.effect_id, 'name': effect.name if locale == 'zh-CN' else native_effect_name(effect.effect_id, locale) or effect.name} for effect in ordinary],
                        'grace_effects': [{'effect_id': effect_id, 'name': native_effect_name(effect_id, locale) or f'0x{effect_id:04X}'} for effect_id in grace_ids],
                    }
                    from .catalog_application import auxiliary_catalog, recommended_level_metadata
                    result.update(auxiliary_catalog(3, locale))
                    result['recommended_level'] = recommended_level_metadata()
                elif method == 'recommended_level.resolve':
                    from .catalog_application import resolve_recommended_level_payload
                    # JSON Schema integers include integral JSON numbers such as 350.0.
                    result = resolve_recommended_level_payload(int(params['displayed_level']))
                elif method == 'search.start':
                    result = jobs.start(params)
                elif method == 'cache.register':
                    result = jobs.register_cache(params['cache_json'])
                elif method == 'candidate.preview':
                    from dataclasses import replace
                    from .models import ScrollCandidate
                    from .effect_sequence import generate_ng3_certified_effect_sequence
                    from .auxiliary_generation import generate_complete_auxiliary
                    from .candidate_transfer import export_candidate
                    from .worker_contracts import candidate_payload
                    from .search_application import require_search_candidate_ready
                    candidate = ScrollCandidate.from_effect_sequence(generate_ng3_certified_effect_sequence(params['seed'], rarity=params['rarity'], level=params['level'], tables=jobs.generation_tables))
                    candidate = replace(candidate, auxiliary=generate_complete_auxiliary(params['seed'], 3))
                    require_search_candidate_ready(candidate)
                    result = {'candidate': candidate_payload(candidate, jobs.service),
                              'transfer': export_candidate(candidate, jobs.service.context.context_digest, params['level'])}
                elif method == 'candidate.export':
                    result = jobs.export(params['job_id'], params['candidate_id'])
                elif method == 'job.snapshot':
                    result = jobs.snapshot(params['job_id'])
                elif method == 'job.current':
                    result = jobs.current()
                elif method == 'job.cancel':
                    result = jobs.cancel(params['job_id'])
                else:
                    jobs.shutdown()
                    result, shutdown = {'stopped': True}, True
                response = {'protocol': 1, 'id': request_id, 'ok': True, 'result': result}
            except (ValueError, RuntimeError) as error:
                response = {'protocol': 1, 'id': request_id, 'ok': False,
                            'error': {'code': getattr(error, 'code', 'INVALID_REQUEST'), 'message': str(error)}}
            write_frame(sink, response)
            if shutdown:
                break
    finally:
        jobs.shutdown()
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
