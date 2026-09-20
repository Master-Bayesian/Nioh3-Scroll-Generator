"""Selected effect resources must reach the post-accelerator exact replay."""

from __future__ import annotations

from types import SimpleNamespace
import unittest
from unittest.mock import patch

from nioh3_scroll_editor.effect_path_inverse import (
    FullCompositionRequest,
    OneWildcardCompositionRequest,
)
from nioh3_scroll_editor.effect_preimage_search import (
    collect_full_composition_preimage_page,
    collect_one_wildcard_composition_preimage_page,
)


class PythonR5PreimageIdentityTests(unittest.TestCase):
    @staticmethod
    def _result():
        effect = lambda value: SimpleNamespace(effect_id=value)
        return SimpleNamespace(
            primary=effect(1),
            secondaries=tuple(effect(value) for value in (2, 3, 4, 5)),
            grace=effect(6),
        )

    def _run_page(self, *, wildcard: bool) -> None:
        selected_tables = object()
        selected_mapping = object()
        plan = SimpleNamespace(pivot_state_count=1)
        generated_calls: list[dict[str, object]] = []

        def generate(seed: int, **kwargs):
            self.assertEqual(seed, 1)
            generated_calls.append(kwargs)
            return self._result()

        if wildcard:
            request = OneWildcardCompositionRequest(5, (1, 2, 3, 4), 6)
            collector = collect_one_wildcard_composition_preimage_page
            compiler_name = (
                "nioh3_scroll_editor.effect_preimage_search."
                "compile_one_wildcard_composition_plans"
            )
        else:
            request = FullCompositionRequest(5, 1, (2, 3, 4, 5), 6)
            collector = collect_full_composition_preimage_page
            compiler_name = (
                "nioh3_scroll_editor.effect_preimage_search."
                "compile_full_composition_plans"
            )

        with (
            patch(compiler_name, return_value=(plan,)) as compiler,
            patch(
                "nioh3_scroll_editor.effect_preimage_search."
                "collect_effect_preimage_matches_d3d11",
                return_value=((1, 0),),
            ),
            patch(
                "nioh3_scroll_editor.effect_path_inverse."
                "generate_rarity5_grace_effect_sequence",
                side_effect=generate,
            ),
        ):
            page = collector(
                request,
                page_size=1,
                tables=selected_tables,
                special_mapping=selected_mapping,
            )

        self.assertIsNotNone(page)
        assert page is not None
        self.assertEqual(tuple(match.seed for match in page.matches), (1,))
        self.assertIs(compiler.call_args.kwargs["tables"], selected_tables)
        self.assertIs(compiler.call_args.kwargs["special_mapping"], selected_mapping)
        self.assertEqual(len(generated_calls), 1)
        self.assertIs(generated_calls[0]["tables"], selected_tables)
        self.assertIs(generated_calls[0]["grace_mapping"], selected_mapping)

    def test_complete_page_exact_replay_uses_selected_context(self) -> None:
        self._run_page(wildcard=False)

    def test_wildcard_page_exact_replay_uses_selected_context(self) -> None:
        self._run_page(wildcard=True)


if __name__ == "__main__":
    unittest.main()
