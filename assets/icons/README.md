# Component icon sources

These 24 × 24 icons use explicit operation-specific line drawings. White strokes and a solid family colour remain legible against light and dark canvases. No font, external image service or random identity marker is used at runtime.

- `manifest.json` maps asset names to their visual meaning.
- SVG is the editable source used by GH2.
- PNG is the antialiased 24-pixel rendering embedded by GH1.
- Xvarna aliases only represent the same capability under different GH1/GH2 class names.
- Run `python tools/check_icons.py` from the repository root to check asset dimensions and component coverage.

PNG files were rasterized from the SVG sources at 4× scale with Sharp, then reduced to 24 pixels. Keep both representations synchronized after editing. Product code embeds the resources; sidecar image paths are not required after deployment.
