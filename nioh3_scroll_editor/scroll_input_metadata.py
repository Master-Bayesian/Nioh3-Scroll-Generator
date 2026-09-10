"""Read-only application metadata; retain the frozen generation/display functions."""
from .effect_sequence import generate_challenge_attempt_count
from .recommended_level import predict_recommended_level


def initial_challenge_capacity(seed: int) -> int:
    return generate_challenge_attempt_count(seed)


def record_input_metadata(record: bytes, header) -> dict:
    """Observed remaining byte is not the seed-derived initial capacity.

    Preserve out-of-range experimental records for inspection. This DTO does not
    authorize changing that byte or promise that an over-cap value is accepted.
    """
    if len(record) != 0xE8:
        raise ValueError('Expected a complete scroll record')
    prediction = predict_recommended_level(header.recommended_level)
    return {
        'initial_challenge_capacity': initial_challenge_capacity(header.seed),
        'remaining_challenge_attempts': record[0x33],
        'recommended_displayed_level': prediction.displayed_level,
        'recommended_raw_was_clamped': prediction.was_clamped,
    }
