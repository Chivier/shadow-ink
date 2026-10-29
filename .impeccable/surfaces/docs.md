# Usage documentation

MODE: Read.

## Scope and visual authority

The user requested a simple GitHub Pages usage and parameter guide maintained in `docs/`. The published root serves documentation; `/demo/` preserves the interactive public sample film. This is a reading surface inside the incumbent charcoal, coral, mint, Sora and JetBrains Mono world. It does not replace the film identity or expand root `DESIGN.md` and `.impeccable/design.json` into a general application system.

A plain documentation shell carries a sticky section index, readable prose, command blocks and contained reference tables. The first section opens directly with documentation and a runnable command. There is no promotional hero or concept reset.

## Extracted surface decisions

**Palette inheritance.** `docs/site.css` reuses the existing background (`--bg: #111313`), HTML surface (`--panel: #191d1d`), ink (`#f2eee7`), muted (`#a3b1ac`), line (`#34413c`), coral (`--accent: #ff876e`) and mint (`--mint: #a9d9ba`). The Canvas-only panel token is not needed here. Coral identifies links, navigation state and focus; mint distinguishes inline code. Mint text in the guide is syntax emphasis, not evidence of a completed transfer. Fine rules and tonal surfaces provide depth without shadows.

**Reading typography.** Both faces are self-hosted through `demo/assets/fonts/` relative to the published documentation root, with `font-display: swap`. Sora body text is 15px/1.8, distinct from the film player's 14px/1.65 supporting text. The lead is 19px/1.7. Heading sizes are 38px, 27px and 18px for h1–h3, at weight 600, line-height 1.3 and letter-spacing −0.025em. Prose is capped at 75ch. JetBrains Mono code is 13px/1.65; code blocks use a 1.8 line-height. Navigation, labels and table text are 13px; small counts and the Copy action have their own 12px and 11px roles. These are local reading roles, not replacements for Canvas display sizes.

**Layout and responsive behavior.** The desktop layout is capped at 1260px with 24px side padding, a 234px index column, a 68px gap and a main reading column capped at 820px. The 76px topbar and section index are sticky. At 900px and below, the index becomes 202px and the column gap 35px. At 700px and below, the layout becomes one column with 20px inset, a 68px topbar, and a Sections button revealing the inline index. Mobile h1/h2/lead become 30px/24px/17px; code blocks become 12px. Tables and command blocks scroll within their own containers rather than widening the page. Reduced motion disables smooth scrolling.

**Shapes and controls.** Controls and navigation retain the incumbent 6px radius. Inline code has a local 3px radius; code blocks use 10px, with padding that reserves room for Copy. Search and select controls retain 44px minimum height. Utility buttons use 40px, and the inset Copy button uses 30px; these local Read controls differ from the film's 44px transport controls. Focus uses a 2px coral outline with 4px offset. The skip link preserves dark text on coral in its focus and hover states. These extracted radius and size roles are scoped to this guide.

## Content and interaction

Parameter rows, version text and default command help render at build time so useful content survives disabled JavaScript. The script adds option filtering with a live count and empty state, command-reference switching with loading/failure/retry feedback, code copying with selection fallback, and mobile navigation. The menu exposes its expanded state, closes after a section selection, and returns focus to its trigger on Escape. Current section links expose `aria-current="location"`. Inputs have visible labels; the skip link reaches the main reading region.

CLI flags retain their literal `--` prefixes. The guide distinguishes configured frame rates and choreographed demo timing from measured throughput. Product facts and exact CLI help come from their source material, not from this surface brief.

## Source and publication paths

- `docs/index.html`: semantic source template, sections, labels, no-JavaScript fallback and build insertion markers.
- `docs/site.css`: actual palette mappings, typography, shapes, layout and breakpoints above.
- `docs/site.js`: navigation, filtering, command help loading, copy interaction and failure handling.
- `docs/cli-reference.json` and `docs/CLI.md`: captured CLI help; refreshed by `scripts/update-cli-reference.py`.
- `docs/README.md`: site authoring/build instructions.
- `scripts/build-site.py`: publishes the documentation at the site root, injects parameter rows/default help/version and preserves the film at `/demo/`. It copies tracked `docs/` and `demo/` files plus an explicit six-file documentation source allowlist, excluding local reference images and rendered videos.
- `.github/workflows/demo.yml`: GitHub Pages build/deployment workflow.

## Comparison evidence

Compared `docs/site.css`, `docs/index.html` and `docs/site.js` against `PRODUCT.md` and the existing root `DESIGN.md`. Palette values and font families inherit directly; typography sizes, inline-code/block radii, utility-control heights and responsive breakpoints above are documented as local Read decisions. Root design files remain unchanged.

Inspected `.impeccable/review/docs/desktop.png`, `parameters.png` and `mobile.png`: they show the desktop section index and reading column, the parameter reference, and the single-column mobile reading surface. The recorded browser checks in `acceptance-output/readme-base64-20261029/docs-checks.json` report no errors, 14 static parameters, and passes for filter empty/reset, command switching, clipboard, mobile menu/overflow, reference fetch failure/retry, no-JavaScript content and the preserved public sample demo. This comparison records the supplied capture and check evidence; it does not claim a new browser test run or deployment verification.
