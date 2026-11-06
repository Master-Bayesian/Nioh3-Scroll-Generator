"""The mistaken attribute state (5,5,5,36,6,5,140) hit 849: run the model on it. Not blind, the state was read first. argv: layers out"""
import json
import struct
import sys
from pathlib import Path

SRC = "F:/Nioh3_r16_build/worktrees/research-damage/research/damage/evaluator/src"
sys.path.insert(0, SRC)
sys.argv = ["cross_state", sys.argv[1], sys.argv[2]]
text = Path(SRC + "/cross_state.py").read_text(encoding="utf-8").replace("Path(__file__)", "Path(SRC + '/cross_state.py')")
exec(compile(text[:text.index("frozen = {")], "cross_state_head", "exec"))
A = [5, 5, 5, 36, 6, 5, 140]
print("PRED", worn(849, attrs=A, label="mistaken"))
live_attr = Path(W + "live-1106/attr-a2/attr.bin").read_bytes()
live_stat = Path(W + "live-1106/attr-a2/stat.bin").read_bytes()
live_d = Path(W + "live-1106/attr-a2/D.bin").read_bytes()
t = ComposedModel(CACHE, W + "blind-1023/h1/capture.json")
t.raw_attrs, t.sync_attrs = list(A), list(A)
s_t, d_t, source, gain = pipeline(t, frame, WORN + ["state_1023"])
val = lambda s: {e["key_u16"]: e["base_u32"] + e["duplicate_u16"] for e in s["map_35d0"]}
live = val(base.B.typed_source(live_attr, b"\xff" * len(live_attr), 0))
got = val(source)
print("groups live", len(live), "model", len(got))
print("group diff (live, model)", {hex(g): (live.get(g), got.get(g)) for g in set(live) | set(got) if live.get(g) != got.get(g)})
print("record model", struct.unpack_from("<I", s_t, 0x24)[0], "+gain", gain + CORE2["attack"], "live", struct.unpack_from("<I", live_stat, 0x24)[0])
print("weight/load model", struct.unpack_from("<I", s_t, 0x7B8)[0], struct.unpack_from("<f", s_t, 0x7A0)[0], "live", struct.unpack_from("<I", live_stat, 0x7B8)[0], struct.unpack_from("<f", live_stat, 0x7A0)[0])
