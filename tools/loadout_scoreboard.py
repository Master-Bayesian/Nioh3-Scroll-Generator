"""Score PC202 optimizer catalog bindings against retained game observations."""

import argparse
from collections import Counter, defaultdict
from enum import StrEnum
import hashlib
import json
import math
import os
from pathlib import Path
import re
import struct
import subprocess
import sys


ROOT = Path(__file__).resolve().parents[1]
FIXTURES = ROOT / "tests/fixtures"
MANIFEST = ROOT / "crates/nioh3-loadout/Cargo.toml"
PC_BUILD = "PC202"
CHANNELS = ("total", "physical", "elemental")
SCENE_ID = re.compile(r"[a-zA-Z0-9-]{1,64}")
SHA256 = re.compile(r"[a-f0-9]{64}")


class Status(StrEnum):
    UNBOUND = "unbound"
    UNWITNESSED = "unwitnessed"
    MECHANISM_WITNESSED = "mechanism-witnessed"
    PENDING_REPLAY = "pending-replay"
    MATCH = "match"
    MISMATCH = "mismatch"


def require(condition, message):
    if not condition:
        raise ValueError(message)


def read_json(path):
    return json.loads(path.read_bytes())


def sha256(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def observed_number(value):
    require(type(value) in (int, float) and math.isfinite(value) and value >= 0,
            f"Invalid damage value: {value!r}")
    return value


def action_key(row):
    resource, action, parameter = (row[key] for key in ("resource", "action", "parameter_sha256"))
    require(isinstance(resource, str) and resource and type(action) is int and 0 <= action <= 65535
            and isinstance(parameter, str) and SHA256.fullmatch(parameter), f"Invalid catalog key: {row!r}")
    return resource, action, parameter


def evidence_file(root, reference):
    path = (root / reference["artifact"]).resolve()
    require(path.is_relative_to(root.resolve()), f"Evidence path escapes its root: {path}")
    require(sha256(path) == reference["sha256"], f"Evidence identity differs: {path}")
    return path


def witnessed_values(root, witness):
    source = read_json(evidence_file(root, witness["observation"]))
    refs = witness["rea"]
    require(isinstance(refs, list) and refs, f"{witness['id']} has no REA evidence")
    for ref in refs:
        require(re.fullmatch(r"REA[0-9]+:[a-zA-Z0-9_]+", ref["id"]), f"Invalid REA ID: {ref['id']}")
        evidence_file(root, ref)
    measurement = witness["measurement"]
    measured = measurement["rows"]
    require(measurement["kind"] in {"cast", "contacts"} and measured
            and (measurement["kind"] != "cast" or len(measured) == 1),
            f"Invalid measurement shape: {witness['id']}")
    expected = []
    for row in measured:
        require("total" in row and set(row) <= set(CHANNELS), f"Invalid measured channels: {witness['id']}")
        channels = {}
        for channel, item in row.items():
            paths = item["paths"]
            require(paths and all(isinstance(path, list) and path for path in paths),
                    f"Missing observed paths: {witness['id']}")
            values = []
            for path in paths:
                value = source
                for key in path:
                    value = value[key]
                values.append(observed_number(value))
            expected_value = observed_number(item["value"])
            require(sum(values) == expected_value, f"Fixture differs from retained observation: {witness['id']}")
            channels[channel] = expected_value
        expected.append(channels)
    return expected


def scene_mechanism(bundle_root, scene, key):
    require(SCENE_ID.fullmatch(scene), f"Invalid scene identity: {scene}")
    path = bundle_root / scene / "event_plan.json"
    if not path.is_file():
        return None
    plan = read_json(path)
    if plan["schema"] != "nioh3-production-event-plan/v1":
        return None
    templates = [row for row in plan["templates"] if row["scored"] and row["landed"]]
    if not templates or key not in {action_key(row["identity"]) for row in templates}:
        return None
    mode = plan["mode"]["kind"]
    if mode != "fixed_contacts":
        return mode
    packets = {row["packet"]["kind"] for row in templates}
    if packets == {"prepared_actor"}:
        return "fixed-direct-single" if len(templates) == 1 else "fixed-direct-multi"
    return "fixed:" + "+".join(sorted(packets))


def current_replay_executable():
    target = Path(os.environ["CARGO_TARGET_DIR"])
    subprocess.run(
        ["cargo", "build", "--manifest-path", str(MANIFEST), "--locked", "--offline",
         "--example", "loadout_witness_replay"],
        cwd=ROOT, check=True, stdout=sys.stderr, timeout=600,
    )
    return target / "debug/examples/loadout_witness_replay.exe"


def f32(value):
    return struct.unpack("<f", struct.pack("<f", value))[0]


def prediction_shape(contacts, channel, kind):
    if kind == "contacts":
        return [contact[channel] for contact in contacts]
    sums = dict.fromkeys(CHANNELS, 0.0)
    for contact in contacts:
        for field in CHANNELS:
            sums[field] = f32(sums[field] + observed_number(contact[channel][field]))
    return [sums]


def compare_rows(expected, actual):
    if len(expected) != len(actual):
        return {"expected_contacts": len(expected), "actual_contacts": len(actual)}
    for index, (wanted, got) in enumerate(zip(expected, actual, strict=True), 1):
        for channel, value in wanted.items():
            actual_value = observed_number(got[channel])
            if actual_value != value:
                return {"row": index, "channel": channel, "expected": value, "actual": actual_value}
    return None


def replay(executable, args, witness, expected):
    recipe = witness["replay"]
    scene = recipe["scene"]
    inputs = args.replay_root / scene
    original = inputs / "original"
    for name in ("memory.bin", "scene.txt"):
        require(sha256(original / name) == recipe["original_sha256"][name],
                f"Original MAIN input differs: {scene}/{name}")
    result = subprocess.run(
        [str(executable), str(args.bundle_root / scene), str(inputs / "context"), str(original)],
        capture_output=True, text=True, encoding="utf-8", check=False, timeout=120,
    )
    if result.returncode:
        return {"status": Status.PENDING_REPLAY, "difference": {"replay_failed": result.stderr.strip()}}
    receipt = json.loads(result.stdout)
    require(receipt["schema"] == "nioh3-loadout-witness-replay/v1"
            and receipt["context_digest"] == recipe["context_digest"],
            f"Replay used a different context: {witness['id']}")
    contacts = receipt["contacts"]
    require(contacts and all(row["index"] == index for index, row in enumerate(contacts, 1)),
            f"Invalid replay sequence: {witness['id']}")
    kind = witness["measurement"]["kind"]
    difference = None
    for row in contacts:
        if row["bits_equal"] is not True:
            difference = {"original_main_disagreed_at_contact": row["index"]}
            break
    if difference is None:
        difference = compare_rows(expected, prediction_shape(contacts, "cpu", kind))
    return {"status": Status.MISMATCH if difference else Status.MATCH,
            "difference": difference, "replay": receipt}


def source_identity():
    commit = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
    files = subprocess.check_output(
        ["git", "ls-files", "-z", "--cached", "--others", "--exclude-standard", "--",
         "crates/nioh3-loadout", ".cargo/config.toml"], cwd=ROOT,
    )
    digest = hashlib.sha256()
    for name in sorted(set(files.split(b"\0")) - {b""}):
        path = ROOT / os.fsdecode(name)
        digest.update(name + b"\0")
        digest.update(sha256(path).encode("ascii") if path.is_file() else b"missing")
        digest.update(b"\0")
    return {"commit": commit, "crate_sha256": digest.hexdigest()}


def run(args):
    source = source_identity()
    catalog_bytes = args.catalog.read_bytes()
    witness_bytes = args.witnesses.read_bytes()
    catalog = json.loads(catalog_bytes)
    require(catalog["version"] == 1 and catalog["selections"], "Expected the populated v1 action catalog")
    entries = {action_key(row): row for row in catalog["selections"]}
    require(len(entries) == len(catalog["selections"]), "Duplicate action catalog identity")
    witnesses = {}
    witness_digests = {}
    grouped = defaultdict(list)
    for line in witness_bytes.splitlines():
        if not line.strip():
            continue
        witness = json.loads(line)
        witness_id = witness["id"]
        key = action_key(witness["selection"])
        require(isinstance(witness_id, str) and SCENE_ID.fullmatch(witness_id) and witness_id not in witnesses
                and witness["pc_build"] == PC_BUILD, f"Invalid or duplicate witness: {witness_id}")
        witnesses[witness_id] = (witness, key, witnessed_values(args.evidence_root, witness))
        witness_digests[witness_id] = hashlib.sha256(line).hexdigest()
        grouped[key].append(witness_id)
    require(witnesses, "The observed fixture is empty")

    plans = {}
    for key, entry in entries.items():
        require(type(entry["plan_ready"]) is bool, f"Invalid catalog plan flag: {key}")
        plans[key] = {}
        if entry["plan_ready"]:
            for scene in entry["scene_ids"]:
                mechanism = scene_mechanism(args.bundle_root, scene, key)
                if mechanism:
                    plans[key][scene] = mechanism

    runnable = {name for name, (witness, key, _) in witnesses.items()
                if witness.get("replay", {}).get("scene") in plans.get(key, {})}
    executable = current_replay_executable() if runnable else None
    executable_sha = sha256(executable) if executable else None
    findings = {}
    validated_mechanisms = set()
    for name, (witness, key, expected) in witnesses.items():
        finding = {"selection": witness["selection"], "expected": expected,
                   "kind": witness["measurement"]["kind"], "observation": witness["observation"],
                   "rea": [ref["id"] for ref in witness["rea"]], "status": Status.PENDING_REPLAY,
                   "fixture_sha256": witness_digests[name], "scope": witness["scope"]}
        if name in runnable:
            finding.update(replay(executable, args, witness, expected))
            if finding["status"] == Status.MATCH:
                validated_mechanisms.add(plans[key][witness["replay"]["scene"]])
        findings[name] = finding

    rows = {}
    for key, entry in entries.items():
        names = grouped.get(key, [])
        results = {findings[name]["status"] for name in names}
        mechanisms = set(plans[key].values())
        if not mechanisms:
            status = Status.UNBOUND
        elif Status.MISMATCH in results:
            status = Status.MISMATCH
        elif Status.MATCH in results:
            status = Status.MATCH
        elif names:
            status = Status.PENDING_REPLAY
        elif len(mechanisms) == 1 and mechanisms <= validated_mechanisms:
            status = Status.MECHANISM_WITNESSED
        else:
            status = Status.UNWITNESSED
        rows[key] = {"resource": key[0], "action": key[1], "parameter_sha256": key[2],
                     "attack_id": entry["attack_id"], "name": entry["name"], "form": entry["school"],
                     "weapon": entry["weapon"], "stance": entry["stance"], "status": status,
                     "mechanisms": sorted(mechanisms), "witnesses": names}

    required = set(args.require_match)
    accepted = {}
    if args.check:
        baseline = read_json(args.baseline)
        accepted = baseline["required_matches"]
        require(baseline["version"] == 1 and isinstance(accepted, dict) and accepted
                and all(SHA256.fullmatch(digest) for digest in accepted.values()),
                "Expected a nonempty match baseline with fixture identities")
        required.update(accepted)
    failed = {}
    for name in sorted(required):
        if name not in findings:
            failed[name] = "witness missing"
            continue
        if name in accepted and accepted[name] != witness_digests[name]:
            failed[name] = "accepted fixture identity changed"
            continue
        status = rows.get(witnesses[name][1], {}).get("status")
        if status != Status.MATCH or findings[name]["status"] != Status.MATCH:
            failed[name] = f"selection={status or 'missing'}, witness={findings[name]['status']}"

    require(source_identity() == source, "The scorer source changed during replay")
    require(not executable or sha256(executable) == executable_sha, "The replay executable changed during replay")
    summary = {status: 0 for status in Status}
    summary.update(Counter(row["status"] for row in rows.values()))
    by_mechanism = defaultdict(Counter)
    for row in rows.values():
        mechanism = " / ".join(row["mechanisms"]) or "not-classified"
        by_mechanism[mechanism][row["status"]] += 1
    print(json.dumps({"catalog_entries": len(entries), "catalog_plan_ready": sum(
        row["plan_ready"] for row in entries.values()), "witnesses": len(witnesses),
        "replayed_witnesses": len(runnable), "statuses": summary,
        "by_mechanism": by_mechanism, "failed": failed}))
    if args.output:
        receipt = {"schema": "nioh3-loadout-scoreboard/v1", "pc_build": PC_BUILD,
                   "source": source, "catalog_sha256": hashlib.sha256(catalog_bytes).hexdigest(),
                   "witnesses_sha256": hashlib.sha256(witness_bytes).hexdigest(),
                   "replay_executable_sha256": executable_sha,
                   "summary": summary, "by_mechanism": by_mechanism, "failed": failed,
                   "witnesses": findings, "rows": list(rows.values())}
        args.output.parent.mkdir(parents=True, exist_ok=True)
        with args.output.open("x", encoding="utf-8", newline="\n") as stream:
            json.dump(receipt, stream, ensure_ascii=False, separators=(",", ":"))
            stream.write("\n")
    return 1 if failed else 0


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--catalog", type=Path, default=ROOT / "data/loadout/catalog.json")
    parser.add_argument("--witnesses", type=Path, default=FIXTURES / "loadout_witnesses.jsonl")
    parser.add_argument("--baseline", type=Path, default=FIXTURES / "loadout_scoreboard.baseline.json")
    parser.add_argument("--evidence-root", type=Path, default=Path("F:/Nioh3_r16_build/deliverables"))
    parser.add_argument("--bundle-root", type=Path, default=Path("D:/N4i/project-source/quality/data/loadout"))
    parser.add_argument("--replay-root", type=Path, default=Path("D:/N4i/resources/scenes"))
    parser.add_argument("--output", type=Path, help="A fresh JSON receipt path")
    parser.add_argument("--summary", action="store_true", help="Print summary (also printed with --check)")
    parser.add_argument("--check", action="store_true", help="Fail if a baseline witness stops matching")
    parser.add_argument("--require-match", action="append", default=[], metavar="WITNESS",
                        help="Fail unless this work unit's witness matches")
    try:
        return run(parser.parse_args())
    except (KeyError, IndexError, OSError, ValueError, TypeError, subprocess.SubprocessError) as error:
        print(f"Scoreboard input or replay error: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    sys.exit(main())
