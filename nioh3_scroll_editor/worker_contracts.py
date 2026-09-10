"""Versioned transport DTOs; numeric generation remains in the existing core."""
from __future__ import annotations

from dataclasses import asdict, dataclass
import hashlib
import json
from pathlib import Path
import sys
from typing import Any

from jsonschema import Draft7Validator

from .auxiliary_generation import AuxiliarySearchCriteria
from .core_services import CandidateApplicationService
from .effect_seed_solver import EffectSeedRequest, validate_effect_request_feasibility
from .models import ScrollCandidate
from .scroll_input_metadata import initial_challenge_capacity

PROTOCOL_VERSION = 1
MAX_FRAME_BYTES = 4 * 1024 * 1024
SCHEMA_PATH = Path(getattr(sys, '_MEIPASS', Path(__file__).resolve().parents[1])) / 'packages/contracts/request.schema.json'
REQUEST_SCHEMA = json.loads(SCHEMA_PATH.read_text(encoding='utf-8'))
VALIDATOR = Draft7Validator(REQUEST_SCHEMA)
RESPONSE_SCHEMA_PATH = SCHEMA_PATH.with_name('response.schema.json')
CONTRACT_DIGEST = hashlib.sha256(SCHEMA_PATH.read_bytes() + RESPONSE_SCHEMA_PATH.read_bytes()).hexdigest()


class RequestError(ValueError):
    def __init__(self, code: str, message: str):
        super().__init__(message)
        self.code = code


def validate_request(payload: Any) -> dict:
    if not isinstance(payload, dict) or payload.get('protocol') != PROTOCOL_VERSION:
        raise RequestError('PROTOCOL_MISMATCH', 'Expected protocol version 1')
    if not VALIDATOR.is_valid(payload):
        raise RequestError('INVALID_REQUEST', 'Request does not match the versioned contract')
    return payload


@dataclass(frozen=True)
class SearchQuery:
    request: EffectSeedRequest
    level: int
    digest: str
    initial_challenge_counts: tuple[int, ...] = ()
    grace_effect_ids: tuple[int, ...] = ()
    grouped_rolls: tuple[tuple[int, int], ...] = ()
    effect_occurrences: tuple[dict, ...] = ()

    @classmethod
    def from_payload(cls, payload: dict) -> SearchQuery:
        from .catalog_application import resolve_terrain_selections
        auxiliary = AuxiliarySearchCriteria(**{
            key: tuple(frozenset(group) for group in value) if key.endswith('_groups') else frozenset(value)
            for key, value in payload['auxiliary'].items()
        }, terrain_row_indices=resolve_terrain_selections(payload.get('terrain_selection_ids', [])))
        if len({pair[0] for pair in payload['minimum_roll_percent_by_effect_id']}) != len(payload['minimum_roll_percent_by_effect_id']):
            raise ValueError('Roll constraints must contain unique effect IDs')
        grouped_ids = set().union(*map(set, payload['required_secondary_id_groups']))
        grouped_rolls = tuple(tuple(pair) for pair in payload['minimum_roll_percent_by_effect_id'] if pair[0] in grouped_ids)
        request = EffectSeedRequest(
            playthrough=payload['playthrough'], rarity=payload['rarity'],
            primary_effect_ids=frozenset(payload['primary_effect_ids']),
            required_secondary_ids=frozenset(payload['required_secondary_ids']),
            required_secondary_id_groups=tuple(frozenset(group) for group in payload['required_secondary_id_groups']),
            grace_effect_id=payload['grace_effect_id'], auxiliary_criteria=auxiliary,
            minimum_roll_percent_by_effect_id=tuple(tuple(pair) for pair in payload['minimum_roll_percent_by_effect_id'] if pair[0] not in grouped_ids),
        )
        grace_ids = tuple(payload.get('grace_effect_ids', ()))
        if grace_ids:
            from .catalog import R4_FINAL_GRACE_IDS
            from .grace_map import load_grace_output_map
            allowed = R4_FINAL_GRACE_IDS if request.rarity == 4 else ({row.grace_id for row in load_grace_output_map(rarity=5).ranges} if request.rarity == 5 else set())
            if not set(grace_ids).issubset(allowed):
                raise ValueError('Grace choices do not belong to this rarity')
            if request.grace_effect_id is not None and request.grace_effect_id not in grace_ids:
                raise ValueError('Conflicting grace filters')
        validate_effect_request_feasibility(request)
        digest = hashlib.sha256(json.dumps(payload, sort_keys=True, separators=(',', ':')).encode()).hexdigest()
        return cls(request, payload['level'], digest, tuple(payload.get('initial_challenge_counts', ())), grace_ids, grouped_rolls, tuple(payload.get('effect_occurrences', ())))


def candidate_payload(candidate: ScrollCandidate, service: CandidateApplicationService, *, evidence='certified_offline_replay') -> dict:
    preview = asdict(service.preview(candidate))
    preview['record_stage'] = candidate.record_stage.value
    auxiliary = candidate.auxiliary
    return {
        **preview,
        'effects': [asdict(effect) for effect in candidate.effects],
        'auxiliary': {
            'terrain': {'value': auxiliary.terrain.value, 'display_effect_keys': list(auxiliary.terrain.display_effect_keys)},
            'enemy_groups': [[{'lookup_key': entry.lookup_key, 'role': entry.role} for entry in group.entries] for group in auxiliary.enemies.groups],
            'special_rules': [{key: getattr(entry, key) for key in ('key', 'raw_value', 'display_value', 'display_unit', 'display_grade', 'qualifier_kind', 'qualifier_key')} for entry in auxiliary.special_rules.entries],
        } if auxiliary is not None else None,
        'cursor': candidate.joint_search_trial,
        'evidence': evidence,
        'installation_available': preview['installable'],
        'initial_challenge_capacity': initial_challenge_capacity(candidate.seed),
    }
