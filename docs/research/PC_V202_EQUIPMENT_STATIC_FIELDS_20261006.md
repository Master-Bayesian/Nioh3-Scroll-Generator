# PC v2.02 Equipment Static Weight and Requirement Fields

## Scope and evidence

This read-only study exports armor weight and minimum-stat fields from the PC
v2.02 `item.bin` resource already tracked in this repository. It joins item
names from the repository's `apps/workshop/item-names.json` catalog, which is
declared as PC v2.01 / zh-CN. The source catalog's upstream workbook is not part
of this handoff, so catalog names are identifying context rather than
independently revalidated v2.02 facts.

The supplied `item.bin` was checked against the repository resource by SHA-256
and matched. The binary is not duplicated in this research package. Reproduce
the export from a repository checkout with the project Python wrapper:

```powershell
$extractorArgs = [string[]]@(
  '--item-bin',
  'nioh3_scroll_editor/data/r4_finalizer/pc_v2_02/resource_v1/tables/item.bin',
  '--out',
  'D:/Nioh3_v080_deliverables/equipment-static-v202-repro'
)
pwsh -File tools/run_python_tests.ps1 `
  -ScriptPath research/equipment_static_v202/research_extract_equipment_static_v202.py `
  -MinimumFreeGiB 0 `
  -ScriptArgument $extractorArgs
```

The script reads the checked-in name catalog by default and writes only to the
requested output directory. `SHA256SUMS.txt` records this handoff's files.

## Findings

### Current armor weight

For current armor rows (`type_class` 24 through 38), base weight is stored at
`row + 0x98` as an unsigned 32-bit little-endian integer. The displayed value
is `raw / 10`.

### Minimum-stat requirements

Five two-byte fields begin at `row + 0x18C`, `0x18E`, `0x190`, `0x192`, and
`0x194`, corresponding to Body, Heart, Stamina, Strength, and Skill. For the
standard requirement, the low byte is the value; do not interpret the whole
word as a requirement. The high byte is preserved as `req_*_aux` and remains
unexplained.

For example, the `盗贼轻铠` mask row has bytes `06 03` at `+0x18C`: the standard
requirement is Body 6, not 774 (`0x0306`). This corrects the supplied guidance's
interpretation of the fields as five complete u16 values.

## Dataset checks

The input has an 8-byte header followed by 3,362 rows of 0x1A0 bytes. Its size is
1,398,600 bytes and SHA-256 is
`f680966b0a21ab64eadf31a26e80f83e2eeeaac1446e305bb87b9e64617b53ce`.

| Check | Result |
| --- | ---: |
| Rows / unique item IDs | 3,362 / 3,362 |
| Catalog-known rows | 3,169 |
| Current armor rows (`type_class` 24..38) | 757 |
| Named / blank-name current armor rows | 520 / 237 |
| Current armor rows with zero weight | 0 |
| Current armor weight range | 0.6–11.8 |
| Named armor with exactly two nonzero standard requirements | 520 / 520 |

Among the 193 IDs absent from the name catalog, all are weapons (173) or soul
cores (20). Some catalog-known armor rows have blank display names. The 237
blank-name armor rows have less regular requirement patterns (145 with one,
87 with two, and five with three nonzero requirements), so this research does
not mark them as planner candidates.

The table begins with legacy `type_class = 58` rows, including some items whose
names also appear on current equipment. Match records using item ID and class,
not display name alone. For example, `盗贼轻铠` mask has an old row (ID `0xE990`,
class 58, weight 1.0, Body 4 / Skill 5) and a current row (ID `0x8CC9`, class
24, weight 0.6, Body 6 / Skill 5).

## External reference spot checks

Eighteen rows were compared with the public
[Nioh 3 armor reference page](https://www.niohwiki.com/mediawiki/index.php?title=%E4%B8%89%E4%BB%A3%E9%98%B2%E5%85%B7%E8%B5%84%E6%96%99%E5%92%8C%E5%A5%97%E8%A3%85%E6%95%88%E6%9E%9C): five pieces each from `盗贼轻铠`, `足轻中铠`, and `传家大铠`, plus `猪前立头盔`, `爱字前立头盔`, and `龙头形盔`. Weight and minimum-stat fields matched for all 18 rows.

This is external-reference validation (`PASS_EXTERNAL_REFERENCE`), not live-game
screenshot or owner acceptance. Row IDs and details are recorded in
`research/equipment_static_v202/validation_samples_v202.csv`.

## Output files

- `equipment_static_data_v202.json`: source identity, field contract, summary
  statistics, and 757 current armor rows.
- `equipment_static_data_v202.csv`: current armor rows for review.
- `item_table_static_fields_v202_full.csv`: all 3,362 rows, including legacy
  and other classes; not a planner candidate list.
- `validation_samples_v202.csv`: 18 external-reference checks.
- `research_extract_equipment_static_v202.py`: read-only reproducible extractor.
- `SHA256SUMS.txt`: SHA-256 manifest for the report and research package.

## Evidence boundaries

Confirmed for PC v2.02 current armor: the binary layout, item ID at `+0x152`,
class at `+0x15C`, weight at `+0x98`, and the low-byte standard requirements.
The repository's existing `type_class` range 24..38 is used for filtering.

Open questions include the meaning of each requirement high byte, whether any
blank-name armor rows should be shown to users, and the business meaning of
`+0x98` for weapons, accessories, and other item classes. No equipment rules,
UI, game memory, or saves were modified. Any product integration remains a
separate maintainer decision.
