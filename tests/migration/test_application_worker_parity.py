"""End-to-end application-surface gate for the Rust read-only worker (M2.3c).

Drives the real Rust development worker and the shipped Python worker over the
shipped framed-JSON protocol with the same working-tree data root, and compares
the whole search-application surface:

* `search.catalog` for every contract locale (en-US, ja-JP, zh-CN) and every
  certified rarity (3, 4, 5), including the ordered effect, terrain, enemy and
  special-rule arrays and every localized string inside them,
* the same payloads against the frozen zh-CN capture in
  `deliverables/m23c-application/catalog_reference_zh.json`, so a later change
  to the Python oracle cannot silently redefine what "correct" means,
* `recommended_level.resolve` over the captured exact, unreachable, saturated,
  clamping and out-of-range inputs,
* `cache.register`: one save-bound payload, every shipped rejection, the
  registry limit, and the `search.start` cache binding that refuses a cache for
  NG3 and refuses NG4/NG5 without a usable save-bound map.

The gate never skips and never mocks: a missing Rust bin target, a missing
method, or a stale binary is a failure with an owner-actionable message, and
every reply must satisfy `response.schema.json`. It is read-only: no live game,
no save access, and no job that would run a search.
"""
from __future__ import annotations

import json
import os
from pathlib import Path
import shutil
import struct
import subprocess
import sys
import tomllib
import unittest

from jsonschema import Draft7Validator


ROOT = Path(__file__).resolve().parents[2]
SCHEMA_DIR = ROOT / "packages" / "contracts"
DATA_ROOT = ROOT / "nioh3_scroll_editor" / "data"
ACCELERATOR = ROOT / "bin" / "nioh3_seed_accelerator.dll"
CATALOG_REFERENCE = ROOT / "deliverables" / "m23c-application" / "catalog_reference_zh.json"

if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

from nioh3_scroll_editor.grace_map import (  # noqa: E402
    grace_map_to_cache_payload,
    load_grace_output_map,
)
from tests.migration.cargo_target import resolved_cargo_target_dir  # noqa: E402

MAX_FRAME_BYTES = 4 * 1024 * 1024

RESPONSE_VALIDATOR = Draft7Validator(
    json.loads((SCHEMA_DIR / "response.schema.json").read_text(encoding="utf-8"))
)

# The contract's catalog surface.
LOCALES = ("en-US", "ja-JP", "zh-CN")
RARITIES = (3, 4, 5)
# Ordinary effects reachable inside one captured NG3 scroll context. Rarity 3
# cannot draw the promotion-only effect that rarity 4 and 5 both list.
ORDINARY_EFFECT_COUNTS = {3: 49, 4: 50, 5: 50}
# Measured Grace ids: rarity 4 keeps only its verified final Grace ids.
GRACE_EFFECT_COUNTS = {3: 0, 4: 21, 5: 11}
# Terrain display keys are the only option identity the auxiliary half derives
# from raw table fields; the order is the reference's first-occurrence order.
TERRAIN_OPTION_IDS = (
    "exact:24",
    "exact:",
    "exact:24,39",
    "exact:58",
    "exact:24,39F",
    "contains:24",
)
ENEMY_OPTION_COUNT = 487
SPECIAL_RULE_OPTION_COUNT = 277
SPECIAL_RULE_FAMILY_COUNT = 103

# `recommended_level.py`'s captured curve: below the first canonical display,
# the single unreachable display (328), the saturation band start, the cap and
# one past it, plus the exact and clamping inputs in between.
RECOMMENDED_LEVEL_CASES = (
    -5,
    0,
    1,
    137,
    138,
    142,
    200,
    250,
    313,
    328,
    530,
    699,
    700,
    701,
    1000,
    2147483647,
)
SATURATION_INTERNAL_RANGE = (1301, 1400)
UNREACHABLE_DISPLAYED_LEVEL = 328

FINGERPRINT = "ab" * 32

# The preferred effect locale is the only catalog input that does not come from
# the request. `NIOH3_SCROLL_LOCALE` is the only value the shipped interpreter
# honours for it: on Windows `locale.getlocale()` reports the OS default UI
# locale (`English_United States`) and ignores `LC_ALL`, `LC_CTYPE` and `LANG`,
# and the reference only accepts a shipped locale when the token before the
# hyphen is `en`, `ja` or `zh`, which such a name never is. Each case therefore
# records the reference's own resolved tag (None = the host-locale fallback)
# before the two workers are compared.
PREFERRED_LOCALE_CASES = (
    ("shipped default", {}, None),
    ("empty variable", {"NIOH3_SCROLL_LOCALE": ""}, None),
    ("lc_all only", {"LC_ALL": "ja_JP.UTF-8"}, None),
    ("explicit chinese", {"NIOH3_SCROLL_LOCALE": "zh-CN"}, "zh-CN"),
    ("explicit japanese", {"NIOH3_SCROLL_LOCALE": "ja-JP"}, "ja-JP"),
    ("explicit english", {"NIOH3_SCROLL_LOCALE": "en-US"}, "en-US"),
    (
        "underscore and unmatched language",
        {"NIOH3_SCROLL_LOCALE": "  German_Germany  "},
        "german-GERMANY",
    ),
)
SHIPPED_LANGUAGE_TOKENS = ("en", "ja", "zh")


def response_validator_errors(frame: dict) -> list:
    return sorted(RESPONSE_VALIDATOR.iter_errors(frame), key=lambda error: list(error.path))


def python_preferred_locale(env: dict[str, str]) -> str:
    """`catalog._PREFERRED_EFFECT_LOCALE` the shipped worker resolves for one env."""

    script = (
        "import sys; sys.path.insert(0, sys.argv[1]);"
        "from nioh3_scroll_editor import catalog;"
        "print(catalog._PREFERRED_EFFECT_LOCALE)"
    )
    completed = subprocess.run(
        [sys.executable, "-c", script, str(ROOT)],
        env=env,
        capture_output=True,
        text=True,
        encoding="utf-8",
        timeout=300,
    )
    if completed.returncode != 0:
        raise AssertionError(
            "the shipped catalog could not resolve a preferred locale: "
            + completed.stderr[-2000:]
        )
    return completed.stdout.strip()


def develop_worker_target() -> tuple[Path, str]:
    """Return (manifest, bin name) for the development worker; missing = failure."""

    override = os.environ.get("NIOH3_M23_WORKER_MANIFEST", "").strip()
    candidates: list[Path] = []
    if override:
        candidates.append(Path(override))
    crates = ROOT / "crates"
    if crates.is_dir():
        candidates.extend(sorted(crates.glob("*/Cargo.toml")))
    for manifest in candidates:
        if not manifest.is_file():
            continue
        with manifest.open("rb") as stream:
            payload = tomllib.load(stream)
        name = str(payload.get("package", {}).get("name", ""))
        bins = [str(entry.get("name", name)) for entry in payload.get("bin", [])]
        # The read-only worker is identified by its package or its binary name.
        # A substring test on "worker" is not enough: `crates/nioh3-protected`
        # sorts first and ships `nioh3-protected-worker`, so a substring test
        # would drive the protected host through this read-only gate.
        if name != "nioh3-worker" and "nioh3-readonly-worker" not in bins:
            continue
        return manifest, (bins[0] if bins else name)
    raise AssertionError(
        "the development worker crate is required by this gate and was not found "
        f"under {ROOT / 'crates'}"
    )


def worker_env(**locale_overrides: str) -> dict[str, str]:
    """The environment both workers see.

    `NIOH3_SCROLL_LOCALE` is *not* pinned: the shipped default path is part of
    the contract, and the environment matrix test below covers the explicit
    values. Callers pass `NIOH3_SCROLL_LOCALE=...` to force one.
    """

    env = dict(os.environ)
    env.setdefault("CARGO_TARGET_DIR", resolved_cargo_target_dir("m23c-worker"))
    env.pop("NIOH3_SCROLL_LOCALE", None)
    env.update(locale_overrides)
    return env


class FramedProcess:
    """Framed JSON stdio client with contract validation on every reply."""

    def __init__(self, argv: list[str], *, name: str, env: dict[str, str] | None = None) -> None:
        self.name = name
        self.counter = 0
        self.pending_id: str | None = None
        self.process = subprocess.Popen(
            argv,
            cwd=str(ROOT),
            env=worker_env() if env is None else env,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        handshake = self.call("handshake")
        if not handshake.get("ok"):
            raise AssertionError(f"{self.name} refused the handshake: {handshake}")
        self.digest = handshake["result"]["context"]["context_digest"]

    def call(self, method: str, params: dict | None = None) -> dict:
        self.counter += 1
        request_id = f"m23c-{self.counter}"
        body = json.dumps(
            {"protocol": 1, "id": request_id, "method": method, "params": params or {}},
            separators=(",", ":"),
        ).encode("utf-8")
        assert self.process.stdin is not None and self.process.stdout is not None
        self.process.stdin.write(struct.pack("<I", len(body)) + body)
        self.process.stdin.flush()
        self.pending_id = request_id
        return self.read_frame()

    def read_frame(self) -> dict:
        assert self.process.stdout is not None
        header = self.process.stdout.read(4)
        if len(header) != 4:
            raise EOFError(f"{self.name} closed stdout")
        (size,) = struct.unpack("<I", header)
        if size > MAX_FRAME_BYTES:
            raise ValueError(f"{self.name} frame exceeds the contract limit: {size}")
        body = self.process.stdout.read(size)
        if len(body) != size:
            raise EOFError(f"{self.name} frame truncated")
        frame = json.loads(body.decode("utf-8"))
        if frame.get("id") != self.pending_id:
            raise AssertionError(
                f"{self.name} replied to {frame.get('id')!r} while "
                f"{self.pending_id!r} was outstanding"
            )
        errors = response_validator_errors(frame)
        if errors:
            raise AssertionError(
                f"{self.name} frame violates response.schema.json: "
                + "; ".join(f"{list(error.path)}: {error.message}" for error in errors)
            )
        return frame

    def result(self, method: str, params: dict | None = None) -> dict:
        reply = self.call(method, params)
        if not reply.get("ok"):
            raise AssertionError(f"{self.name} {method} failed: {reply.get('error')}")
        return reply["result"]

    def outcome(self, method: str, params: dict | None = None) -> dict:
        """The semantic outcome, so two workers are compared on code and message."""

        reply = self.call(method, params)
        if reply.get("ok"):
            return {"ok": True, "result": reply["result"]}
        error = reply.get("error", {})
        return {"ok": False, "code": error.get("code"), "message": error.get("message")}

    def close(self) -> int:
        try:
            self.call("shutdown")
        except (EOFError, AssertionError):
            pass
        assert self.process.stdin is not None
        self.process.stdin.close()
        return self.process.wait(timeout=120)


def catalog_params(rarity: int, locale: str) -> dict:
    return {"playthrough": 3, "rarity": rarity, "locale": locale}


def catalog_projection(payload: dict) -> dict:
    """The frozen reference's shape for one catalog payload.

    The reference stores the auxiliary half as four arrays and keeps only the
    Grace ids, because the names are also compared through `ordinary_effects`.
    """

    return {
        "ordinary_effects": [
            {"effect_id": effect["effect_id"], "name": effect["name"]}
            for effect in payload["ordinary_effects"]
        ],
        "grace_effects_ids": [grace["effect_id"] for grace in payload["grace_effects"]],
        "auxiliary": {
            key: payload[key]
            for key in (
                "terrain_options",
                "enemy_options",
                "special_rule_options",
                "special_rule_families",
            )
        },
        "recommended_level": payload["recommended_level"],
    }


def search_params(context_digest: str, *, playthrough: int, rarity: int, cache_id: str | None) -> dict:
    params = {
        "query": {
            "playthrough": playthrough,
            "rarity": rarity,
            "level": 180,
            "primary_effect_ids": [],
            "required_secondary_ids": [],
            "required_secondary_id_groups": [],
            "grace_effect_id": None,
            "minimum_roll_percent_by_effect_id": [],
            "auxiliary": {
                "required_terrain_effect_keys": [],
                "required_terrain_effect_key_groups": [],
                "required_special_rule_keys": [113],
                "required_special_rule_key_groups": [],
                "required_enemy_lookup_keys": [],
                "required_enemy_lookup_key_groups": [],
            },
        },
        "context_digest": context_digest,
        "result_count": 1,
        "page_trials": 1000,
        "job_trials": 1000,
        "allow_cpu_fallback": False,
        "resume_token": None,
    }
    if cache_id is not None:
        params["cache_id"] = cache_id
    return params


class ApplicationWorkerParityTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        manifest, binary = develop_worker_target()
        cargo = shutil.which("cargo")
        if cargo is None:
            raise AssertionError("cargo is required to build and run the development worker")
        env = worker_env()
        build = subprocess.run(
            [
                cargo,
                "build",
                "--locked",
                "--offline",
                "--manifest-path",
                str(manifest),
                "--bin",
                binary,
            ],
            cwd=str(ROOT),
            env=env,
            capture_output=True,
            timeout=3600,
        )
        if build.returncode != 0:
            raise AssertionError(
                "the development worker did not build from the current candidate: "
                + build.stderr.decode("utf-8", "replace")[-4000:]
            )
        cls.binary_path = Path(env["CARGO_TARGET_DIR"]) / "debug" / f"{binary}.exe"
        cls.rust_argv = [
            str(cls.binary_path),
            "--dev-preview-only",
            "--data-root",
            str(DATA_ROOT),
            "--contract-dir",
            str(SCHEMA_DIR),
            "--accelerator",
            str(ACCELERATOR),
        ]
        cls.python_argv = [sys.executable, "-u", "-m", "nioh3_scroll_editor.search_worker"]

    def workers(self, **locale_overrides: str):
        """Both workers for one test, closed together even if the test fails."""

        env = worker_env(**locale_overrides)
        rust = FramedProcess(self.rust_argv, name="rust worker", env=env)
        python = FramedProcess(self.python_argv, name="python worker", env=env)
        self.addCleanup(rust.close)
        self.addCleanup(python.close)
        return rust, python

    def assert_catalog_invariants(self, payload: dict, rarity: int) -> None:
        """Ordering and completeness facts that do not depend on the oracle."""

        ordinary = payload["ordinary_effects"]
        self.assertEqual(len(ordinary), ORDINARY_EFFECT_COUNTS[rarity])
        self.assertTrue(all(effect["name"].strip() for effect in ordinary))
        self.assertEqual(
            len({effect["effect_id"] for effect in ordinary}),
            len(ordinary),
            "one effect id may appear only once",
        )

        grace_ids = [grace["effect_id"] for grace in payload["grace_effects"]]
        self.assertEqual(len(grace_ids), GRACE_EFFECT_COUNTS[rarity])
        self.assertEqual(grace_ids, sorted(grace_ids))
        self.assertTrue(all(grace["name"].strip() for grace in payload["grace_effects"]))

        self.assertEqual(
            tuple(option["option_id"] for option in payload["terrain_options"]),
            TERRAIN_OPTION_IDS,
        )
        for option in payload["terrain_options"]:
            self.assertTrue(option["name"].strip())
            self.assertEqual(
                option["aggregate"], option["option_id"].startswith("contains:")
            )

        enemies = payload["enemy_options"]
        self.assertEqual(len(enemies), ENEMY_OPTION_COUNT)
        self.assertEqual(
            [enemy["lookup_key"] for enemy in enemies],
            sorted(enemy["lookup_key"] for enemy in enemies),
        )
        self.assertTrue(all(enemy["name"].strip() for enemy in enemies))

        rules = payload["special_rule_options"]
        self.assertEqual(len(rules), SPECIAL_RULE_OPTION_COUNT)
        self.assertEqual([rule["key"] for rule in rules], sorted(rule["key"] for rule in rules))
        self.assertTrue(all(rule["name"].strip() for rule in rules))

        families = payload["special_rule_families"]
        self.assertEqual(len(families), SPECIAL_RULE_FAMILY_COUNT)
        folded = [family["name"].casefold() for family in families]
        self.assertEqual(folded, sorted(folded))
        for family in families:
            self.assertEqual(family["keys"], sorted(family["keys"]))
            self.assertTrue(family["keys"])

    def test_search_catalog_matches_the_python_worker_for_every_locale_and_rarity(self) -> None:
        rust, python = self.workers()
        self.assertEqual(
            rust.digest,
            python.digest,
            "both workers must report one generation context before their catalogs "
            "can be compared",
        )
        for locale in LOCALES:
            for rarity in RARITIES:
                with self.subTest(locale=locale, rarity=rarity):
                    rust_result = rust.result("search.catalog", catalog_params(rarity, locale))
                    python_result = python.result("search.catalog", catalog_params(rarity, locale))
                    self.assertEqual(rust_result, python_result)
                    self.assertEqual(rust_result["context_digest"], rust.digest)
                    self.assert_catalog_invariants(rust_result, rarity)
                    if locale == "zh-CN":
                        # `_player_ready_effect_name` resolves the preferred
                        # locale's name, and this gate pins that to zh-CN, so the
                        # delivered order is the sort key's order.
                        keys = [
                            (effect["name"].casefold(), effect["effect_id"])
                            for effect in rust_result["ordinary_effects"]
                        ]
                        self.assertEqual(keys, sorted(keys))

    def test_search_catalog_matches_the_frozen_zh_reference(self) -> None:
        reference = json.loads(CATALOG_REFERENCE.read_text(encoding="utf-8"))
        # The capture keeps the progression- and rarity-independent auxiliary
        # half and the curve metadata once, on rarity 3, and stores the two
        # rarity-specific arrays for every rarity. Asserting the shape keeps a
        # gutted capture from passing as "no differences".
        self.assertEqual(
            set(reference["3"]),
            {"ordinary_effects", "grace_effects_ids", "auxiliary", "recommended_level"},
        )
        for rarity in (4, 5):
            self.assertEqual(set(reference[str(rarity)]), {"ordinary_effects", "grace_effects_ids"})
        rust, python = self.workers()
        for rarity in RARITIES:
            with self.subTest(rarity=rarity):
                expected = reference[str(rarity)]
                for worker in (python, rust):
                    payload = worker.result("search.catalog", catalog_params(rarity, "zh-CN"))
                    projection = catalog_projection(payload)
                    for key, value in expected.items():
                        self.assertEqual(
                            projection[key],
                            value,
                            f"{worker.name} zh-CN catalog {key} for rarity {rarity}",
                        )

    def test_preferred_locale_resolution_matches_under_every_environment(self) -> None:
        """The non-request catalog input must resolve identically on both sides.

        The reference reads `NIOH3_SCROLL_LOCALE` and otherwise the host locale;
        the port must not invent a different default. Each case first records the
        reference's own resolved tag, then requires both workers to deliver the
        same payload for every request locale.
        """

        zh_catalog_payloads: dict[str, dict] = {}
        for label, overrides, expected_tag in PREFERRED_LOCALE_CASES:
            with self.subTest(case=label):
                env = worker_env(**overrides)
                resolved = python_preferred_locale(env)
                if expected_tag is None:
                    # A host-locale name never carries a shipped language token,
                    # so the reference falls through to its Chinese-name
                    # fallback. If that ever stops being true this gate says so
                    # instead of quietly comparing two different defaults.
                    self.assertNotIn(
                        resolved.split("-")[0].lower(),
                        SHIPPED_LANGUAGE_TOKENS,
                        f"the host locale resolved to the shipped tag {resolved!r}",
                    )
                else:
                    self.assertEqual(resolved, expected_tag)

                rust = FramedProcess(self.rust_argv, name="rust worker", env=env)
                python = FramedProcess(self.python_argv, name="python worker", env=env)
                self.addCleanup(rust.close)
                self.addCleanup(python.close)
                for locale in LOCALES:
                    payload_rust = rust.result("search.catalog", catalog_params(4, locale))
                    payload_python = python.result("search.catalog", catalog_params(4, locale))
                    self.assertEqual(payload_rust, payload_python, f"{label} / {locale}")
                    if locale == "zh-CN":
                        zh_catalog_payloads[label] = payload_rust

        # With no variable, an empty variable, or only `LC_ALL`, the shipped
        # worker delivers exactly the names an explicit `zh-CN` produces, and an
        # explicit other language really does change them.
        for label in ("shipped default", "empty variable", "lc_all only"):
            self.assertEqual(
                zh_catalog_payloads[label]["ordinary_effects"],
                zh_catalog_payloads["explicit chinese"]["ordinary_effects"],
                label,
            )
        self.assertNotEqual(
            zh_catalog_payloads["explicit japanese"]["ordinary_effects"],
            zh_catalog_payloads["explicit chinese"]["ordinary_effects"],
            "an explicit preferred locale must reach the payload",
        )

    def test_recommended_level_resolution_matches_for_every_curve_case(self) -> None:
        rust, python = self.workers()
        for level in RECOMMENDED_LEVEL_CASES:
            with self.subTest(displayed_level=level):
                params = {"displayed_level": level}
                rust_result = rust.result("recommended_level.resolve", params)
                self.assertEqual(rust_result, python.result("recommended_level.resolve", params))
                self.assertEqual(rust_result["requested_displayed_level"], level)
                self.assertEqual(
                    rust_result["canonical_internal_levels"],
                    sorted(rust_result["canonical_internal_levels"]),
                )

        saturation = rust.result("recommended_level.resolve", {"displayed_level": 700})
        self.assertEqual(saturation["status"], "exact")
        self.assertEqual(
            (saturation["canonical_internal_levels"][0], saturation["canonical_internal_levels"][-1]),
            SATURATION_INTERNAL_RANGE,
        )
        self.assertEqual(saturation["selected_internal_level"], SATURATION_INTERNAL_RANGE[0])

        unreachable = rust.result(
            "recommended_level.resolve", {"displayed_level": UNREACHABLE_DISPLAYED_LEVEL}
        )
        self.assertEqual(unreachable["status"], "unreachable")
        self.assertEqual(unreachable["canonical_internal_levels"], [])
        self.assertIsNone(unreachable["selected_internal_level"])

    def test_cache_registration_matches_the_python_worker(self) -> None:
        rust, python = self.workers()
        valid_json = json.dumps(
            grace_map_to_cache_payload(
                load_grace_output_map(rarity=5),
                context_fingerprint=FINGERPRINT,
                generation_context_digest=rust.digest,
            ),
            sort_keys=True,
        )
        rust_valid = rust.result("cache.register", {"cache_json": valid_json})
        python_valid = python.result("cache.register", {"cache_json": valid_json})
        self.assertEqual(rust_valid, python_valid)
        cache_id = rust_valid["cache_id"]

        def mutated(**changes: object) -> str:
            clone = json.loads(valid_json)
            clone.update(changes)
            return json.dumps(clone, sort_keys=True)

        rejections = {
            "unsupported schema": mutated(schema="nioh3-grace-output-map-cache/v1"),
            "foreign game version": mutated(game_version="1.00.00"),
            "stale generation context": mutated(generation_context_digest="cd" * 32),
            "not a draw-1 partition": mutated(draw_index=2),
            "truncated partition": json.dumps(
                {**json.loads(valid_json), "ranges": json.loads(valid_json)["ranges"][:-1]},
                sort_keys=True,
            ),
        }
        for label, cache_json in rejections.items():
            with self.subTest(rejection=label):
                rust_outcome = rust.outcome("cache.register", {"cache_json": cache_json})
                python_outcome = python.outcome("cache.register", {"cache_json": cache_json})
                self.assertEqual(rust_outcome, python_outcome, label)
                self.assertFalse(rust_outcome["ok"], label)
                self.assertEqual(rust_outcome["code"], "INVALID_REQUEST", label)

        # The registry holds the shipped 16 distinct maps. The valid payload above
        # is one of them, so the sixteenth distinct map still fits and the
        # seventeenth is refused with the registry-full message.
        fill_outcomes = []
        for index in range(1, 17):
            distinct = mutated(context_fingerprint=f"{index:064x}")
            rust_outcome = rust.outcome("cache.register", {"cache_json": distinct})
            python_outcome = python.outcome("cache.register", {"cache_json": distinct})
            self.assertEqual(rust_outcome, python_outcome, f"registry fill {index}")
            fill_outcomes.append(rust_outcome)
        self.assertTrue(all(outcome["ok"] for outcome in fill_outcomes[:-1]))
        self.assertFalse(fill_outcomes[-1]["ok"])
        self.assertIn("registry is full", fill_outcomes[-1]["message"])
        overflow = mutated(context_fingerprint="ef" * 32)
        rust_overflow = rust.outcome("cache.register", {"cache_json": overflow})
        python_overflow = python.outcome("cache.register", {"cache_json": overflow})
        self.assertEqual(rust_overflow, python_overflow)
        self.assertFalse(rust_overflow["ok"])
        self.assertEqual(rust_overflow["code"], "INVALID_REQUEST")

        # NG3 always uses the certified bundled map, and NG4/NG5 refuse without
        # a save-bound map for their own record type. Both refuse before a job
        # exists, so this stays a read-only check.
        ng3_cache = rust.outcome(
            "search.start", search_params(rust.digest, playthrough=3, rarity=4, cache_id=cache_id)
        )
        python_ng3_cache = python.outcome(
            "search.start",
            search_params(python.digest, playthrough=3, rarity=4, cache_id=cache_id),
        )
        self.assertEqual(ng3_cache, python_ng3_cache)
        self.assertFalse(ng3_cache["ok"])

        ng5_without_cache = rust.outcome(
            "search.start", search_params(rust.digest, playthrough=5, rarity=5, cache_id=None)
        )
        python_ng5_without_cache = python.outcome(
            "search.start",
            search_params(python.digest, playthrough=5, rarity=5, cache_id=None),
        )
        self.assertEqual(ng5_without_cache, python_ng5_without_cache)
        self.assertFalse(ng5_without_cache["ok"])


if __name__ == "__main__":
    unittest.main()
