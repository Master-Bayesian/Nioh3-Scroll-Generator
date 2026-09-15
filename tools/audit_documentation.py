"""Audit local Markdown targets and the top-level knowledge-document index.

The audit intentionally runs without network access. It validates Markdown link
and image destinations that resolve inside the repository and confirms that the
knowledge index links every top-level knowledge Markdown document.
"""

from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path
from typing import Iterable
from urllib.parse import unquote
import argparse
import re


REPOSITORY_ROOT = Path(__file__).resolve().parents[1]
DOCUMENTATION_ROOT = REPOSITORY_ROOT / "docs"
KNOWLEDGE_ROOT = DOCUMENTATION_ROOT / "knowledge"
INDEX_PATH = KNOWLEDGE_ROOT / "INDEX.md"

FENCE_PATTERN = re.compile(r"^ {0,3}(`{3,}|~{3,})")
REFERENCE_DEFINITION_PATTERN = re.compile(r"^ {0,3}\[[^]]+\]:\s*(.*)$")


@dataclass(frozen=True)
class AuditFinding:
    """A deterministic audit failure with a source file and human-readable detail."""

    path: Path
    detail: str

    def display(self, root: Path) -> str:
        return f"{self.path.relative_to(root).as_posix()}: {self.detail}"


def markdown_without_fenced_code(text: str) -> str:
    """Remove fenced blocks so example links do not become audited targets."""

    kept_lines: list[str] = []
    fence_marker: str | None = None
    fence_length = 0
    for line in text.splitlines(keepends=True):
        match = FENCE_PATTERN.match(line)
        if fence_marker is None:
            if match:
                fence_marker = match.group(1)[0]
                fence_length = len(match.group(1))
            else:
                kept_lines.append(line)
        elif match and match.group(1)[0] == fence_marker and len(match.group(1)) >= fence_length:
            fence_marker = None
            fence_length = 0
    return "".join(kept_lines)


def _destination_from_parenthesized(content: str) -> str | None:
    value = content.strip()
    if not value:
        return ""
    if value.startswith("<"):
        closing = value.find(">", 1)
        return value[1:closing] if closing != -1 else value[1:]
    return value.split(maxsplit=1)[0]


def _parenthesized_destinations(text: str) -> Iterable[str]:
    """Yield destinations from inline Markdown links and images.

    A small scanner is used instead of a regular expression so parentheses in a
    destination do not truncate the target.
    """

    cursor = 0
    length = len(text)
    while cursor < length:
        if text[cursor] == "!" and cursor + 1 < length and text[cursor + 1] == "[":
            opening = cursor + 1
        elif text[cursor] == "[":
            opening = cursor
        else:
            cursor += 1
            continue

        label_end = opening + 1
        while label_end < length:
            if text[label_end] == "\\":
                label_end += 2
                continue
            if text[label_end] == "]":
                break
            label_end += 1
        if label_end >= length or label_end + 1 >= length or text[label_end + 1] != "(":
            cursor = opening + 1
            continue

        destination_start = label_end + 2
        position = destination_start
        depth = 1
        while position < length and depth:
            if text[position] == "\\":
                position += 2
                continue
            if text[position] == "(":
                depth += 1
            elif text[position] == ")":
                depth -= 1
            position += 1
        if depth:
            cursor = destination_start
            continue
        destination = _destination_from_parenthesized(text[destination_start : position - 1])
        if destination is not None:
            yield destination
        cursor = position


def markdown_destinations(text: str) -> list[str]:
    """Return inline and reference-definition destinations outside fenced code."""

    visible_text = markdown_without_fenced_code(text)
    destinations = list(_parenthesized_destinations(visible_text))
    for line in visible_text.splitlines():
        match = REFERENCE_DEFINITION_PATTERN.match(line)
        if match:
            destination = _destination_from_parenthesized(match.group(1))
            if destination is not None:
                destinations.append(destination)
    return destinations


def local_destination(destination: str) -> str | None:
    """Return a repository-local path component, or None for external targets."""

    value = destination.strip().replace("\\ ", " ")
    if not value or value.startswith("#") or value.startswith("//"):
        return None
    lowered = value.casefold()
    if lowered.startswith(("http:", "https:", "mailto:", "tel:", "data:")):
        return None
    if re.match(r"^[a-z][a-z0-9+.-]*:", value, flags=re.IGNORECASE):
        return None
    return unquote(value.split("#", maxsplit=1)[0].split("?", maxsplit=1)[0])


def resolve_destination(source: Path, destination: str, repository_root: Path) -> Path | None:
    """Resolve a local Markdown destination only when it remains in the repository."""

    local = local_destination(destination)
    if local is None:
        return None
    candidate = (source.parent / local).resolve()
    try:
        candidate.relative_to(repository_root.resolve())
    except ValueError:
        return None
    return candidate


def audit_markdown_links(documentation_root: Path, repository_root: Path) -> list[AuditFinding]:
    """Report missing repository-local Markdown link and image targets under docs."""

    findings: list[AuditFinding] = []
    for source in sorted(documentation_root.rglob("*.md")):
        for destination in markdown_destinations(source.read_text(encoding="utf-8")):
            target = resolve_destination(source, destination, repository_root)
            if target is not None and not target.exists():
                findings.append(AuditFinding(source, f"missing local target '{destination}'"))
    return findings


def indexed_top_level_knowledge_documents(knowledge_root: Path, index_path: Path) -> set[Path]:
    """Resolve local index destinations for the complete-index coverage check."""

    indexed: set[Path] = set()
    repository_root = knowledge_root.parents[1]
    for destination in markdown_destinations(index_path.read_text(encoding="utf-8")):
        target = resolve_destination(index_path, destination, repository_root)
        if target is not None:
            indexed.add(target)
    return indexed


def audit_knowledge_index(knowledge_root: Path, index_path: Path) -> list[AuditFinding]:
    """Report top-level knowledge Markdown documents omitted from INDEX.md."""

    # Normalize both sides before comparing path identity. Windows runners may
    # expose the temporary root through an 8.3 alias (for example RUNNER~1)
    # while Path.resolve() expands link destinations to their long form.
    expected = {path.resolve() for path in knowledge_root.glob("*.md")}
    indexed = indexed_top_level_knowledge_documents(knowledge_root, index_path)
    missing = sorted(expected - indexed)
    return [
        AuditFinding(index_path, f"top-level knowledge document is not indexed: {path.name}")
        for path in missing
    ]


def audit(repository_root: Path = REPOSITORY_ROOT) -> list[AuditFinding]:
    """Run the full offline documentation audit in deterministic path order."""

    repository_root = repository_root.resolve()
    documentation_root = repository_root / "docs"
    knowledge_root = documentation_root / "knowledge"
    index_path = knowledge_root / "INDEX.md"
    findings = audit_markdown_links(documentation_root, repository_root)
    if not index_path.is_file():
        findings.append(AuditFinding(index_path, "knowledge index is missing"))
    else:
        findings.extend(audit_knowledge_index(knowledge_root, index_path))
    return sorted(findings, key=lambda finding: (finding.path.as_posix(), finding.detail))


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repository-root", type=Path, default=REPOSITORY_ROOT)
    args = parser.parse_args()
    root = args.repository_root.resolve()
    findings = audit(root)
    if findings:
        for finding in findings:
            print(f"ERROR: {finding.display(root)}")
        return 1
    print("Documentation audit passed.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
