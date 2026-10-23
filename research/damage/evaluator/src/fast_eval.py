"""In-process exact evaluator for search: the same model and native MAIN replay as rank_outfits.py, kept warm.

Per candidate: OutfitModel.predict (about 10 ms) gives the STAT/D word writes and the attack-packet writes, exactly as
rank_outfits.py writes them into the baseline request. MAIN is then replayed natively, unless a previous replay
proves the answer:

  Read-set memo. Every replay records every byte address the emulator read (all reads, including bookkeeping
  reads). The emulator is deterministic, so two candidates that hold the same bytes at every address one of them
  read during its run execute identically and produce the same damage. Candidates differ from the template only
  where they write, so a memo key is the set of (address, byte) writes that land inside a recorded read set and
  differ from the template. This is exact, not a similarity heuristic.

The cache-integrity check of the sealed runtime (r12_bind.prepare) runs once per process instead of once per replay.
Search replays run without the initial-score ablation control; `confirm()` re-runs a candidate through the full
v1 acceptance path (run_request with ablation) for reported results.
"""
import bisect
import json
import struct
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
from outfit_model import OutfitModel, Unknown  # noqa: E402
import demand_fill_loop  # noqa: E402

READS = None  # set of byte addresses read during the current replay, or None when not recording


def _install_read_recorder():
    import native_vm
    base_read = native_vm.Memory.read

    def read(self, address, size):
        if READS is not None:
            READS.update(range(address, address + size))
        return base_read(self, address, size)

    native_vm.Memory.read = read


def build_template(model, request_path, retained_path, extra_cells):
    """The baseline filled request plus retained static-table cells and static-resource reconstructions.

    Same construction as rank_outfits.py: retained cells only from static-table readers and only where the request
    has no region of that address and size; reconstructed cells only on bytes no captured region covers.
    """
    from demand_fill_loop import STATIC_TABLE_READERS
    template = json.loads(Path(request_path).read_bytes())
    have = {(r["address_u64"], len(r["value_hex"])) for r in template["scene"]["memory_regions"]}
    for cell in json.loads(Path(retained_path).read_bytes()):
        if cell["provenance"]["demand_reader_rva"].split(":")[0] in STATIC_TABLE_READERS and \
                (cell["address_u64"], len(cell["value_hex"])) not in have:
            template["scene"]["memory_regions"].append(cell)
    spans = sorted((r["address_u64"], r["address_u64"] + len(r["value_hex"]) // 2) for r in template["scene"]["memory_regions"])
    starts = [s for s, _ in spans]
    for path in extra_cells:
        for cell in json.loads(Path(path).read_bytes()):
            a, data = cell["address_u64"], bytes.fromhex(cell["value_hex"])
            k = bisect.bisect_right(starts, a + len(data)) - 1
            covered = set()
            while k >= 0 and spans[k][1] > a - 0x10000:
                covered.update(range(max(a, spans[k][0]), min(a + len(data), spans[k][1])))
                k -= 1
            run = []
            for i in range(len(data) + 1):
                if i < len(data) and a + i not in covered:
                    run.append(i)
                    continue
                if run:
                    piece = data[run[0]:run[-1] + 1]
                    template["scene"]["memory_regions"].append({**cell, "name": cell["name"] + f"+{run[0]}",
                                                                "address_u64": a + run[0], "value_hex": piece.hex(),
                                                                "known_hex": "ff" * len(piece)})
                    run = []
    return template


class Evaluator:
    def __init__(self, cache, capture, request, retained, extra_cells=(), buff=None, observe_only=False,
                 armored_baseline=False):
        self.cache, self.capture, self.buff = Path(cache), Path(capture), buff
        self.model = OutfitModel(cache, capture, observe_only, armored_baseline)
        sys.path.insert(0, str(demand_fill_loop.REPLAY_DIR))
        import replay
        import r12_bind
        self.replay = replay
        demand_fill_loop.install_getter_cut(replay)
        prepared = r12_bind.prepare(self.cache)  # integrity check of the sealed runtime, once per process
        r12_bind.prepare = lambda cache: prepared
        _install_read_recorder()
        self.template = build_template(self.model, request, retained, extra_cells)
        self._index()
        self.owner_addr = self.model.R["source.HP_owner"][0]
        self.stats = {"replays": 0, "memo_hits": 0, "replay_seconds": 0.0, "unknown": 0}

    def _index(self):
        regions = sorted(self.template["scene"]["memory_regions"], key=lambda r: r["address_u64"])
        self._starts = [r["address_u64"] for r in regions]
        self._regions = regions
        self._span = max(len(r["value_hex"]) // 2 for r in regions)
        self._byte = {}  # template (value, known) per byte, for addresses candidates write
        self.memo = []  # [(read set, {key: result})]; a template change invalidates every entry

    def add_region(self, name, address, size, provenance):
        """Map a synthetic zero-filled writable region (e.g. a constructed status-map node) that overlaps nothing.

        Its contents come from candidate overrides; the provenance says what the region stands for."""
        end = address + size
        k = bisect.bisect_right(self._starts, end) - 1
        while k >= 0 and self._starts[k] + self._span > address:
            r = self._regions[k]
            if r["address_u64"] < end and address < r["address_u64"] + len(r["value_hex"]) // 2:
                raise ValueError(f"synthetic region {name} overlaps {r['name']}")
            k -= 1
        self.template["scene"]["memory_regions"].append({"name": name, "address_u64": address, "value_hex": "00" * size,
                                                         "known_hex": "ff" * size, "writable": True,
                                                         "provenance": {"grade": "synthetic", **provenance}})
        self._index()

    def _template_byte(self, address):
        if address not in self._byte:
            k = bisect.bisect_right(self._starts, address) - 1
            found = None
            while k >= 0:  # regions may overlap; the first region holding the byte is the one write() edits first
                r = self._regions[k]
                if r["address_u64"] <= address < r["address_u64"] + len(r["value_hex"]) // 2:
                    at = address - r["address_u64"]
                    found = (int(r["value_hex"][2 * at:2 * at + 2], 16), int(r["known_hex"][2 * at:2 * at + 2], 16))
                if r["address_u64"] + self._span < address:
                    break
                k -= 1
            self._byte[address] = found
        return self._byte[address]

    def writes(self, pieces):
        """Byte writes of one candidate, in the order rank_outfits.py applies them, plus model facts."""
        m = self.model
        if m.base_model is None:  # observe-only: the captured outfit as worn
            if pieces:
                raise Unknown("observe-only evaluator cannot predict candidates")
            return {}, {}
        pred = m.predict(pieces)
        out = {}
        for address, actual, name in ((m.stat_addr, m.stat, "stat"), (m.d_addr, m.d, "d")):
            for at in pred[name + "_changed_words"]:
                for i, b in enumerate(pred[name][at:at + 4]):
                    out[address + at + i] = b
        if any(0x24 <= a < 0x64 for a in pred["stat_changed_words"]):
            r_old, r_new = m.stat[0x24:0x64], pred["stat"][0x24:0x64]
            u = lambda b, o: struct.unpack_from("<I", b, o)[0]
            packet = {8: (u(m.main, 8) + (u(r_new, 0) + u(r_new, 8)) - (u(r_old, 0) + u(r_old, 8))) & 0xFFFFFFFF,
                      0xC: u(r_new, 0xC), 0x14: u(r_new, 0x18)}
            for off, value in packet.items():
                for i, b in enumerate(struct.pack("<I", value)):
                    out[m.main_addr + off + i] = b
        if self.buff is not None:
            for i, b in enumerate(struct.pack("<f", self.buff)):
                out[self.owner_addr + 0x2E3C + i] = b
        # Bytes outside every template region are dropped by rank_outfits.write(); they cannot reach the replay.
        out = {a: b for a, b in out.items() if self._template_byte(a) is not None}
        facts = {"weight": struct.unpack_from("<I", pred["stat"], 0x7B8)[0],
                 "load_factor": struct.unpack_from("<f", pred["stat"], 0x7A0)[0],
                 "set_rewards": pred["set_rewards"], "approximate_words": pred["approximate_words"]}
        return out, facts

    def _key(self, writes, reads):
        return frozenset((a, b) for a, b in writes.items() if a in reads and (b, 0xFF) != self._template_byte(a))

    def _request(self, writes):
        request = json.loads(json.dumps(self.template))
        regions = request["scene"]["memory_regions"]
        index = sorted(range(len(regions)), key=lambda i: regions[i]["address_u64"])
        starts = [regions[i]["address_u64"] for i in index]
        edits = {}
        for address, b in writes.items():  # every region holding the byte is edited, like rank_outfits.write()
            k = bisect.bisect_right(starts, address) - 1
            while k >= 0 and starts[k] + self._span >= address:
                r = regions[index[k]]
                at = address - r["address_u64"]
                if 0 <= at < len(r["value_hex"]) // 2:
                    edits.setdefault(index[k], []).append((at, b))
                k -= 1
        for i, changes in edits.items():
            value, known = bytearray.fromhex(regions[i]["value_hex"]), bytearray.fromhex(regions[i]["known_hex"])
            for at, b in changes:
                value[at], known[at] = b, 0xFF
            regions[i]["value_hex"], regions[i]["known_hex"] = value.hex(), known.hex()
        return request

    def evaluate(self, pieces, overrides=None):
        """pieces: [(E8 record, physical slot)] -> {"damage": float|None, "status", "memo": bool, facts...}.

        overrides: optional {address: bytes} written after the candidate (counterfactual state such as a trigger).
        The result's "reads" is the read set of the replay that produced it (shared with memo hits)."""
        global READS
        try:
            writes, facts = self.writes(pieces)
        except (Unknown, ValueError, KeyError) as error:
            self.stats["unknown"] += 1
            return {"damage": None, "status": "UNKNOWN", "reason": str(error)[:300], "memo": False}
        for address, data in (overrides or {}).items():
            for i, b in enumerate(data):
                if self._template_byte(address + i) is not None:
                    writes[address + i] = b
        for reads, table in self.memo:
            hit = table.get(self._key(writes, reads))
            if hit is not None:
                self.stats["memo_hits"] += 1
                return {**hit, **facts, "memo": True, "reads": reads}
        started = time.time()
        READS = set()
        try:
            result = self.replay._execute_request(self._request(writes), self.cache)
        finally:
            reads, READS = READS, None
        self.stats["replays"] += 1
        self.stats["replay_seconds"] += time.time() - started
        out = result["output_record_not_automatically_success"]
        gate = result["score_acceptance"]
        ok = all(gate[k] for k in ("normal_return", "nonvolatile_restoration", "full_original_score_store",
                                   "executed_epilogue_status", "finite_score"))
        hit = {"damage": struct.unpack("<f", bytes.fromhex(out["value_hex"]))[0] if ok else None,
               "status": "computed" if ok else "UNKNOWN",
               "reason": None if ok else {"next_rva": result["native"]["next_rva"], "missing": result["last_missing_read"]}}
        frozen = frozenset(reads)
        for known, table in self.memo:
            if known == frozen:
                table[self._key(writes, known)] = hit
                break
        else:
            self.memo.append((frozen, {self._key(writes, frozen): hit}))
        return {**hit, **facts, "memo": False, "reads": frozen}

    def confirm(self, pieces):
        """Full v1 acceptance path (run_request: replay plus the initial-score ablation control)."""
        writes, facts = self.writes(pieces)
        result = self.replay.run_request(self._request(writes), self.cache)
        return {"damage": result["computed_damage_f32"], "accepted": result["score_acceptance"]["computed_score_accepted"], **facts}
