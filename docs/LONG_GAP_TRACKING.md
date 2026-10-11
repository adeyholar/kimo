# Long-gap tracking: first appearance-only version

`track_long_gap` is a separate read-only MCP tool. Existing `track_point`,
planar tracking and roto propagation are unchanged. It returns track observations,
not attached-layer keys or propagated masks.

Input: media `clip_id`, normalized seed `bbox: [x,y,width,height]`, optional
inclusive clip-local `start_frame`/`end_frame`, `threshold` (NCC, default .8),
`margin` (default .08), `confirm_frames` (default 2). Up to 10,000 frames per
call; long clips can be processed with explicitly seeded shorter windows.
Normal playback only, including existing in-point/fit/speed decoding. Reverse
and freeze are rejected. Box coordinates refer to fitted decoded media before
clip transforms, effects and masks, not to the final composited result.

## Design

- Seed frame supplies an immutable 12x12 grayscale descriptor. Each descriptor
  sample is a 3x3 local average, then mean-subtracted and normalized. Seeds
  smaller than 8 analysis pixels or low texture are rejected.
- Analyze at at most 640 pixels on either dimension, via the existing bounded
  decode cache. Only the requested clip is decoded.
- Search the entire canvas every frame at .65/.8/1/1.2/1.5 times seed dimensions.
  Refine 12 spatially distinct peaks. NCC and a margin over a spatially distinct
  runner-up gate acceptance. Correlation is a score, not calibrated probability.
- Failed/ambiguous matches immediately become `lost`, with null boxes. Keep the
  anchor for any gap length within the requested range. No template updates,
  velocity extrapolation, interpolated positions or expiry timer.
- After loss, consecutive spatially consistent accepted candidates are required.
  Until confirmation they are `tentative` with null boxes. Confirmed return is
  `reacquired`; subsequent accepted frames are `visible`. First frame is `seed`.
- Checks inside search loops cooperate with the existing cancellation sandbox.
  Flat-frame shortcut avoids search when no textured anchor can be present.

## Proof at e9c4762 + this patch

`cargo test -p kino-core --lib`: 91 passed (85 baseline + 6 focused tests).
Focused tests cover 300-frame disappearance, shifted reappearance and confirmation,
identical distractors, interrupted confirmation, affine intensity changes,
malformed/flat/out-of-frame inputs, command registration and read-only errors.

Real footage: OpenCV's faceocc2 test video, 812 frames at 320x240/25fps:
https://raw.githubusercontent.com/opencv/opencv_extra/5.x/testdata/cv/tracking/faceocc2/data/faceocc2.webm
Ground truth:
https://raw.githubusercontent.com/opencv/opencv_extra/5.x/testdata/cv/tracking/faceocc2/gt.txt
Observed source directory:
https://github.com/opencv/opencv_extra/tree/5.x/testdata/cv/tracking/faceocc2

Natural clip (no inserted gaps): 220 reported boxes; mean center error 6.60px,
max 14.81px among reported boxes. 7 reacquisitions, 582 lost + 10 tentative frames.
Final run 18.06s. It remains lost after frame 317. Face rotation and later a cap
break the immutable appearance model. Do not call this a benchmark pass or
semantic identity tracking. Error on reported boxes excludes lost frames.

Controlled gap with real footage: frames 0..7, 300 completely black frames,
then original frames 40..100 translated +60px. This is an artificial blackout
and abrupt displacement, not natural 12-second occlusion footage. End-to-end
command ran windowed decoding on the encoded 369-frame clip in 3.61s.
Zero non-null boxes across the 300 blackout frames (12s). Frame 308 is
null/tentative, frame 309 reacquires. 58 of the 61 post-return frames report
boxes; mean center error 5.55px, max 13.81px. The whole clip has two
reacquisitions (309 and 334). Project JSON is asserted unchanged on success.
Visual contact sheet inspected: face box before blackout; no boxes inside
blackout or on first returning frame; second returning frame has a face box.

Reproduce (requires Rust, ffmpeg with libx264, Python and network):

```
python3 tools/long_gap_validate.py /tmp/long-gap-proof
```

Writes input clips, full per-frame JSON and metrics. Release runtimes depend
on hardware; these are local measured results, not performance guarantees.

## Honest limits

Not a learned detector, semantic person re-ID, pose/rotation tracker or
segmentation system. No appearance adaptation. Similar-looking subjects can
be ambiguous or swap when one distractor matches and the real subject does not.
A full-frame search reduces motion assumptions, not identity uncertainty.
Very small subjects, major rotation, deformation, lighting shape changes,
background-heavy seeds and scale outside the five search scales can fail.
Boxes can shrink to a face subregion even if the seed included more of the head.
The default is intentionally allowed to abstain. Threshold tuning cannot turn
this into semantic re-ID. Returned null gaps must stay null unless separately
reviewed; do not interpolate them as observed motion.
