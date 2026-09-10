"""Regenerate the CE layout from the accepted Python profile, without game access."""
import argparse
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))
from nioh3_scroll_editor.live_add_profile import PC_V201


def render(profile=PC_V201):
    lines = ['-- Generated from live_add_profile.PC_V201; do not edit addresses here.',
             'return {', 'profile_id="' + profile.profile_id + '",']
    for key, value in profile.lua_layout().items():
        lines.append(key + '=' + ('"' + value + '"' if isinstance(value, str) else hex(value)) + ',')
    return '\n'.join(lines + ['}']) + '\n'


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--output', type=Path, default=ROOT / 'research/live_add_layout_ce.lua')
    args = parser.parse_args()
    args.output.write_text(render(), encoding='utf-8')
