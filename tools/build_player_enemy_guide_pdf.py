"""Build the Simplified-Chinese player enemy-combination guide PDF."""

from __future__ import annotations

import argparse
import json
from collections import defaultdict
from pathlib import Path
from typing import Iterable

from reportlab.lib import colors
from reportlab.lib.enums import TA_CENTER, TA_LEFT
from reportlab.lib.pagesizes import A4
from reportlab.lib.styles import ParagraphStyle, getSampleStyleSheet
from reportlab.lib.units import mm
from reportlab.pdfbase import pdfmetrics
from reportlab.pdfbase.ttfonts import TTFont
from reportlab.platypus import (
    KeepTogether,
    Paragraph,
    SimpleDocTemplate,
    Spacer,
    Table,
    TableStyle,
)


REPOSITORY_ROOT = Path(__file__).resolve().parents[1]
DEFAULT_ROLE_CATALOG = (
    REPOSITORY_ROOT
    / "docs"
    / "knowledge"
    / "versions"
    / "pc-v2.00.02"
    / "catalogs"
    / "enemy-roles.json"
)
DEFAULT_OUTPUT = (
    REPOSITORY_ROOT
    / "output"
    / "pdf"
    / "仁王3绘卷敌人组合指南_PC-v2.00.02.pdf"
)
REGULAR_FONT_PATH = Path(r"C:\Windows\Fonts\Deng.ttf")
BOLD_FONT_PATH = Path(r"C:\Windows\Fonts\Dengb.ttf")


def _register_fonts() -> None:
    pdfmetrics.registerFont(TTFont("PlayerGuide", str(REGULAR_FONT_PATH)))
    pdfmetrics.registerFont(TTFont("PlayerGuideBold", str(BOLD_FONT_PATH)))


def _unique_names_by_role(payload: dict[str, object]) -> dict[int, list[str]]:
    names: dict[int, set[str]] = defaultdict(set)
    for row in payload["rows"]:  # type: ignore[index]
        role = int(row["role"])
        name = str(row["names"]["zh-CN"])
        names[role].add(name)
    return {role: sorted(values) for role, values in names.items()}


def _chunks(values: list[str], size: int) -> Iterable[list[str]]:
    for index in range(0, len(values), size):
        yield values[index : index + size]


def _header_footer(canvas: object, document: object) -> None:
    canvas.saveState()
    width, height = A4
    canvas.setStrokeColor(colors.HexColor("#D7DEE8"))
    canvas.setLineWidth(0.5)
    canvas.line(18 * mm, 14 * mm, width - 18 * mm, 14 * mm)
    canvas.setFont("PlayerGuide", 8)
    canvas.setFillColor(colors.HexColor("#6B7280"))
    canvas.drawString(
        18 * mm,
        9 * mm,
        "仁王3绘卷敌人组合指南 · MasterBayesian & Saber_Li",
    )
    canvas.drawRightString(
        width - 18 * mm,
        9 * mm,
        f"第 {document.page} 页",
    )
    canvas.restoreState()


def _name_table(names: list[str], body_style: ParagraphStyle) -> Table:
    columns = 3
    cells = [Paragraph(name, body_style) for name in names]
    rows = []
    for chunk in _chunks(cells, columns):
        rows.append(chunk + [""] * (columns - len(chunk)))
    table = Table(rows, colWidths=[54 * mm] * columns, hAlign="LEFT")
    table.setStyle(
        TableStyle(
            [
                ("FONTNAME", (0, 0), (-1, -1), "PlayerGuide"),
                ("FONTSIZE", (0, 0), (-1, -1), 8.5),
                ("TEXTCOLOR", (0, 0), (-1, -1), colors.HexColor("#1F2937")),
                ("BACKGROUND", (0, 0), (-1, -1), colors.HexColor("#F8FAFC")),
                ("GRID", (0, 0), (-1, -1), 0.35, colors.HexColor("#D7DEE8")),
                ("VALIGN", (0, 0), (-1, -1), "MIDDLE"),
                ("LEFTPADDING", (0, 0), (-1, -1), 6),
                ("RIGHTPADDING", (0, 0), (-1, -1), 6),
                ("TOPPADDING", (0, 0), (-1, -1), 4),
                ("BOTTOMPADDING", (0, 0), (-1, -1), 4),
            ]
        )
    )
    return table


def build_pdf(role_catalog: Path, output: Path) -> None:
    _register_fonts()
    payload = json.loads(role_catalog.read_text(encoding="utf-8"))
    names_by_role = _unique_names_by_role(payload)

    palette = {
        "ink": colors.HexColor("#172033"),
        "muted": colors.HexColor("#526071"),
        "blue": colors.HexColor("#2456A6"),
        "blue_soft": colors.HexColor("#EAF1FC"),
        "gold": colors.HexColor("#C88718"),
        "gold_soft": colors.HexColor("#FFF6E3"),
        "green": colors.HexColor("#2F7D5B"),
        "green_soft": colors.HexColor("#EAF7F1"),
        "line": colors.HexColor("#D7DEE8"),
    }
    styles = getSampleStyleSheet()
    title = ParagraphStyle(
        "GuideTitle",
        parent=styles["Title"],
        fontName="PlayerGuideBold",
        fontSize=24,
        leading=31,
        textColor=palette["ink"],
        alignment=TA_CENTER,
        spaceAfter=8,
    )
    subtitle = ParagraphStyle(
        "GuideSubtitle",
        parent=styles["Normal"],
        fontName="PlayerGuide",
        fontSize=11,
        leading=17,
        textColor=palette["muted"],
        alignment=TA_CENTER,
    )
    heading = ParagraphStyle(
        "GuideHeading",
        parent=styles["Heading2"],
        fontName="PlayerGuideBold",
        fontSize=15,
        leading=20,
        textColor=palette["blue"],
        spaceBefore=8,
        spaceAfter=7,
    )
    subheading = ParagraphStyle(
        "GuideSubheading",
        parent=styles["Heading3"],
        fontName="PlayerGuideBold",
        fontSize=11.5,
        leading=16,
        textColor=palette["ink"],
        spaceBefore=7,
        spaceAfter=5,
    )
    body = ParagraphStyle(
        "GuideBody",
        parent=styles["BodyText"],
        fontName="PlayerGuide",
        fontSize=9.5,
        leading=15,
        textColor=palette["ink"],
        alignment=TA_LEFT,
        spaceAfter=5,
    )
    compact = ParagraphStyle(
        "GuideCompact",
        parent=body,
        fontSize=8.5,
        leading=12,
        spaceAfter=0,
    )
    callout = ParagraphStyle(
        "GuideCallout",
        parent=body,
        fontName="PlayerGuideBold",
        fontSize=10,
        leading=16,
        textColor=palette["blue"],
        leftIndent=6,
        rightIndent=6,
        spaceBefore=5,
        spaceAfter=5,
    )

    output.parent.mkdir(parents=True, exist_ok=True)
    document = SimpleDocTemplate(
        str(output),
        pagesize=A4,
        leftMargin=18 * mm,
        rightMargin=18 * mm,
        topMargin=18 * mm,
        bottomMargin=20 * mm,
        title="仁王3绘卷敌人组合指南",
        author="MasterBayesian & Saber_Li",
        subject="PC v2.00.02 player enemy-combination guide",
    )
    story: list[object] = [
        Spacer(1, 14 * mm),
        Paragraph("仁王3绘卷敌人组合指南", title),
        Paragraph("PC v2.00.02 · 中文玩家版", subtitle),
        Spacer(1, 7 * mm),
        Table(
            [
                [Paragraph("这份指南解决什么问题？", callout)],
                [
                    Paragraph(
                        "告诉你哪些敌人可以组合在同一张绘卷里，以及软件中的“必含”和“任一组”应该怎么用。后面的敌人目录只列原生绘卷生成池中真实可用的敌人。",
                        body,
                    )
                ],
            ],
            colWidths=[164 * mm],
            style=TableStyle(
                [
                    ("BACKGROUND", (0, 0), (-1, 0), palette["blue_soft"]),
                    ("BACKGROUND", (0, 1), (-1, 1), colors.white),
                    ("BOX", (0, 0), (-1, -1), 0.8, palette["blue"]),
                    ("LEFTPADDING", (0, 0), (-1, -1), 10),
                    ("RIGHTPADDING", (0, 0), (-1, -1), 10),
                    ("TOPPADDING", (0, 0), (-1, -1), 8),
                    ("BOTTOMPADDING", (0, 0), (-1, -1), 8),
                ]
            ),
        ),
        Spacer(1, 7 * mm),
        Paragraph("先记住两种选择方式", heading),
        Table(
            [
                [Paragraph("必含", subheading), Paragraph("同一张绘卷里必须出现这个敌人。多个必含条件需要全部满足。", body)],
                [Paragraph("任一组", subheading), Paragraph("把几个都能接受的敌人放进同一个任一组，最终出现其中任意一个即可。不同任一组之间仍要同时满足。", body)],
            ],
            colWidths=[30 * mm, 134 * mm],
            style=TableStyle(
                [
                    ("BACKGROUND", (0, 0), (0, -1), palette["gold_soft"]),
                    ("BOX", (0, 0), (-1, -1), 0.6, palette["line"]),
                    ("INNERGRID", (0, 0), (-1, -1), 0.35, palette["line"]),
                    ("VALIGN", (0, 0), (-1, -1), "TOP"),
                    ("LEFTPADDING", (0, 0), (-1, -1), 8),
                    ("RIGHTPADDING", (0, 0), (-1, -1), 8),
                    ("TOPPADDING", (0, 0), (-1, -1), 6),
                    ("BOTTOMPADDING", (0, 0), (-1, -1), 6),
                ]
            ),
        ),
        Spacer(1, 6 * mm),
        Paragraph("三种原生组合结构", heading),
    ]

    class_rows = [
        ["结构", "可使用的 role", "玩家需要知道的限制"],
        ["Class 0", "role 4 / 5", "只放专用池敌人，共 2 或 3 项；纯 role 4 最多 2 项，最高位置需要 role 5。"],
        ["Class 1", "role 0-3 + 最多一个 role 5", "普通敌人为主；整张绘卷最多要求一个 role 5。"],
        ["Class 2", "role 0-3", "只使用普通敌人，不会出现 role 4/5。"],
    ]
    class_table = Table(
        [
            [Paragraph(str(value), compact) for value in row]
            for row in class_rows
        ],
        colWidths=[24 * mm, 52 * mm, 88 * mm],
        repeatRows=1,
    )
    class_table.setStyle(
        TableStyle(
            [
                ("BACKGROUND", (0, 0), (-1, 0), palette["blue"]),
                ("TEXTCOLOR", (0, 0), (-1, 0), colors.white),
                ("FONTNAME", (0, 0), (-1, 0), "PlayerGuideBold"),
                ("GRID", (0, 0), (-1, -1), 0.45, palette["line"]),
                ("VALIGN", (0, 0), (-1, -1), "TOP"),
                ("LEFTPADDING", (0, 0), (-1, -1), 6),
                ("RIGHTPADDING", (0, 0), (-1, -1), 6),
                ("TOPPADDING", (0, 0), (-1, -1), 6),
                ("BOTTOMPADDING", (0, 0), (-1, -1), 6),
            ]
        )
    )
    story.extend(
        [
            class_table,
            Spacer(1, 5 * mm),
            Paragraph("快速判断组合是否合法", heading),
        ]
    )

    quick_rows = [
        ["目标组合", "结果"],
        ["只有 role 0-3", "可以，走 Class 1 或 Class 2。"],
        ["role 0-3 + 一个 role 5", "可以，走 Class 1。"],
        ["role 0-3 + role 4", "不可以。"],
        ["role 0-3 + 两个或更多 role 5", "不可以。"],
        ["只选 role 4/5，合计不超过 3 项，纯 role 4 不超过 2 项", "可以，走 Class 0。"],
        ["三个纯 role 4", "不可以。"],
        ["四项或更多 role 4/5", "不可以。"],
    ]
    quick_table = Table(
        [[Paragraph(str(value), compact) for value in row] for row in quick_rows],
        colWidths=[108 * mm, 56 * mm],
        repeatRows=1,
    )
    quick_table.setStyle(
        TableStyle(
            [
                ("BACKGROUND", (0, 0), (-1, 0), palette["green"]),
                ("TEXTCOLOR", (0, 0), (-1, 0), colors.white),
                ("FONTNAME", (0, 0), (-1, 0), "PlayerGuideBold"),
                ("ROWBACKGROUNDS", (0, 1), (-1, -1), [colors.white, palette["green_soft"]]),
                ("GRID", (0, 0), (-1, -1), 0.45, palette["line"]),
                ("VALIGN", (0, 0), (-1, -1), "TOP"),
                ("LEFTPADDING", (0, 0), (-1, -1), 6),
                ("RIGHTPADDING", (0, 0), (-1, -1), 6),
                ("TOPPADDING", (0, 0), (-1, -1), 5),
                ("BOTTOMPADDING", (0, 0), (-1, -1), 5),
            ]
        )
    )
    story.extend(
        [
            quick_table,
            Spacer(1, 4 * mm),
            Paragraph(
                "注意：表格用于提前排除结构上必定无解的组合。结构上可以组合，不代表一定很快找到 Seed；软件仍会完整生成并验证每个候选。",
                body,
            ),
            Paragraph("例子：为什么“一目连 + 两名德川”不成立", heading),
            Paragraph(
                "一目连属于普通池；德川国松和德川庆喜属于 role 5。Class 0 放不下一目连，Class 1 最多只能要求一个 role 5，Class 2 又不能放两名德川，所以三者不能同时作为必含条件。若两名德川任选其一，可以把二者放入同一个任一组，再把一目连设为必含。",
                body,
            ),
            Paragraph("可生成敌人目录", title),
            Paragraph(
                "以下只列游戏原生绘卷候选池中的敌人。一个名称如果有多个原生变体，可能同时出现在多个 role 章节；软件会自动考虑全部合法变体。",
                subtitle,
            ),
            Spacer(1, 5 * mm),
        ]
    )

    role_descriptions = {
        0: "普通池之一，可用于 Class 1 和 Class 2。",
        1: "普通池之一，可用于 Class 1 和 Class 2。",
        2: "普通池之一，可用于 Class 1 和 Class 2。",
        3: "普通池之一，可用于 Class 1 和 Class 2。",
        4: "专用池 A，只能用于 Class 0；纯 role 4 最多要求两个。",
        5: "专用池 B，可用于 Class 0；与普通池组合时，Class 1 最多要求一个。",
    }
    for role in range(6):
        names = names_by_role.get(role, [])
        story.append(
            KeepTogether(
                [
                    Paragraph(f"Role {role}（{len(names)} 个名称）", heading),
                    Paragraph(role_descriptions[role], body),
                    _name_table(names, compact),
                    Spacer(1, 4 * mm),
                ]
            )
        )

    document.build(story, onFirstPage=_header_footer, onLaterPages=_header_footer)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--role-catalog", type=Path, default=DEFAULT_ROLE_CATALOG)
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    args = parser.parse_args()
    build_pdf(args.role_catalog.resolve(), args.output.resolve())
    print(f"Built player guide: {args.output.resolve()}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
