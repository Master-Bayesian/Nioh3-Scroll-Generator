# Nioh 3 Scroll Generator UI Design System

## Reference concepts

- `design/concepts/canonical-search.png`
- `design/concepts/local-editor-backups.png`

The concepts define layout and visual hierarchy only. All visible controls and
text remain code-native. Generated sample values are not product data.

## Product structure

- One persistent application shell.
- Two primary destinations only: canonical search and local scroll editing.
- Backup management stays inside local editing.
- Canonical search prioritizes filter inputs, intersection progress, candidate
  comparison, and selected-record inspection.
- Local editing prioritizes the physical inventory, ordered effect slots, raw
  field editing, and recoverable backup operations.

## Tokens

| Token | Value | Purpose |
|---|---:|---|
| `canvas` | `#0B0D0F` | Window and navigation background |
| `surface` | `#121518` | Main panels |
| `surfaceRaised` | `#181C20` | Inputs, selected subpanels |
| `surfaceSelected` | `#20375E` | Selected navigation and rows |
| `border` | `#343A40` | Panel and table dividers |
| `text` | `#EEE7D6` | Primary text |
| `textMuted` | `#A7ABB0` | Secondary text |
| `gold` | `#D6A64B` | Canonical, verified, and important states |
| `blue` | `#4F73B8` | Primary selection and action |
| `green` | `#48A86B` | Offline-ready status |
| `danger` | `#C95252` | Destructive actions and local-only warnings |

## Typography

- UI family: `Microsoft YaHei UI`, with `Segoe UI` fallback.
- Application title: 17 px equivalent, semibold.
- Section title: 11 px equivalent, semibold.
- Controls and table rows: 10 px equivalent.
- Metadata and status: 9 px equivalent.
- IDs and hashes use the same family to preserve Chinese fallback and avoid
  mixed baseline rendering.

## Geometry

- Default window: 1440 x 920; minimum: 1120 x 760.
- Header: 54 px.
- Navigation rail: 188 px.
- Global spacing scale: 4, 8, 12, 16, 24 px.
- Panels use one-pixel borders and restrained two-to-four-pixel corner radii.
- Tables and split panes are preferred over nested cards.

## Component rules

- Primary buttons use muted indigo, not saturated blue.
- Canonical status and counts use gold; ready status uses green.
- Destructive actions are outlined red and separated from primary actions.
- Search and local editing use the same table, input, scrollbar, and inspector
  treatments.
- Selection must remain visible with keyboard focus removed.
- Disabled controls retain readable text contrast.

## Motion and accessibility

- No decorative animation.
- Long-running operations communicate through status text and progress counts.
- Keyboard focus remains visible.
- Color is never the only distinction for destructive or canonical states.
