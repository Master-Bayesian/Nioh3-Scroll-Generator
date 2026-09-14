from __future__ import annotations

import importlib.util
from pathlib import Path
import sys
import tempfile
import unittest


MODULE_PATH = Path(__file__).resolve().parents[1] / "tools" / "audit_documentation.py"
SPEC = importlib.util.spec_from_file_location("audit_documentation", MODULE_PATH)
assert SPEC is not None and SPEC.loader is not None
MODULE = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)


class DocumentationAuditTests(unittest.TestCase):
    def create_repository(self, root: Path, index: str) -> tuple[Path, Path]:
        docs = root / "docs"
        knowledge = docs / "knowledge"
        knowledge.mkdir(parents=True)
        (knowledge / "INDEX.md").write_text(index, encoding="utf-8")
        return docs, knowledge

    def test_audit_accepts_links_images_references_and_fenced_examples(self) -> None:
        with tempfile.TemporaryDirectory() as raw_temp:
            root = Path(raw_temp)
            docs, knowledge = self.create_repository(
                root,
                "[Index](INDEX.md)\n[Current](CURRENT.md)\n",
            )
            (knowledge / "CURRENT.md").write_text(
                "[Guide](../guide.md#part)\n"
                "![Diagram](../images/diagram.png)\n"
                "[Reference]: ../guide.md\n"
                "```markdown\n[Example](missing-example.md)\n```\n",
                encoding="utf-8",
            )
            (docs / "guide.md").write_text("# Guide\n", encoding="utf-8")
            image_directory = docs / "images"
            image_directory.mkdir()
            (image_directory / "diagram.png").write_bytes(b"png")

            self.assertEqual(MODULE.audit_markdown_links(docs, root), [])
            self.assertEqual(MODULE.audit_knowledge_index(knowledge, knowledge / "INDEX.md"), [])

    def test_audit_reports_missing_local_target_and_unindexed_document(self) -> None:
        with tempfile.TemporaryDirectory() as raw_temp:
            root = Path(raw_temp)
            docs, knowledge = self.create_repository(root, "[Index](INDEX.md)\n")
            (knowledge / "CURRENT.md").write_text(
                "[Missing](missing.md)\n[Remote](https://example.test/docs)\n",
                encoding="utf-8",
            )

            link_findings = MODULE.audit_markdown_links(docs, root)
            index_findings = MODULE.audit_knowledge_index(knowledge, knowledge / "INDEX.md")

            self.assertEqual(len(link_findings), 1)
            self.assertIn("missing local target 'missing.md'", link_findings[0].detail)
            self.assertEqual(len(index_findings), 1)
            self.assertIn("CURRENT.md", index_findings[0].detail)

    def test_parenthesized_destinations_and_anchors_resolve_deterministically(self) -> None:
        with tempfile.TemporaryDirectory() as raw_temp:
            root = Path(raw_temp)
            docs, knowledge = self.create_repository(
                root,
                "[Index](INDEX.md)\n[Current](CURRENT.md)\n",
            )
            (knowledge / "CURRENT.md").write_text(
                "[Release](../releases/(candidate).md?view=full#evidence)\n",
                encoding="utf-8",
            )
            release_directory = docs / "releases"
            release_directory.mkdir()
            (release_directory / "(candidate).md").write_text("# Candidate\n", encoding="utf-8")

            self.assertEqual(MODULE.audit_markdown_links(docs, root), [])


if __name__ == "__main__":
    unittest.main()
