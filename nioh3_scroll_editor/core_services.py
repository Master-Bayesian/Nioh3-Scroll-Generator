"""UI-independent contracts for the frozen v0.6 backend.

These facades are intentionally small.  They give the current Tk client and a
future headless host the same policy and context boundary without moving the
verified generation algorithms during the Frontend V2 transition.
"""

from __future__ import annotations

import hashlib
import json
from dataclasses import asdict, dataclass
from enum import Enum
from functools import lru_cache
from pathlib import Path
import struct
import sys
from typing import TYPE_CHECKING, Any

from .models import CandidateRecordStage, ScrollCandidate
from .seed_accelerator import seed_accelerator_identity
from .version import APP_VERSION

if TYPE_CHECKING:
    from .savegame import InstallResult, SaveInstaller


GENERATION_ALGORITHM_VERSION = "scroll-generation-v0.6-freeze-1"
OPERATION_POLICY_VERSION = "operation-policy-v1"
SUPPORTED_GAME_PROFILE = "pc-v2.00.02-v2.01"


class CoreErrorCode(str, Enum):
    UNSUPPORTED_CONTEXT = "UNSUPPORTED_CONTEXT"
    RESOURCE_MISMATCH = "RESOURCE_MISMATCH"
    ABI_MISMATCH = "ABI_MISMATCH"
    CANDIDATE_NOT_INSTALLABLE = "CANDIDATE_NOT_INSTALLABLE"
    CANDIDATE_CHANGED = "CANDIDATE_CHANGED"
    SAVE_CHANGED = "SAVE_CHANGED"
    REMOTE_CALL_PENDING = "REMOTE_CALL_PENDING"
    HOOK_STATE_UNKNOWN = "HOOK_STATE_UNKNOWN"


class OperationCommand(str, Enum):
    PREVIEW_GENERATED = "preview_generated"
    INSTALL_GENERATED = "install_generated"
    CUSTOM_EDIT = "custom_edit"
    RUNTIME_OVERRIDE = "runtime_override"


class SearchJobState(str, Enum):
    QUEUED = "queued"
    RUNNING = "running"
    CANCEL_REQUESTED = "cancel_requested"
    COMPLETED = "completed"
    CANCELLED = "cancelled"
    FAILED = "failed"


@dataclass(frozen=True, slots=True)
class CoreServiceError(RuntimeError):
    code: CoreErrorCode
    message: str
    details: dict[str, Any] | None = None

    def __str__(self) -> str:
        return self.message

    def to_payload(self) -> dict[str, Any]:
        return {
            "code": self.code.value,
            "message": self.message,
            "details": self.details or {},
        }


@dataclass(frozen=True, slots=True)
class SearchJobSnapshot:
    job_id: str
    state: SearchJobState
    context_digest: str
    cursor: int = 0
    delivered_candidates: int = 0
    error: CoreServiceError | None = None


def _runtime_data_root() -> Path:
    frozen_root = getattr(sys, "_MEIPASS", None)
    if frozen_root:
        return Path(frozen_root) / "nioh3_scroll_editor" / "data"
    return Path(__file__).resolve().parent / "data"


def _hash_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


@lru_cache(maxsize=4)
def runtime_resource_digest(data_root_text: str | None = None) -> str:
    """Hash every packaged runtime data file with its stable relative path."""

    data_root = Path(data_root_text) if data_root_text else _runtime_data_root()
    data_root = data_root.resolve()
    if not data_root.is_dir():
        raise CoreServiceError(
            CoreErrorCode.RESOURCE_MISMATCH,
            f"runtime data directory is missing: {data_root}",
        )
    files = tuple(sorted(path for path in data_root.rglob("*") if path.is_file()))
    if not files:
        raise CoreServiceError(
            CoreErrorCode.RESOURCE_MISMATCH,
            "runtime data directory is empty",
        )
    digest = hashlib.sha256()
    for path in files:
        relative = path.relative_to(data_root).as_posix().encode("utf-8")
        digest.update(struct.pack("<I", len(relative)))
        digest.update(relative)
        digest.update(bytes.fromhex(_hash_file(path)))
    return digest.hexdigest()


@dataclass(frozen=True, slots=True)
class GenerationContext:
    product_version: str
    game_profile: str
    resources_digest: str
    algorithm_version: str
    policy_version: str
    seed_accelerator_abi: int | None
    seed_accelerator_build_id: str | None
    context_digest: str

    @classmethod
    def capture(
        cls,
        *,
        game_profile: str = SUPPORTED_GAME_PROFILE,
        data_root: Path | None = None,
    ) -> "GenerationContext":
        identity = seed_accelerator_identity()
        identity_payload = {
            "product_version": APP_VERSION,
            "game_profile": game_profile,
            "resources_digest": runtime_resource_digest(
                str(data_root.resolve()) if data_root is not None else None
            ),
            "algorithm_version": GENERATION_ALGORITHM_VERSION,
            "policy_version": OPERATION_POLICY_VERSION,
            "seed_accelerator_abi": identity[0] if identity else None,
            "seed_accelerator_build_id": identity[1] if identity else None,
        }
        canonical = json.dumps(
            identity_payload,
            sort_keys=True,
            separators=(",", ":"),
        ).encode("utf-8")
        return cls(
            **identity_payload,
            context_digest=hashlib.sha256(canonical).hexdigest(),
        )

    def to_payload(self) -> dict[str, Any]:
        return asdict(self)


@dataclass(frozen=True, slots=True)
class OperationDecision:
    command: OperationCommand
    allowed: bool
    code: CoreErrorCode | None = None
    message: str | None = None


class OperationPolicy:
    """Central policy for generated candidates and explicitly custom paths."""

    def evaluate(
        self,
        command: OperationCommand,
        *,
        candidate: ScrollCandidate | None = None,
    ) -> OperationDecision:
        if command in (OperationCommand.CUSTOM_EDIT, OperationCommand.RUNTIME_OVERRIDE):
            return OperationDecision(command, True)
        if candidate is None:
            return OperationDecision(
                command,
                False,
                CoreErrorCode.CANDIDATE_NOT_INSTALLABLE,
                "generated-candidate operation requires a candidate",
            )
        if command is OperationCommand.PREVIEW_GENERATED:
            return OperationDecision(command, True)
        blocker = candidate.install_blocker
        if blocker:
            return OperationDecision(
                command,
                False,
                CoreErrorCode.CANDIDATE_NOT_INSTALLABLE,
                blocker,
            )
        return OperationDecision(command, True)

    def require(
        self,
        command: OperationCommand,
        *,
        candidate: ScrollCandidate | None = None,
    ) -> None:
        decision = self.evaluate(command, candidate=candidate)
        if not decision.allowed:
            raise CoreServiceError(
                decision.code or CoreErrorCode.CANDIDATE_NOT_INSTALLABLE,
                decision.message or "operation rejected",
            )


def candidate_identity(candidate: ScrollCandidate, context_digest: str) -> str:
    digest = hashlib.sha256()
    digest.update(context_digest.encode("ascii"))
    digest.update(struct.pack("<I", candidate.seed))
    digest.update(struct.pack("<i", candidate.playthrough or 0))
    digest.update(struct.pack("<i", candidate.rarity))
    digest.update(candidate.record_stage.value.encode("ascii"))
    digest.update(candidate.record)
    digest.update(candidate.installation_record or b"")
    for effect in candidate.effects:
        digest.update(
            struct.pack(
                "<7I",
                effect.slot,
                effect.effect_id,
                effect.value,
                effect.metadata,
                effect.prefix,
                effect.tail_0,
                effect.tail_1,
            )
        )
    return digest.hexdigest()


@dataclass(frozen=True, slots=True)
class FinalPreview:
    candidate_id: str
    context_digest: str
    seed: int
    playthrough: int | None
    rarity: int
    record_stage: CandidateRecordStage
    installable: bool
    install_blocker: str | None


@dataclass(frozen=True, slots=True)
class InstallPlan:
    candidate_id: str
    context_digest: str
    materialize_at_install: bool
    record_sha256: str | None
    custom_only_drop: bool


class CandidateApplicationService:
    """Shared candidate preflight and install facade for Tk and headless hosts."""

    def __init__(
        self,
        context: GenerationContext | None = None,
        policy: OperationPolicy | None = None,
    ) -> None:
        self.context = context or GenerationContext.capture()
        self.policy = policy or OperationPolicy()

    def preview(self, candidate: ScrollCandidate) -> FinalPreview:
        decision = self.policy.evaluate(
            OperationCommand.INSTALL_GENERATED,
            candidate=candidate,
        )
        return FinalPreview(
            candidate_id=candidate_identity(candidate, self.context.context_digest),
            context_digest=self.context.context_digest,
            seed=candidate.seed,
            playthrough=candidate.playthrough,
            rarity=candidate.rarity,
            record_stage=candidate.record_stage,
            installable=decision.allowed,
            install_blocker=decision.message,
        )

    def prepare_generated_install(self, candidate: ScrollCandidate) -> InstallPlan:
        self.policy.require(OperationCommand.INSTALL_GENERATED, candidate=candidate)
        selected_record = candidate.installation_record or candidate.record
        return InstallPlan(
            candidate_id=candidate_identity(candidate, self.context.context_digest),
            context_digest=self.context.context_digest,
            materialize_at_install=candidate.can_materialize_for_install,
            record_sha256=(
                hashlib.sha256(selected_record).hexdigest()
                if selected_record
                else None
            ),
            custom_only_drop=(
                candidate.playthrough in (1, 2) and candidate.rarity == 4
            ),
        )

    def execute_generated_install(
        self,
        plan: InstallPlan,
        candidate: ScrollCandidate,
        installer: "SaveInstaller",
        *,
        level: int,
        recommended_level: int,
        transfer_count: int,
    ) -> "InstallResult":
        if plan.context_digest != self.context.context_digest:
            raise CoreServiceError(
                CoreErrorCode.RESOURCE_MISMATCH,
                "install plan belongs to a different generation context",
            )
        if plan.candidate_id != candidate_identity(candidate, self.context.context_digest):
            raise CoreServiceError(
                CoreErrorCode.CANDIDATE_CHANGED,
                "candidate changed after install preflight",
            )
        self.policy.require(OperationCommand.INSTALL_GENERATED, candidate=candidate)
        if candidate.can_materialize_for_install:
            return installer.install_effect_sequence_candidate(
                candidate,
                level=level,
                recommended_level=recommended_level,
                transfer_count=transfer_count,
            )
        selected_record = candidate.installation_record or candidate.record
        if hashlib.sha256(selected_record).hexdigest() != plan.record_sha256:
            raise CoreServiceError(
                CoreErrorCode.CANDIDATE_CHANGED,
                "candidate installation record changed after preflight",
            )
        return installer.install(selected_record, transfer_count=transfer_count)


__all__ = [
    "CandidateApplicationService",
    "CoreErrorCode",
    "CoreServiceError",
    "FinalPreview",
    "GenerationContext",
    "InstallPlan",
    "OperationCommand",
    "OperationDecision",
    "OperationPolicy",
    "SearchJobSnapshot",
    "SearchJobState",
    "candidate_identity",
    "runtime_resource_digest",
]
