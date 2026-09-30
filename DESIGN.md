# melori design — «журнал практика»

| | |
|---|---|
| Identity | «Журнал практика»: страница бумажного рабочего журнала — линейки вместо карточек, serif для записей, mono для дат и таймкодов, один акцент |
| Tokens (single source) | `design/tokens.json` |
| Generator | `node scripts/build-tokens.mjs` → `src/theme/tokens.css` (main window, `@theme` + echo aliases), `src/meeting/tokens.css`, `src/overlay/tokens.css`; `--check` fails on a hand-edited output |
| Contrast gate | `node scripts/check-contrast.mjs` — WCAG 2.x, every pair in `tokens.json › contrast`, both themes |
| Themes | `day` (default) and `evening`, set as `data-theme` on `<html>` of every webview by `src/theme/theme.ts`; setting `ui_theme` = `system` \| `day` \| `evening` |
| Fonts | Literata (serif, variable, roman + italic), IBM Plex Sans (sans), IBM Plex Mono (mono) — local via `@fontsource`, never a font CDN |
| Reference artboards | `design/mockups/*.dc.html` (13 boards: clients, card, consent form, settings, onboarding, meeting day/evening/error, pill, empty and revoked states, icon) |
| Icon | `scripts/build-icon.py [--apply]` → `design/icon/*.svg` (letter in outlines, no font dependency) → `src-tauri/icons/`, tray states in `src-tauri/resources/tray_*.png` |

## Rules

1. **Colours come only from tokens.** No hex, `rgb()`/`rgba()` or `oklch()` in `src/**/*.css` outside the three generated `tokens.css` files (`src/theme/tokens.test.ts` enforces it for CSS); components use `var(--color-<role>)` or the Tailwind role classes (`bg-surface`, `text-ink-muted`, `border-rule`, `text-accent`, `bg-err-surface`…). A missing role is added to `tokens.json` and regenerated — never inlined.
2. **Roles, not shades.** `ground` / `surface` for paper, `ink` / `ink-muted` for text, `rule` / `rule-strong` / `edge-strong` for lines and field underlines, `accent` / `accent-hover` / `on-accent` for the one action colour (royal blue), `err*` and `warn*` for states. Evening is the same roles with other values — a component never branches on the theme.
3. **No radius on controls, panels and fields** (`--radius-none`). Round shapes are reserved for indicators (recording dot, progress ring).
4. **No glass, glow or decorative gradients.** Separation is done with rules and whitespace. A gradient is allowed only as a functional fill (e.g. playback progress).
5. **Motion is information, not decoration.** Short opacity transitions (`--motion-fast`); a spinner only where work is in progress; everything off under `prefers-reduced-motion`.
6. **Type.** Headlines and journal entries — serif; UI chrome — sans; dates, timecodes, status lines and counters — mono with tabular numerals. Timecodes and dates stay LTR inside RTL locales (`dir="ltr"` / `unicode-bidi: isolate`); user-provided names use `dir="auto"`.
7. **Every string through i18next**, present in all locale files; English and Russian are real translations. Consent text is not UI copy — it comes from the engine's versioned template.
8. **Consultation surfaces only.** The inherited echo modules stay in the tree but are not styled or shown; new visual work targets the consult journal, meeting panel, pill, settings and onboarding.

## Changing the design

Edit `design/tokens.json` → `npm run tokens:build` → `npm run contrast:check` → `npx vitest run src/theme`. For a new surface, draw the artboard first (`design/mockups/`), then build to it and compare in both themes, a long client alias and an RTL locale.
