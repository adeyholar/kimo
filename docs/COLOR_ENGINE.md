# Color engine: measured status

## Explicit managed transform (V2, not Resolve superiority)
`add_effect kind=color_managed` converts RGB primaries through derived RGB/XYZ matrices, Bradford white adaptation, and explicit transfer functions. Gamut IDs: Rec709=0, Rec2020=1, P3D65=2, ACES AP1=3, AP0=4. Transfers: linear=0, sRGB=1, gamma2.4=2, ACEScct=3, LogC3 EI800=4, SLog3=5, PQ=6, HLG scene=7, BT709 OETF=8.

Linear values and negatives are preserved until an explicitly selected output compression. PQ uses `reference_white` nits per linear 1.0 (default 100). HLG is the scene OETF inverse, NOT a display EOTF/OOTF. Use `tone_map=1` for luminance Reinhard and `gamut_compress=1` for constant-luminance chroma compression into 0..1. Neither is an ACES RRT/ODT or measured display rendering transform. ACEScct + AP1 is the working encoding, not a complete ACES pipeline.

Tests: 25 gamut-pair inverses/neutral adaptation within 2e-12, PQ 100-nit reference/10,000-nit endpoint and 999 samples roundtrip <1e-12, HLG roundtrip <3e-8, negative scene preservation, compression luminance preservation, hostile/nonfinite parameters. These are math tests, not camera/chart certification. Render proof: 48-frame 640x360 HDR gradient, 2.239 s wall-clock including encoding. Synthetic only.

Sources used for constants:
- https://docs.acescentral.com/encodings/acescg/
- https://www.itu.int/dms_pubrec/itu-r/rec/bt/R-REC-BT.2100-2-201807-S!!PDF-E.pdf

## Free-position curves (V2)
`set_color_curve clip_id channel points`: 2..32 increasing [x,y] pairs, channel m/r/g/b. Shape-preserving PCHIP, no monotone segment overshoot; endpoint linear extrapolation rather than input clipping. Saved as normal effect params and keyframable with existing MCP tools. `add_effect kind=point_curves` permits combined master/RGB curves with channel_count and channel_xN/channel_yN params.

Tests: nonuniform identity including negative/high values, 10,001 monotone samples, invalid/duplicate/nonfinite points. Proof: same 48-frame gradient with custom master curve, 1.448 s wall-clock. No claim of visual superiority from a gradient.

## Remaining
Float linear project-wide decode/compositing, higher-bit export, OCIO config/official ACES output transforms, measured film stock, full covariance/perceptual shot matching, HDR scopes with metadata, camera-gamut-specific input transforms. Legacy effects remain display-referred. Existing color_space is transfer-only for compatibility.

## LUT precision/safety (V2)
The existing `lut` effect now uses domain-aware tetrahedral interpolation. Accepts 2..65-cube grids, DOMAIN_MIN/MAX and finite rows; rejects 1-point grids, invalid dimensions, nonfinite data and inconsistent rows. File cap 32 MiB, cache cap 8 entries, cache key includes size and mtime (avoids stale replacement). Existing old unbounded parser removed. Three unit tests cover domain identity over all tetrahedra (1000 samples, error <3e-7), vertices and malformed data. Proof: 48-frame 640x360 synthetic 33-cube grade, 0.607 s including encode.

## Quantitative scopes
`analyze_color` returns float RGB/luma range statistics, under/over-range counts, 16..4096-bin histograms, 256-column waveform density, 128x128 BT709 CbCr density. No implicit HDR/nits interpretation. The render output's color state is the caller's responsibility, and these scope data describe that render, not unmodified source media. Alpha-zero samples ignored. Two unit tests cover range accounting and hostile/empty inputs. 230,400-pixel proof analyzed in 0.0314 s. Existing PNG scopes remain display-referred/unlabelled, not pro-scopes parity.

40 targeted real-clip hostile color calls passed without server death, including render-time bad LUT sizes/NaN and extreme curve counts/gamut ids. Generic fuzz prior: 3190 calls, zero crashes; initial media fixture setup failed, so those generic cases did not exercise real media ids. Reproducer: examples/color_engine_proof.py.

## Perceptual full-covariance shot matching (V2/BETA)
`match_grade` samples source/reference frames (optional normalized [x,y,w,h] regions) and builds a symmetric Gaussian optimal-transport matrix in normalized CIELAB. The frame must be sRGB/D65 display-referred. Uses all nine covariance terms rather than independent per-channel scales. Regularization controls flat/low-rank regions. Adds `lab_match` with existing parameter/keyframe persistence.

Three tests: 1000 Lab roundtrips <2e-12, full covariance transport <1e-10, flat covariance identity and invalid regularization. Synthetic warm/cool gradient test: 230400 samples/shot, match computed in about 0.12 s, 48-frame output about 2.52 s including encode. See examples/color_match_proof.py for render-space Delta E76 measurement. This proves this specific synthetic transfer, not semantic shot match, matching different subjects, camera accuracy, or Resolve superiority. Whole-scene statistics can contaminate the match with black canvas/background; select representative regions for differently framed shots. No skin/subject segmentation or temporal smoothing.

Lab equations/constants source: http://www.brucelindbloom.com/Eqn_Lab_to_XYZ.html

## Portable pointwise grade (V2)
`cdl` supports clamped slope/offset/power and weighted saturation (caller chooses input encoding). `export_grade_lut` freezes supported pointwise effects into a 17/33/65 grid .cube; rejects spatial effects and per-effect mask bindings rather than silently dropping them. Clip transforms, timeline masks and source pixels are intentionally not part of the exported grade. Includes DOMAIN_MIN/MAX for bounded extended domains. No CDL XML import/export or certified ASC conformance.

Synthetic 33-cube CDL roundtrip: bake 0.0122 s (35937 entries), 48-frame 640x360 export 0.602 s; 8-bit direct-vs-baked RGBA mean absolute error 0.00278 code values, max 1 code value. Unit tests cover known SOP result and invalid params. No measured film stock LUT included.

Synthetic Lab transfer: render-space mean Delta E76 against same-layout reference improved 27.1945 -> 0.8418. This is a warm/cool gradient pair, not different real scenes and not a perceptual study.

## Real-footage validation and sampling fix
On SCUMBLE footage (two-second segment, 640x360), added deliberate CDL cast/contrast/saturation distortion then matched the same reference frame. A floating-point frame-boundary bug in existing clip isolation sampled shifted clips one frame earlier (`3.8-3.0` underflow at 25fps), causing the first test to match different fast-cut frames and worsen Delta E76. Fixed color-tool isolation by zero-based clock + frame-boundary snap; out-of-clip sample times now fail. Corrected mean Delta E76 8.596 -> 2.447 (71.5% lower). 48-frame export 3.758 s; sampling+matching 0.472 s. This is known-distortion recovery, NOT different-shot creative matching or reference-tool parity.

Qualifier adds edge-aware guided matte refine, black/white cleanup and matte gamma, with bounded radii. SCUMBLE hue qualifier proof: 48 frames 640x360, 11.450 s; hue keys matching colors across the scene, not a semantic object/skin selector. Matte inspected. General color effects now reject nonfinite output rather than passing corrupt pixels into encode.

## Color blur hot-path fix
Replaced color qualifier/halation use of the generic per-pixel-allocation box blur with a color-only separable RGB cascade. Correct clamp extension, f64 running sums, bounded radius, one scratch allocation, alpha preserved. Constant-border and alpha invariance tests pass. Same actual SCUMBLE 50-frame qualifier export fell from 11.450 s to 2.868 s on this workspace (about 4x faster end-to-end, includes decode/encode). Recovery export 3.635 s, Delta E76 unchanged. Prior documents called the 2-second/25fps real proof 48 frames; corrected count is 50 frames. Synthetic 24fps proofs are 48 frames.

## Scene-linear grade (V2)
`linear_grade`: exposure/gains/offset, signed pivoted power contrast and gamut-aware luminance saturation without intermediate clipping. Explicit chain: sRGB Rec709 -> linear AP1 via color_managed, linear_grade gamut=3, linear AP1 -> display sRGB Rec709 with optional output gamut compression. Existing display grading is unchanged. Managed tone-map/compression now uses destination gamut's luminance coefficients (was always 709).

Tests: extended-negative/HDR identity, radiometric doubling for +1 stop, luminance preservation across all five gamuts. Real 50-frame 640x360 SCUMBLE proof: 2.824 s including decode/audio/encode. 33-cube portable bake of 3-effect chain 0.0483 s. Visually checked source/grade and clip. Input decode still comes from 8-bit media; this explicit effect chain does NOT switch the entire compositor or export to a scene-linear/10-bit workflow.

Final generic MCP fuzz after adding an actual image fixture: 129 tools, 5283 calls, zero failures, with real clip/asset ids. Added a Rust test that renders every catalog color effect at extreme params and requires finite output or explicit error. Color unit/regression count: 26 (plus one existing unrelated engine test).
