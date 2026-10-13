# Advisory frame lint

`lint_frame` reports possible problems. It never changes the project, creates an undo entry, rejects an edit or blocks export. It is not a design certification.

The first slice implements `out_of_bounds`, `invisible_layer`, `low_res_upscale` and `static_frame`. Text ink bounds, contrast, safe zones and text/caption overlap checks are pending the shared text metrics and safe-margin APIs. Asking for an unsupported check returns `invalid_check`; absence of those findings does not mean the text was checked.

Choose one selector: `time`, `times`, or `range: {start,end,step}`. The default range step is 0.5 seconds. The default and hard cap is 120 sampled frames. `params.max_frames` may lower it. Too many samples return `too_many_frames` before rendering.

Thresholds are defaults, not rules. `params` supports max_static_s (3), mse_threshold (0), changed_ratio_threshold (0), pixel_threshold (0.04), opacity_epsilon (0.001), upscale_ratio (1.5) and max_frames (120). All effective values are echoed in params_used. Unknown parameters fail rather than being ignored. `delivery_height` is recorded for the later text checks; it does not change layer geometry.

Layer geometry uses renderer-resolved parent transforms and source extents, not the full-canvas quad of a small shape. These are layer bounds, not effect-expanded ink. Rotated layer bounds are axis-aligned envelopes. Masks, blur, shadows, 3D tilt and text ink are not certified by those bounds. Low-resolution upscale uses the drawn bounds, so rotation can increase its ratio.

`lint:allow_bleed` suppresses out_of_bounds on that clip. `lint:ignore=<code>` suppresses that code. Every suppression is counted and listed with the layer and tag. A suppression is not a successful check.

Adjacent sampled layer hits merge into intervals, with the largest measured overflow/upscale (or lowest opacity) recorded as worst. Static intervals use the existing diff_frames metric implementation. They say only that sampled frames were unchanged for longer than max_static_s; audio presence is reported as context, not a claim of narration or an intentional hold.

`annotate` defaults true. A finding frame gets a PNG with orange-red boxes, a file path, and inline image delivery over MCP. Annotations do not edit the project. max_findings defaults 200 and limits returned findings across the call; summary counts and omitted_findings retain visibility of truncation. Each frame reports measured elapsed_ms, which is not deterministic; finding data is deterministic.
