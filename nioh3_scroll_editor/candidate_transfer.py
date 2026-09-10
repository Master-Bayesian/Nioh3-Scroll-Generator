"""Broker-only candidate transfer. Never expose this payload to a renderer API."""
from dataclasses import asdict

from .core_services import candidate_identity
from .models import CandidateRecordStage, ScrollCandidate, ScrollEffect


def export_candidate(candidate: ScrollCandidate, context_digest: str, level: int) -> dict:
    return {
        'candidate_id': candidate_identity(candidate, context_digest),
        'context_digest': context_digest, 'level': level,
        'seed': candidate.seed, 'playthrough': candidate.playthrough, 'rarity': candidate.rarity,
        'record_stage': candidate.record_stage.value, 'record_hex': candidate.record.hex(),
        'installation_record_hex': candidate.installation_record.hex() if candidate.installation_record else None,
        'effects': [asdict(effect) for effect in candidate.effects],
    }


def import_candidate(payload: dict, context_digest: str) -> ScrollCandidate:
    if payload['context_digest'] != context_digest:
        raise ValueError('Candidate generation context has changed')
    candidate = ScrollCandidate(
        seed=payload['seed'], playthrough=payload['playthrough'], rarity=payload['rarity'],
        record=bytes.fromhex(payload['record_hex']),
        installation_record=bytes.fromhex(payload['installation_record_hex']) if payload['installation_record_hex'] else None,
        record_stage=CandidateRecordStage(payload['record_stage']),
        effects=tuple(ScrollEffect(**effect) for effect in payload['effects']),
    )
    if candidate_identity(candidate, context_digest) != payload['candidate_id']:
        raise ValueError('Candidate identity does not match the transferred payload')
    return candidate
