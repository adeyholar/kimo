# Masking, selection and matte stack: status and roadmap

Everything below is callable over MCP. Tiers: SOLID = works at full strength and is tested; BASIC = works, known quality gap; QUEUED = not built.

## Built (commit "Masking stack")
| Capability | Tool | Tier | Notes |
|---|---|---|---|
| Unlimited mask stack per clip, ordered, add / subtract / intersect / xor / lighten / darken / difference / none | add_mask, update_mask, reorder_mask, remove_mask_layer, list_masks | SOLID | Same combine rules as AE; first subtract/intersect layer starts from "everything" |
| Rect (rounded corners, rotation), ellipse | add_mask kind=rect/ellipse | SOLID | analytic AA edge |
| Bezier path with in/out tangents, regular polygon, star | kind=path (points or ngon) | SOLID | 4x4 supersampled coverage, non-zero winding |
| Animated paths (rotoscoping) | set_mask_path_key | BASIC | per-point linear tangents + smoothstep between keys; no per-key bezier easing yet |
| Linear / radial gradient masks | kind=linear / radial | SOLID | |
| Feather (px, gaussian-like), expand / choke (px, signed), opacity, invert | on every layer | SOLID / BASIC | expand uses a square structuring element (not euclidean) |
| Every param keyframable (and expression-capable) | set_mask_prop_key | SOLID | |
| Luma selection, color selection, HSL qualifier (hue, sat, lum windows, softness) | kind=luma / color / hsl | SOLID | Resolve-qualifier style, no despill |
| Difference matte vs a clean-plate clip | kind=difference (ref_clip) | BASIC | RGB distance, no noise reduction |
| Magic wand (contiguous or global, tolerance, softness) | kind=wand | SOLID | |
| Scribble select (foreground/background points) | kind=scribble | BASIC | color + spatial distance model, not a graph cut; no ML |
| Edge mask (Sobel band, thickness) | kind=edge | SOLID | |
| Own-alpha mask (threshold, softness) for shrink/grow/outline work | kind=alpha | SOLID | |
| Procedural noise mask, animated threshold (organic dissolves) | kind=noise | SOLID | kino extra |
| Track mattes: alpha, luma, inverted alpha, inverted luma, hide source | set_track_matte | SOLID | matte clip is rendered with its own transforms, effects, masks |
| Per-effect masks: any effect gated by any mask layer | set_effect_mask | SOLID | |
| Masks on adjustment layers keyed on the image below | add_mask on adjustment clip | SOLID | power-window + qualifier workflow |
| Mask inspection (matte / overlay / cutout PNG) | render_mask | SOLID | |

## Queued (not built, in priority order)
1. ML segmentation (SAM-class, point/box/text prompt) and portrait/depth matte. Needs an ONNX runtime and a bundled model; no model is shipped today, so nothing in kino claims ML roto.
2. Mask tracking: attach a mask to a point/planar track so it follows footage (tracking.rs has a point tracker; no planar tracker yet).
3. Roto assist: optical-flow in-betweening between path keys, edge snapping.
4. Per-point variable feather (AE Mask Feather tool) and per-point easing.
5. Edge-aware refine (guided filter), despill, clean-plate keyer with core/edge/shadow split (Primatte / Ultra Key class).
6. Euclidean expand/choke, GPU path for all coverage ops (currently CPU, about 30-80 ms per full-HD layer).
7. Kino-exclusive ideas not yet built: motion mask (frame-difference), audio-reactive masks, mask math between clips, prompt-by-text selection once ML lands.
