"""Where one prediction spends its time, with and without the read-set memo. argv: layers.json"""
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from state_predict import ITEM, Predictor, W  # noqa: E402

p = Predictor(sys.argv[1])
live = Path(W + "live-1106/core2/attr_1b63eb628d0.bin").read_bytes()
worn = [p.core(live[0x80 + s * ITEM:0x80 + (s + 1) * ITEM]) for s in range(2)]
spent = {"pipeline": 0.0, "evaluate": 0.0}
original_pipeline, original_eval = p._pipeline, p.ev.evaluate


def timed(name, f):
    def run(*a, **k):
        t = time.perf_counter()
        try:
            return f(*a, **k)
        finally:
            spent[name] += time.perf_counter() - t
    return run


p._pipeline = timed("pipeline", original_pipeline)
p.ev.evaluate = timed("evaluate", original_eval)
sets = [[37, 150, 27, 26, 80, 15, 150], [38, 149, 27, 26, 80, 15, 150], [37, 151, 27, 26, 79, 15, 150], [37, 150, 27, 27, 80, 15, 149],
        [36, 150, 28, 26, 80, 15, 150], [37, 152, 27, 26, 80, 14, 149], [40, 150, 27, 26, 77, 15, 150], [37, 155, 27, 26, 80, 15, 145]]
t0 = time.perf_counter()
for a in sets * 3:
    p.predict("blind-1023/h1", a, None, worn)
total = time.perf_counter() - t0
n = len(sets) * 3
print(f"calls {n} total {total:.2f}s per call {total / n * 1000:.0f} ms", {k: f"{v / n * 1000:.0f} ms" for k, v in spent.items()})
