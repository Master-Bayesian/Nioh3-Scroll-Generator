"""Context-bound paths shared by legacy and V2 measured-map consumers."""
from pathlib import Path


def grace_map_cache_path(state_root: Path, *, save_fingerprint: str, playthrough: int,
                         rarity: int, generation_context_digest: str) -> Path:
    return state_root / 'grace-output-maps' / (
        f'{save_fingerprint.lower()}-{generation_context_digest.lower()[:16]}-'
        f'p{playthrough}-r{rarity}-draw1.json')


def primary_map_cache_path(state_root: Path, *, save_fingerprint: str, playthrough: int,
                           rarity: int, grace_effect_id: int | None,
                           generation_context_digest: str) -> Path:
    kind = 'draw1' if grace_effect_id is None else f'grace-{grace_effect_id:08X}-draw2'
    return state_root / 'primary-effect-maps' / (
        f'{save_fingerprint.lower()}-{generation_context_digest.lower()[:16]}-'
        f'p{playthrough}-r{rarity}-{kind}.json')
