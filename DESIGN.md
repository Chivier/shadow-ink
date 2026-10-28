---
name: shadow-ink terminal film
description: A deterministic terminal projection reel grounded in real protocol frames.
colors:
  background: "#111313"
  canvas-panel: "#171b1a"
  surface: "#191d1d"
  ink: "#f2eee7"
  muted: "#a3b1ac"
  accent: "#ff876e"
  verified: "#a9d9ba"
  line: "#34413c"
typography:
  display:
    fontFamily: '"Sora", sans-serif'
    fontSize: "68px"
    fontWeight: 500
  brand:
    fontFamily: '"Sora", sans-serif'
    fontSize: "38px"
    fontWeight: 600
  body:
    fontFamily: '"Sora", sans-serif'
    fontSize: "14px"
    lineHeight: 1.65
  caption:
    fontFamily: '"Sora", sans-serif'
    fontSize: "20px"
  terminal:
    fontFamily: '"JetBrains Mono", monospace'
    fontSize: "18px"
    fontWeight: 400
  time:
    fontFamily: '"JetBrains Mono", monospace'
    fontSize: "12px"
rounded:
  control: "6px"
spacing:
  control-gap: "9px"
  chapter-gap: "10px"
  source-gap: "12px"
  transport-gap: "18px"
  control-padding: "10px 14px"
  select-padding: "8px 12px"
components:
  button-primary:
    backgroundColor: "{colors.accent}"
    textColor: "{colors.background}"
    rounded: "{rounded.control}"
    padding: "{spacing.control-padding}"
  button-secondary:
    backgroundColor: "transparent"
    textColor: "{colors.ink}"
    rounded: "{rounded.control}"
    padding: "{spacing.control-padding}"
  chapter:
    backgroundColor: "transparent"
    textColor: "{colors.muted}"
    rounded: "{rounded.control}"
    padding: "8px 13px"
  chapter-current:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.accent}"
    rounded: "{rounded.control}"
    padding: "8px 13px"
  image-selector:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.ink}"
    rounded: "{rounded.control}"
    padding: "{spacing.select-padding}"
  terminal-panel:
    backgroundColor: "{colors.canvas-panel}"
    textColor: "{colors.ink}"
---

# Design System: shadow-ink terminal film

## Overview

**Creative North Star: "Terminal projection reel"**

A dark terminal becomes the stage for a file moving between a remote sender and a local receiver. Quiet charcoal fields, warm lettering and fine structural rules keep attention on the image, protocol frames and verified arrival. Paired coral brackets identify the project; the transfer gutter connects the two panes.

This captures the implemented demo, not a broader application system. The approved experience is a static HTML/Canvas film with English captions, no narration and a deterministic 45-second timeline. Its composition and sequence belong to `.impeccable/surfaces/demo.md`; PRODUCT.md remains the authority for product capabilities and evidence.

**Key Characteristics:**

- One split terminal within a fluid 16:9 film.
- Self-hosted Sora for display and reading; JetBrains Mono for commands and data.
- Coral for transfer emphasis and interaction; mint for recovery and verified state.
- Real sample protocol frames with an explicitly choreographed image reveal.
- Native HTML playback controls, visible captions and a transcript outside Canvas.

## Colors

Warm ink and muted green-gray text sit above closely spaced charcoal surfaces. The frontmatter is the normative palette; names below explain placement.

### Primary

- **Signal Coral** (`accent`): moving packets, active chapter progress, paired-bracket mark, the primary playback control and focus outlines.

### Secondary

- **Verified Mint** (`verified`): receiver and recovery cues, completed progress and the matching SHA-256 conclusion. It is a state color, not a second promotional accent.

### Neutral

- **Terminal Charcoal** (`background`): the page and film ground.
- **Pane Charcoal** (`canvas-panel`): the sender/receiver field inside Canvas.
- **Control Charcoal** (`surface`): HTML hover, current chapter and Image selector surfaces.
- **Warm Ink** (`ink`): headlines, image context and primary reading text.
- **Sage Gray** (`muted`): metadata, explanatory text and inactive controls.
- **Structural Green** (`line`): terminal rules, controls and inactive progress tracks.

**The State Color Rule.** Preserve coral as transfer/interaction emphasis and mint as recovery/verification emphasis; keep terminal framing quiet.

## Typography

Both families are served from `demo/assets/fonts/` with `font-display: swap`. Sora supplies the human voice; JetBrains Mono makes commands, sequences, envelope bytes, time and hashes legible as data. Do not substitute Space Grotesk for the shipped Sora face.

Frontmatter sizes describe actual roles. Canvas display sizes use the internal 1920×1080 coordinate system and scale with the entire film; the final headline is slightly smaller (63px). HTML caption size falls to 18px on narrow screens. Terminal labels vary with density, while the full SHA-256 is displayed as two equal monospace lines.

**The Two Voices Rule.** Use Sora for narrative and controls, JetBrains Mono for literal terminal content and numeric readouts.

## Layout

The film is one 1920×1080 Canvas, fluid at 16:9 within a centered page capped at 1920px. Its content begins 100 canvas pixels from either edge. One 1720×578 terminal panel contains sender and receiver panes separated at the horizontal midpoint. The image fields, protocol area and narrow packet gutter stay in fixed canvas coordinates rather than reflowing independently.

The HTML player sits below the film with 5.2% horizontal inset. Transport, Image selector, chapters, captions and transcript form a compact reading sequence. Below 600px, player inset becomes 16px, transport and source controls wrap, chapter labels tighten and the seek bar moves to its own row. Controls retain minimum 44px height rather than scaling down with the film. Record mode hides the HTML player and contains the film inside the viewport.

## Elevation & Depth

There are no box shadows. Tonal layers distinguish the background, terminal pane and HTML controls; thin rules establish structure. The terminal status strip has its own subdued green field. The image field uses registration marks and square reconstruction tiles, with no floating card treatment.

## Shapes

The terminal and its pane geometry are square and rectilinear. HTML buttons and the select use gently rounded control corners from the frontmatter. Borders are single-pixel structural strokes; keyboard focus uses a separate coral outline. QR modules remain crisp squares with a four-module quiet zone and integer scaling in Canvas.

## Components

### Film and image reconstruction

A single terminal frame carries five time chapters: opening image (0–5s), QR V12-M (5–15s), QR V40-L (15–25s), Base64 pipe (25–36s), and restored image/hash (36–45s). Seeking redraws the requested time directly. A seeded shuffle fixes the 20×20 tile reveal order, while a smoothstep curve authors reconstruction progress. Frame cadence and reveal timing are editorial choices.

The displayed QR matrices and SI1 envelope excerpts come from prepared protocol payloads. Selected frames are reused during playback; the percentage, tile reveal and cached-symbol count illustrate reconstruction rather than reporting a live receiver. The complete SHA-256 and original byte count come from that image's payload. A local screenshot may be cropped for display while its original bytes remain the transfer subject.

**Evidence boundary.** Preserve the visible distinction between actual protocol samples and choreographed playback. Film timing is never throughput evidence or a speed comparison.

### Transport controls

The coral Play/Pause button is the primary action. Replay and fullscreen use outlined icon buttons with accessible names. The native range input has a visible time output and an explicit label. Hover adds the HTML surface; primary hover lightens coral. Focus is a 2px coral outline offset by 5px. Loading disables dependent controls with reduced opacity, and reduced-motion preferences suppress control transitions and prevent autoplay.

### Image selector

The native select has the visible label **Image**, with **Bundled sample** and **Prepared reference · local** options. The local reference is a separately prepared asset, not an upload field. Selection changes the source query and reloads the film. On load failure, the selector remains available and a Retry loading button appears with a readable status message.

### Chapters

Five text buttons seek to chapter starts. The current chapter exposes `aria-current` and receives coral text on the HTML surface. Chapters wrap into rows on narrow screens without becoming tiny targets.

### Captions and transcript

The film's English caption is repeated as visible HTML with polite live updates. An expandable native details element provides a prose transcript; the Canvas also has an accessible summary. These complement the visual film and its native controls. No audio or narration is required to understand the sequence.

## Do's and Don'ts

### Do:

- **Do** preserve the split-terminal film and reflow its HTML controls independently.
- **Do** keep commands, frames, byte counts and hashes tied to the selected prepared payload.
- **Do** keep the choreographed-playback disclosure visible and the HTML transcript available.
- **Do** preserve the labeled native Image selector, focus outlines and minimum control height.

### Don't:

- **Don't** present the 45-second timeline, reveal percentage or frame cadence as measured performance.
- **Don't** replace Sora with Space Grotesk or introduce a third type voice.
- **Don't** add decorative dashboard cards or shadows to this terminal film.
- **Don't** imply that source-code licensing covers a user-supplied reference image.
