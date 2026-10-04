# UX1 reference and dependency study

Question: preserve quick F3 and exclusive gameplay/UI focus while fixing developer chords,
console discovery and text correctness. Existing DUX1/C1 mechanisms remain authoritative.
Reference baseline: locked reconstruction 740c583901e1ff1150e9ef37e37dab5bc0e4f807,
Minecraft.java input/displayGuiScreen and GuiIngame showDebugInfo, recorded in DUX1 study.
refs-status reports optional clones absent here; no reference code/pixels are copied.

Semantic invariants: ordinary digits select hotbar; held F3 consumes developer digits; bare F3
acts on release; focus changes clear held state. Slash opens command entry; backquote opens Rhai.
Intentional deviations: semantic registered shortcuts, Unicode text rather than historical bitmap
font, bundled open fonts, grapheme editor and IME commit/preedit. No gameplay/presentation changes.

Dependency evaluation: cosmic-text 0.19 (MIT/Apache-2.0) supplies HarfRust shaping, bidi,
fontdb and swash rasterization. Disable default fontconfig and populate an empty database explicitly.
Glyphon 0.9 requires wgpu25; 0.10 requires wgpu28, while RustCraft uses26. Avoid duplicate GPU
stacks or a wgpu upgrade: use cosmic-text CPU rasterization into one bounded reusable RGBA text
surface, rendered with existing GPU HUD mechanism. Cache static text and cap glyph generations.
unicode-segmentation supplies grapheme boundaries. No custom Unicode shaping algorithm.

Font candidates: supplied cairopixel.zip not found in available workspace. Fusion Pixel upstream
2026.09.25 provides pixel Latin/Greek/Cyrillic/CJK, OFL1.1 with constituent notices. Noto script
fallbacks provide Arabic/Hebrew/Devanagari/emoji under OFL1.1. Inspect actual cmap and sizes, do
not claim universal glyph coverage. Font resources are semantic startup package overrides.

Acceptance: native input/focus/hotbar tests and real graphical scenario, grapheme/IME tests,
no-system database proof, multilingual capture, semantic replacement, bounded cache soak,
C1 scale/readback and DUX1/C1 regressions; release embedded resources/notices and platform CI.
