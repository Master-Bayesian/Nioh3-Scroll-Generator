"""Import the public PC202 action index and labels from its linked scene descriptors."""
import argparse
import json
from pathlib import Path


def import_catalog(source, output):
    catalog = json.loads((source / "catalog.json").read_bytes())
    if catalog["version"] != 1:
        raise ValueError("Unsupported action catalog")
    for row in catalog["selections"]:
        scenes = []
        for scene_id in row["scene_ids"]:
            descriptor = json.loads((source / scene_id / "bundle.json").read_bytes())
            if descriptor["id"] != scene_id or descriptor["school"] != row["school"]:
                raise ValueError(f"Scene identity mismatch for {scene_id}")
            binding = descriptor.get("resource_action")
            if binding and any(binding[key] != row[key] for key in ("resource", "action", "parameter_sha256")):
                raise ValueError(f"Scene action mismatch for {scene_id}")
            scenes.append(descriptor)
        # Buff-specific scenes may share a single action. Only fill missing
        # labels when every explicitly linked descriptor agrees.
        for field in ("attack_id", "stance", "name"):
            values = {s["damage"] if field == "name" else s["scene"][field] for s in scenes}
            if row[field] is None and len(values) == 1:
                row[field] = values.pop()
    with output.open("w", encoding="utf-8", newline="\n") as stream:
        stream.write('{"version":1,"selections":[\n')
        for index, row in enumerate(catalog["selections"]):
            if index:
                stream.write(",\n")
            stream.write(json.dumps(row, ensure_ascii=False, separators=(",", ":")))
        stream.write("\n]}\n")


if __name__ == "__main__":
    args = argparse.ArgumentParser(description=__doc__)
    args.add_argument("source", type=Path, help="Reviewed loadout tree containing catalog.json and scenes")
    args.add_argument("output", type=Path)
    options = args.parse_args()
    import_catalog(options.source, options.output)
