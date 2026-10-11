# kino feature checklist

Target: every useful Premiere Pro editing tool and the full After Effects effect range, each callable as an MCP tool.
Legend: [x] shipped and tested, [~] partial / approximate, [ ] pending. Catalog counts below are reconciled to engine checkpoint `dd9a517` on 2026-10-11; use the live server (`tools/list`, `list_effects`) for later counts. Timings and fixture measurements are historical checkpoint evidence, not current benchmarks. Mixed checklist rows use [~] when only a subset is shipped; later sections state the limits.

Sources: Adobe AE effect list and category pages (helpx.adobe.com/after-effects/desktop/apply-effects-and-animation-presets/...), AE help index (compositing, expressions, 3D, tracking, time remap), Premiere help (nested sequences, color, proxies), CapCut feature pages (see README).

Design rule: one registry in `crates/kino-core/src/commands.rs` + `edit.rs`; MCP lists straight from it. New feature = new command or effect catalog row, so AI can discover it with `list_effects` / `tools/list`.

## A. Premiere Pro: editing

### Timeline and trimming
- [x] multi-track timeline, add/move/trim/split/delete/duplicate clip
- [x] ripple delete, ripple trim (head/tail), roll edit, slip, slide
- [x] razor all (split_all_at), insert gap, lift / extract range, remove silence ranges
- [x] rate stretch, speed, speed ramp (time remap curve), reverse, freeze frame
- [x] markers (point/range, color, name), snap_time, list_edit_points
- [x] track lock, rename, reorder, delete
- [x] undo/redo, save/load project
- [ ] insert vs overwrite with auto-ripple on add_clip, three/four-point edit, source in/out marks
- [~] group clips via precomps; pending: track targeting, sync lock, linked audio+video clips, link/unlink
- [~] nested compositions via group_clips / ungroup_clip; no editing grouped clips in place or group time remap
- [ ] multicam: sync by audio/timecode, angle switching, multicam to flat
- [ ] subclips, clip labels/colors, bins and metadata, search
- [ ] replace clip (keep effects), paste attributes, effect presets (save/apply), copy/paste keyframes
- [ ] auto reframe (cover-fit decode), scene edit detection, auto-cut to beats / to speech
- [ ] proxy workflow (generate proxy, toggle), offline media relink
- [~] adjustment layers [x], track mattes [x], sequence settings per sequence, multiple sequences
- [ ] rolling/regular edit by timecode text ("+10", "-5 frames") helpers
- [ ] captions: SRT import [x], speech-to-text [ ] (needs whisper), caption styling presets [ ], burn-in [x via text]
- [~] designed title/lower-third presets via add_title; no MOGRT import or general parametric template system

### Transitions (31 registered kinds)
- [x] crossfade, wipes x4, slides x4, zoom, spin, flash, iris, blinds, checker, diagonal
- [x] dip black/white/color, whip left/right, light leak, glitch, zoom blur, clock, split h/v, noise dissolve, shatter / shatter_cells
- [ ] outgoing-clip push/slide-out, luma wipe from custom matte, page peel, morph cut-like, ripple dissolve, gradient wipe, barn doors
- [~] 10 transition easings; no centered/start/end alignment control

### Color (Lumetri-style)
- [x] exposure, contrast, saturation, temperature, tint, color wheels (lift/gamma/gain), curves (luma S-curve), HSL secondary, LUT (.cube), teal/orange, vignette, color isolate, levels, brightness/contrast, channel mixer, gradient map, colorize, shadow/highlight, threshold
- [x] scopes (waveform/vectorscope/histogram data via get_scopes)
- [~] free-position master/RGB curves, hue curves, qualifier with per-effect masks and display-sRGB/D65 Lab shot match; pending: mask tracking binding, semantic match and automatic creative tone selection
- [~] opt-in scene-linear wide-gamut pipeline, explicit gamut/log transforms, supported pointwise grade LUT export, approximate film looks, optional pinned OCIO/ACES2 reference; no automatic camera IDT, measured film-stock calibration, production OCIO config import or certification
- [ ] denoise (temporal/spatial), sharpen [x] unsharp, dehaze, clarity, lens profile correction

### Audio
- [x] per-clip volume + keyframes, fades, mute, pitch, high/low pass, compressor, echo, denoise flags, reverse, beat detection, export_audio
- [ ] essential-sound style: dialogue loudness normalize, auto-duck music under voice, EQ bands, de-esser, de-reverb, limiter, reverb, noise gate, stereo pan keyframes, surround, audio transitions (constant power/exponential)
- [ ] voiceover record, audio sync by waveform, loudness meter (LUFS) readout

### Titles / text
- [x] text clips with typewriter, font size/color, keyframed transforms, masks
- [~] char/word/line animators (position, scale, rotation, opacity, blur, tracking), path text, title presets, stroke/shadow/background styles and font-weight control; pending: per-character color, unrestricted variable-font support, per-span rich text and 3D extrusion

### Delivery
- [x] mp4 export (parallel, streaming), gif, audio export, still frame
- [ ] export presets (YouTube, TikTok, ProRes, DNxHR, image sequence, WebM/VP9, AV1, HEVC), render queue, background render, in/out range export, alpha export, burn-in timecode, watermark, chapter markers

## B. After Effects: compositing and motion graphics core
- [x] layers: video, image, text, shape (rect, ellipse, polygon, line), adjustment layer, solid-like via shapes
- [x] transform keyframes: position, scale, rotation, opacity with ease (linear, in, out, in_out, hold)
- [x] blend modes (set_blend), masks (animated, feather), corner pin, track point, stabilize
- [~] perspective 3D layers/project camera and Scene3D geometry with lights, depth of field and parenting/nulls; these are not unrestricted AE layer/lighting features
- [~] bezier timing handles, spatial paths/auto-orient, fixed expression-op stacks (wiggle/loop/sin/inertia/clamp/round); no free-form interpreter, index/valueAtTime linkage or expression-control objects
- [~] precomps on a local clock; pending: group time remap, collapse transformations, continuous rasterization
- [~] rendered-layer alpha/luma/inverted track mattes; pending: general set-matte effect and stencil/silhouette blend modes
- [ ] shape layer operators: fill, stroke, gradient fill, trim paths, repeater, merge/offset paths, zig zag, pucker/bloat, wiggle paths, round corners, twist; bezier paths
- [ ] roto: AI object matte / roto brush (needs ML model), mask tracking, refine edge
- [~] beta grid mesh pin/tracking for smooth deformation; no puppet rig, general bezier warp/liquify or content-aware object removal
- [~] transform shutter blur, echo/frame blend, speed/time-stretch and posterize time; no flow interpolation/retime or per-effect blur
- [~] planar homography tracking/corner pin; pending: 3D camera solve and face tracking
- [ ] light/color: layer styles (drop shadow [x as effect], inner/outer glow [x outer], bevel, stroke [x outline], gradient overlay)
- [ ] responsive time / presets, essential properties, data-driven animation (JSON/CSV), scripting hooks

## C. After Effects effect categories
(AE category -> kino effect, based on Adobe's effect list)

### Blur & Sharpen
- [x] Gaussian-style blur, directional blur, spin (radial) blur, zoom blur, sharpen (unsharp), tilt-shift (lens blur band)
- [ ] camera lens blur with depth map / bokeh shapes, bilateral, channel blur, compound blur (using layer map), fast box variants, smart blur, reduce interlace flicker

### Channel
- [x] channel mixer, invert, solid via gradient ramp
- [ ] arithmetic, blend, calculations, channel combiner, compound arithmetic, minimax (erode/dilate, partial: choker), remove color matting, set channels, set matte, shift channels, solid composite

### Color correction
- [x] brightness/contrast, curves (luma and free master/RGB), levels (master), exposure, hue/sat (HSL), tint, colorize, color balance via wheels, gamma/pedestal/gain, photo filter-like via temperature, shadow/highlight, vibrance-like via saturation
- [ ] levels per channel, color balance (HLS), black & white mixer, change to color, change color, selective color, tritone [x via gradient_map], leave color [x color_isolate], auto levels/contrast/color, Lumetri full, vibrance, CC tools (toner), color link, equalize, easy levels, output (clip), PS arbitrary map, color stabilizer, match color

### Distort
- [x] bulge, twirl, ripple, wave warp, turbulent displace, polar coordinates, offset, mirror, motion tile, flip (transform), lens distortion (optics compensation), kaleidoscope-style, corner pin, glitch blocks, pixel sort
- [~] beta mesh pin/tracking shipped; pending: bezier warp, general displacement map (using another layer), liquify, magnify, reshape, smear, spherize, warp (presets: arc, flag, fish...), warp stabilizer (stabilize exists via tracking) , detail-preserving upscale, transform effect, rolling shutter repair, CC (bend it, ripple pulse, page turn, lens, power pin, slant...), wave presets

### Expression controls
- [ ] slider, checkbox, color, point, angle, layer, dropdown controls feeding any param

### Generate
- [x] checkerboard, grid, gradient ramp (linear/radial), fractal noise (via fractal_noise), lightning / energy beam, lens flare, circle/ellipse/rect via shape layers
- [ ] 4-color gradient, advanced lightning variants (strike, branch decay), audio spectrum, audio waveform, beam (laser), cell pattern, eyedropper fill, fill, fractal (Mandelbrot/Julia), paint bucket, radio waves, scribble, stroke, vegas (animated outline), write-on, caustics

### Immersive / VR
- [ ] VR to sphere, VR rotate sphere, VR blur/sharpen/denoise/glow, VR chromatic aberrations, VR plane to sphere, VR convert projection

### Keying
- [x] chroma key (color, tolerance, softness), luma key, spill suppress, choker, feather alpha
- [~] keyer with screen gain/balance, clip black/white, despill and edge unmixing (synthetic tests, not Keylight parity); pending: color difference key, color range, difference matte, extract, inner/outer key, linear color key, spill suppressor for any color

### Matte
- [x] matte choker (choker), feather alpha
- [ ] refine soft matte, refine hard matte, simple choker variants, matte tools for tracked mattes

### Noise & Grain
- [x] film grain, noise via fractal noise, film damage (scratches, dust, flicker)
- [ ] add grain with presets, match grain, remove grain, dust & scratches (cleanup), median, noise HLS, noise alpha, noise (additive/mono), motion/temporal denoise

### Perspective
- [x] corner pin, drop shadow (alpha), long shadow
- [~] project 3D camera shipped; pending: bevel alpha, bevel edges, radial shadow, CC cylinder/sphere/environment, extrude

### Simulation
- [x] particles (embers, sparks, snow, rain, dust) via add_particles
- [ ] Particle Playground-style emitter with forces/gravity, particle trails, collisions, Foam / bubbles, Caustics / water surface, Card Dance, Shatter, Wave World, CC particle world/systems (fire, smoke, snow), cloth-ish cards, rigid-body sims (via integration with a physics crate), fluid/smoke sim, hair/rain on lens

### Stylize
- [x] glow, outer glow, edge glow (neon), find edges, emboss, halftone (mosaic-like dots), pixelate (mosaic), posterize, threshold, strobe, scanlines, VHS, vignette, outline, sepia, invert
- [ ] brush strokes, cartoon, color emboss, roughen edges, scatter, texturize, CC glass/kaleida/mr. mercury/blobbylize/burn film, CC threshold RGB, dither, tile mosaic, trace/vector stylize, sketch/pencil, oil paint, anaglyph

### Text
- [ ] numbers, timecode (source timecode burn-in)

### Time
- [x] time remap via speed ramp, reverse, freeze
- [~] echo/frame blend, posterize time and forward optical-flow source blur shipped; pending: time displacement, flow interpolation/retime, slowmo and per-effect force blur

### Transition effects (as effects, not clip transitions)
- [ ] block dissolve, card wipe, gradient wipe, iris wipe, linear wipe, radial wipe, venetian blinds, CC glass wipe, CC grid wipe, CC image wipe, CC jaws, CC light wipe, CC line sweep, CC radial scale wipe, CC scale wipe, CC twister, CC warpo

### Utility
- [x] letterbox (cinema bars)
- [ ] Cineon converter, color profile converter, grow bounds, HDR compander, apply color LUT [x as lut], tone mapping, compositing for log

### Obsolete / classic still used by pros
- [~] basic text, perspective 3D and path text shipped; pending: lens blur, lightning (classic), channel blur, bevel, bulge-style CC effects, reduce noise

### Third-party style looks (Hollywood staples, implement natively)
- [~] light leaks, anamorphic streaks, lens flare, film grain, halation, bloom [glow], chromatic aberration, lens distortion, VHS, glitch
- [~] approximate halation, film-print and bleach-bypass effects shipped; pending: calibrated 35mm film emulation and film gate weave, day-for-night, volumetric light, fog / atmosphere, heat haze [x turbulent_displace], energy shield, hologram, screen replacement (corner pin + tracking), sky replacement (needs segmentation), digital clone/duplicate, speed-line, rain on lens, camera shake presets [x shake], rolling shutter, lens dirt

## D. Infrastructure / performance
- [x] windowed streaming decode (24-frame window), eviction, parallel export, preview cache
- [~] CPU f32 renderer (about 2 fps at 720p for heavy stacks)
- [ ] wgpu pipeline: WGSL effect kernels, GPU compositing, frame textures in VRAM, async readback, hardware encode via ffmpeg/NVENC
- [ ] u8/half-res fast path for glow/blur, SIMD, tile-based rendering, dirty-region cache
- [~] windowed tracking/stabilization shipped; pending: proxy-based preview and cached low-res scrubbing
- [ ] expression evaluator (small interpreter) shared by all params
- [ ] project format v2 with compositions, versioned migrations
- [ ] MCP: batch tool (apply many ops atomically), describe_project summary for LLM context, templates (create_title, create_lower_third, create_trailer_titles, make_montage_from_folder)
- [ ] GUI (deferred by the user)

## E. Quality tiers (honest audit)
User bar: every feature best in class. Tiers: **PRO** = matches or beats the pro tool, **V2** = correct maths, missing refinements, **V1** = works, simple maths. Nothing is PRO until verified on real footage against the reference tool's behavior.

| Area | Tier | What is missing for PRO |
|---|---|---|
| Timeline edit tools (ripple/roll/slip/slide/lift/extract) | V2 | linked A/V, track targeting, sync locks, multi-track ripple |
| Color primaries / log wheels / curves / hue curves | V2 | float pipeline is f32 per-pixel and display-referred by default; opt-in scene-linear working space, 2..32-point free curves and explicit gamut mapping shipped; no calibrated camera/display/film pipeline |
| Color space transforms (ACEScct, LogC3, S-Log3, sRGB, Rec.709) | V2 | legacy color_space is transfer-only; color_managed supports primaries/log gamuts. No certified RRT/ODT or production OCIO config import; pinned reference is opt-in |
| Qualifier (HSL key) + Power window | V2 | bezier/per-effect masks and guided qualifier cleanup shipped; mask expansion/choke belongs to the mask stack. No mask tracking binding or universal temporal denoise |
| Scopes (waveform, parade, vectorscope, histogram) | V2 | quantitative working-linear/caller-declared-nits analysis shipped; PNG scopes remain SDR display-domain. No calibrated IRE/HDR display scope or CIE chromaticity |
| Film print, halation, bleach bypass | V1 | no measured print-stock LUT/density curves (Kodak 2383 etc.), halation is box-blur based |
| White balance picker, color match | V1 | full-covariance sRGB/D65 Lab optimal transport with sample regions shipped; no semantic match, mask-selected matching or camera calibration |
| Chroma key (old `chroma_key`) | V1 | superseded by `keyer` |
| Keyer (`keyer`: screen gain+balance, clip black/white, core shrink, edge soften, despill + luma replace, edge unmix, matte view) | V2 | per-pixel screen color variation map (clean plate), garbage matte binding, motion-adaptive matte, hair-detail refinement (guided filter), tested on synthetic screen only |
| Luma key | V1 | |
| Stabilization (two-pass: 24-patch robust similarity fit with outlier rejection, Gaussian path smoothing, lock mode, adaptive zoom, translation+rotation+scale, windowed decode) | V2 | no rolling-shutter correction, no 3D/perspective (homography) model, no inpainting of borders instead of zoom, no tripod/pan-aware modes. Measured: frame-to-frame jitter 9.8 -> 3.5 on a synthetic shaky clip |
| Tracking (ring of 9 patches, translation + rotation + scale, windowed decode, refit from frame 0 each frame) | V2 | this row describes the point/ring tracker; separate planar tracker has homography/reacquisition with limited-use tiers below. No per-patch confidence UI |
| Motion blur (`set_motion_blur`: shutter-angle multi-sample of x/y/scale/rotation, 4-16 samples) | V2 | no per-effect/param blur, forward source optical-flow blur is separate and has occlusion/fast-motion gaps; adjustment layers/text animators not covered |
| Renderer | V1 | alpha-correct bilinear + 2x2..4x4 supersampling on transforms (done); still CPU f32, no mipmaps/Lanczos for large downscales, no GPU |
| Transitions | V2 | 31 kinds, 10 easings, soft angled wipes, clock, split, iris, noise dissolve, dips, light leak, glitch, whip pan, zoom blur. Missing: outgoing-clip push/slide-out, luma-matte wipes from image, GPU-shader transitions |
| Particles | V2 (emitter) | add_emitter: 11 presets (incl. dust, smoke), forces, turbulence, attractor, floor bounce, trails, fade_in, sprites soft/hard/mote/puff/streak/ring/star/spark; no layer collisions, no texture sprites, no fluid sim |
| Text | V2 | kerned layout, word wrap, AE-style char/word/line animators with range selectors (6 shapes, keyframable) + 9 presets, stroke, blurred shadow, glow, gradient, rounded bg box. Missing: per-span rich text, unrestricted variable fonts, emoji; path text and font-weight control shipped |
| Masks | V2 | bezier path-shape keys and expansion shipped; no per-point feather, ML roto or automated mask tracking |
| Audio mixer | V1 | no EQ bands, no loudness normalize, no ducking |

- [x] HUD tools: add_detection_box, add_counter, add_prompt_box (V1)
- [x] shatter transition V2: radial + concentric spiderweb cracks that grow from an impact point, domain-warped irregular shards, impact flash and shockwave ring, shards of the outgoing frame tumble in true 3D perspective with refraction offset, edge glints and fade. Gaps: no depth-of-field blur on near shards, no secondary micro-fragment cascade, no sound. ("shatter_cells" keeps the old Voronoi reveal.)
- [x] 3D layer system V1 (three_d.rs): any layer becomes a perspective plane via rot_x, rot_y, rotation(z), pos_z, x, y, scale, backface; project camera via set_camera / set_camera_keyframe (cam_x/y/z, pan/tilt/roll, fov). Exact inverse perspective mapping with 2x2 supersampling. Shatter shards use the same Projector. Gaps: layers are composited in track order (no depth sort yet), no motion blur on 3D layers, no lights/shadows, no extruded 3D text, no mesh import.

## Tests
`python3 tests/tool_fuzz.py` reads the live MCP registry and exercises non-skipped tools with empty, null, wrong-type, negative, huge and real-id arguments, failing on crash, hang or malformed reply. The explicit skip set is export, export_gif, project_set, render_frame, export_audio, transcribe and download_stock, not every render/export tool. Call count varies with schemas; real-id coverage requires successful fixture setup.

## Shard burst (add_shard_burst) - V2
3D glass-shard cloud generator: hundreds of prism shards, project-camera projection, internal depth sort, DOF softness, rim glints with dispersion, 4-sample motion blur. Gaps: background-aware screen-space displacement shipped (see below), not ray-traced refraction; no shard shadows/physical light calibration, not GPU.

## 3D suite status
Done: layer transforms, camera, shard-internal depth sort/DOF/motion blur. Scene3D additionally has materials, lights, optional shadows/AO/env and null parenting. Todo: general layer depth sorting, DOF/motion blur on flat 3D layers, extrusion and camera rigs (orbit, look-at).

## Rigid-body physics (physics.rs, used by add_shard_burst physics:true) - V1
Deterministic 240 Hz baked sim: mass from size, linear/angular velocity, gravity, linear and angular drag, floor plane with restitution + Coulomb friction (spin from contact), body-body impulse collisions (sphere proxies, spatial-hash broadphase). Scrub-safe via cached trajectory. Gaps: sphere proxies not true convex hulls (no resting stability on faces), no joints/constraints/soft bodies/fluids, general-layer simulate_physics is a separate 480 Hz circle-proxy tool (see below), CPU.
Shard shading: Schlick fresnel and depth falloff params. Screen-space background displacement is applied by the compositor when refract is enabled. Gaps: no ray-traced refraction, caustics or area lights.

## SCUMBLE hero shatter recreation (examples/scumble_shatter.py) - V1 (recreation, not parity)
Composed from existing tools: keyboard bg with push-in and blur ramp, finger/key sprites, orange spark emitter, physics shard burst with camera fly-in and zoom-blur streaks, detection-box HUD. Gaps vs reference: finger/key/keyboard are drawn stand-ins (no real macro footage or 3D model), shards lack true refraction of the scene, no caustics, lighting is fake, no rigid-body interaction with the finger.

## Background-aware refraction (shard burst refract:px) - V1
The shard pass writes a screen-space displacement field (tilt x thickness, stronger on side walls, motion-blurred with the shards); the compositor bends the canvas behind the layer before blending. Gaps: single offset (no per-channel dispersion of the background), displacement clamped, shard layer only (not a general refractive material for any layer), no caustics.

Shard geometry V1.5: 4-6 vertex irregular convex chunks with varied aspect and thickness. Gaps: still extruded polygons, not fractured volumes (no Voronoi-fractured solids, no bevel normals).

## Scene3D (add_scene3d) - V1
Real geometry renderer: z-buffered triangles, box/rounded-box/cylinder/capsule/sphere/plane/null, parent transforms (nulls), keyframed pos/rot/scale per object, PBR-style materials (albedo, roughness, metal, emissive, opacity, Schlick fresnel, Blinn-Phong GGX-like spec), directional and point lights, ambient, 2x2 SSAA, depth-of-field from the depth buffer, transparent objects sorted back to front. Seen through the project camera, composes with shard bursts and everything else. Optional shadows/AO/procedural environment reflections are described below. Gaps: no global illumination or HDRI environment-map import, no textures/UVs, no mesh import (OBJ/glTF), no extrusion of text/shapes, no motion blur on scene geometry, transparency is per-object sorted (no per-pixel OIT), CPU only.

Scene3D additions (V1): shadows:true (1536 px shadow map per directional light, 3x3 PCF), ao:<strength> (screen-space ambient occlusion from the depth buffer), env:<strength> (procedural studio environment reflections with fresnel, driven by the lights). Known artifacts: faint facet banding on the curved capsule under shadowing (bias tuning), no soft area-light penumbra, shadows from opaque objects only, point-light shadows not done, env is procedural (no HDRI import).

## Fixes from repo issues #1-#7 (all verified)
- #1 add_clip fit: contain | cover | stretch (cover decodes at full res then center-crops).
- #2 shard DOF blur radius capped (10 px at 720) and dispersion fringe clamped, so defocus and glow no longer turn shards into fringed mosaics.
- #3 shard burst px units now scale with the SHORT edge/720 (portrait 1080x1920 gets 1.5x, not 2.67x).
- #4 export uses a work queue across cores (a slow shard frame no longer stalls the batch). Still CPU: real fix is a GPU path. Decode still serial between batches.
- #5 detect_beats / analyze: 11.6 ms resolution, onsets stamped at window centre, strongest-wins min_gap, bpm from autocorrelation. Test on a 140 BPM synthetic kick: first six onsets within 1-2 ms of truth, bpm 139.9.
- #6 add_counter hold default 0.
- #7 add_audio_clip (audio-only, never draws) and a lookahead peak limiter on export (ceiling -1.5 dBFS; tested: +80% gain input exports at -1.5 dBFS peak). Gap: sample-peak limiter, not oversampled true-peak.

## Motion program (queued, requested: all motion features of Premiere, After Effects, Resolve Studio, plus kino-exclusive)
Status honest: nothing below is done unless marked. Existing: motion blur (shutter-angle, per layer), point/ring tracker, stabilizer, speed ramp, particles, physics, Scene3D.
- [~] 1. Track mattes (alpha/luma/inverted) shipped; collapse transforms pending. DONE V1: precomps (group_clips / ungroup_clip: nested tracks on a local clock, group transform/effects/mask/opacity/keyframes apply to the whole group; gaps: no editing of grouped clips in place, no time remap of the group, no continuous rasterization, nested audio not mixed)
- [~] 2. Expressions V1 DONE: op stack per property/effect param (wiggle w/ octaves+seed, loop_in/out cycle|pingpong|offset|continue, sin, inertia, posterize_time, clamp, round) via set_expression; cubic-bezier key handles via set_keyframe_bezier (overshoot ok). Gaps: no free-form expression interpreter (linkage, valueAtTime, index), no per-axis 2D wiggle correlation, no handle UI/ separate in/out influence. Spatial paths with auto-orient shipped; text animators still need polish
- [~] 3. Echo V1 DONE (set_echo: composite/add/screen/average frame-blend ghosts, +/- time; set_posterize_time on whole clip). Gaps: no optical-flow interpolation/retime (forward source blur shipped separately), no time displacement, no rolling-shutter repair. Original: Echo, posterize time, frame blending, optical-flow interpolation and slow-mo, optical-flow motion blur for footage, time displacement, rolling-shutter repair
- [ ] 4. Motion blur for Scene3D geometry and 3D layers, per-effect blur
- [~] 5. Planar/homography tracker and limited occlusion reacquisition shipped; pending: mask tracking, 3D camera solve, face tracking, independently measured warp stabilizer parity, object removal
- [~] 6. Beta mesh pin/tracking shipped; pending: ML roto/segmentation, puppet rig, liquify, smart reframe, speed warp, optical-flow retime
- [ ] 7. Kino-exclusive ideas: beat-driven motion (bind any property to audio::analyze beats), motion-by-prompt presets (AI writes the curve set), physics-driven layers [x via circle-proxy simulate_physics], motion "diff" tool (compare two clips' motion curves and match one to the other), auto motion-match to reference footage via tracker.
- [ ] Infra: HUD timecode as one dynamic text source (currently 2 clips per frame, slow export)

- [x] impact landing for detection boxes (impact:true), add_box_expand (constant-speed expansion + 2-frame flash), add_camera_shake / impact_shake (damped spring, measured from SCUMBLE), add_hud_frame, dynamic text tokens {tc} {bar} {t} {pct}. V1: measured from one reference, not generalized beyond it; see examples/scumble_*.py for known diffs.

## Sandbox layer (guard.rs, built into every tool call)
- [x] V1: argument sanitizer (finite numbers, magnitude +-1e8, array/string size, nesting), project limits checked after every mutation with automatic rollback (canvas pixels, fps, clip count, timeline duration), memory budget via counting allocator (default 70% of RAM or cgroup limit; Frame allocations and the render loop fail the call instead of OOMing; hard backstop refuses allocations far above budget), cooperative cancel plus wall-clock time limit per call (default 600 s, export 7200 s) with a last-resort clean server exit if a call cannot be stopped, panics caught with project rollback. Env overrides: KINO_MEM_MB, KINO_TIMEOUT_S, KINO_EXPORT_TIMEOUT_S, KINO_MAX_PIXELS, KINO_MAX_FPS, KINO_MAX_CLIPS, KINO_MAX_DURATION_S, KINO_MAX_NUM. Test: tests/sandbox_test.py.
- Gaps (honest): in-process, not OS-level isolation (no seccomp/namespaces/cgroups; a native-code bug that corrupts memory still kills the server); memory accounting covers Rust heap only, not ffmpeg child processes or GPU memory; cancellation is checked per clip/frame and inside several color, scope and process-wait loops, but not every legacy effect or synchronous file operation is interruptible; ffmpeg children get no separate CPU/memory limits yet; file paths are not jailed (tools can read/write any path the user can).

- add_speed_lines (manga radial rays, flicker/spin/bg). Gaps: flat-colored rays only, no per-ray gradient or noise streak variant, CPU O(pixels x rays) (~0.1 s/frame at 70 rays).
- add_camera_shake now uses a measured per-frame kick table ([-19,-10],[-11,-6],[-4,-2] px @720p, discrete frames, same as the SCUMBLE reference; pass profile for others, profile='spring' for the old damped spring). Verified frame by frame with tools/bracket_track.py + examples/shake_align.py (kick frames and values identical to the reference at 4.12/4.60/5.08). Gaps: per-hit amplitude differences in the ref (6.04 hit is ~1.3x) and later hits 6.5-7.5 not yet verified because the HUD bracket is covered there.

- `silhouette_fill` (effect): flat-color silhouette of a clip's alpha, mix amount, optional outline ring. Tier: solid. Gap: no pattern fills, no auto-segmentation (mask comes from an RGBA cutout).
- `add_glitch_confetti` (generator): random flat squares + horizontal tear lines, reshuffled at flicker_fps. Tier: solid. Gap: no block displacement of the plate under it (use glitch_blocks).
- Masking stack (masks.rs): see docs/MASKING_ROADMAP.md for the full table and honest tiers (ML roto, mask tracking, per-point feather QUEUED).
- [x] 2b. Spatial motion paths (set_motion_path / remove_motion_path): bezier or through-points (auto-smooth Catmull-Rom) or presets (circle, ellipse, figure8, wave, arc, spiral), constant-speed arc-length progress via keyframable property `path` (ease, bezier handles, expressions work), auto-orient with offset and corner smoothing, correct under motion blur and echo. Gaps: no per-point speed handles (use eased `path` keys), no 3D paths, no path editing UI.

### Planar tracker (mocha-style) - built, tiers
- `track_planar` (homography, 4 corners) + `pin_layer` (perspective corner pin, keyframed, 3x3 supersampled AA edges, premultiplied) + motion blur on the pinned layer from the tracked motion + `light` brightness match.
- Mocha/AE/Resolve-style algorithm (not validated against those products): sub-pixel planar track, perspective pin, occluder-robust (Tukey weights), exposure-invariant (zero-mean/unit-variance photometric model).
- Synthetic fixture measurement: anchor-relative by re-aligning to the frame-0 anchor each frame; coasts through full occlusion and re-acquires with a coarse search. Synthetic perspective move + h264 + 180deg blur + occluder: 0.33 px mean / 0.39 px p95 corner error vs ground truth (examples/planar_demo.py, tools/wall_plate.py).
- Shipped separately: beta mesh/grid tracking, optional reanchor segments and anchor/adaptive auto-mattes (tiers below). NOT YET: backward tracking from a middle frame, general multi-anchor management and grain/lens match on the pinned layer.
- Occluder auto-matte (`track_planar occluder_matte=true`): at render time the live frame is compared with the frame-0 surface (warped by the tracked quad, brightness-matched); wherever they differ the pinned layer is cut, with 2 px grow and feather. Tier: automated difference-matte workflow, not independently measured Mocha parity; limits: occluder with the same colour as the surface is not detected, persisted recipe rebuilds the anchor after reload, no temporal smoothing of the matte yet.
- Adaptive occluder matte (`occluder_mode=adaptive`, LIMITED-USE tier, measured on one real clip): rounded 0..8 plane-px min/max-envelope compare (default 1) (`occluder_tolerance`), chroma-dominant metric (`occluder_chroma`), island/hole cleanup by area (`occluder_min_area`), thin-structure cleanup (`occluder_thin`, two-stage so fragments are not merged), temporal median (`occluder_temporal`), grey closing (`occluder_close`, default off, did not fix notches connected to the occluder), persisted recipe that rebuilds after reload, follows constant clip speed and speed-ramp clips with the same frame selection as the renderer (decoder phase bug found and fixed on the real key). Real keyboard/finger result: finger protected, no flashes over all 110 frames, glyph islands and contour rings removed, residual small notches touching the finger edge at entry and mild edge scallop. NOT claimed: neutral or dark occluders (chroma 1 cannot cut them), universal matting, or a clean solid-key replacement. Synthetic leak 0.01% to 0.14%; synthetic wins did not transfer to the real clip, so only the real-clip numbers count.
- Shard planes behind a moving occluder (`shard_burst` plane.occlusion_target + `occluder_expand`, LIMITED-USE tier): the shard plane consumes the target clip's adaptive matte in final screen space (scale, pos, rotation, motion path, 3D pose, project camera), evaluated on demand so track order and target opacity do not matter. Measured: synthetic magenta-over-skin 2476 -> 0 at target scale 1.12; real macro key shot, original order, 35 burst frames: interior finger protected, small boundary fringe remains, not a dense body mask. Neutral/dark occluders: `occluder_luma_gate` removes 100% core leak to 0.0% (false reject 0.0%) on COLOR's synthetic neutral fixture only, FAILS on real monochrome keyboard footage: gate-on cuts 5.2%/6.1%/1.7% of a clear key patch at 1/2/3 s (false rejects on printed key surface), gate-off cuts none; stays experimental, not neutral-occluder support. Mode 2 (`occluder_luma_gate=2`, brighter-than-surface only) cut that real false-reject to 0.0% at 0/1/2/3 s and keeps 0% core leak, 0% false reject on the synthetic fixture; on a REAL finger crossing over a dark key mode 2 FAILS the bar: skin leak 22.5% (1 s) and 38.7% (1.5 s) of bright skin ROI, plus 47% of dark surface wrongly cut on retreat (ghosting). Experimental, not usable for neutral occluders.
- Contact shadows for debris (`shard_burst` contact_shadow 0..1, shadow_reach, shadow_plane_z; BETA): each shard projects a soft shadow onto the receiving plane along the light vector, blur and opacity falling with height. Measured on a synthetic grey plate: shadows only darken (0 brightened px, 0 shard px changed), 1887 plate px darkened by mean 3/255 at t=1.0 strength 0.7. Not yet checked on a real shot; shadow direction follows the `light` vector and has not been validated against real footage lighting. Shadow respects the occluder matte (real key repro at 1.64, strength 0.6: 1632 plate px darkened, max 8.7 codes, 0 on the finger, 0 brightened; subtle on dark keys). Frame cost about +0 s on the synthetic scene (1.9 s on vs 2.3 s off, noise).
- Rotoscope assist (`roto_propagate`, BETA, weak gain): fills a path mask between your keyframes with per-frame optical flow (robust affine fit to interior flow, forward from one key and backward from the next, falling back toward plain interpolation where the two chains disagree) and snaps the outline to the strongest edge within `snap` px. Measured: exact-GT synthetic (textured ellipse rotating and accelerating over real keyboard footage, mean IoU over held-out frames): keys 6 frames apart interpolation 0.837 vs roto 0.870 (snap 6), 10 apart 0.674 vs 0.708, 25 apart 0.141 vs 0.144 (flow chain not trusted, so it falls back; raising `trust` to 1.5 reaches 0.335 at 25 frames but drops 6-frame IoU to 0.72). Hand-traced real finger (keys 12 frames apart, one held-out frame): 0.732 interpolation vs 0.770 roto; that ground truth is hand-traced by eye (about +-5 px), so treat it as indicative only. Memory is half-resolution gray per segment (300 frame cap). Not a replacement for dense matting: gains over interpolation are small (+3 points) and fast large motion is unsolved. Place keys at most about 10 frames apart.
- Speed ramps: planar tracking and the occluder matte read ramp clips at the displayed source frame (`Clip::source_index`, floor(source_time*fps+1e-6)); the pin itself is rendered from keyframes the tracker wrote at output times, so the pin path does not call source_index at render time and a ramp edit needs a re-track; decode is windowed (COLOR patches 127/128). Renderer-aligned tracker vs export max 1.41 px over 87 frames. Reverse/freeze time modes are not handled by the occluder.
- Off-frame prediction (planar): homography-extrapolated motion prior pulls corners toward the predicted quad in proportion to how much of the surface has left the frame. Overtake room test (sign leaves frame left), mean corner error vs ground truth: 33% visible 14.6 -> 5.6 px, 22% 18 -> 7.6 px, 12% 20 -> 14.6 px, 4% 61 -> 34 px. Track still declared lost at about 3% visible (frame 120 of 138). Not solved: true edge-on (width -> 0) needs a camera-motion model, not just a homography prior.
- Graph editor data tools (V1): get_curve (keys, ease, bezier handles + dense value and SPEED samples for a speed graph), move_key, remove_key, copy_curve (offset, time scale, value scale/offset, reverse, invert), simplify_curve (RDP to a tolerance, e.g. 101 tracker keys -> 45 within 1.0), smooth_curve (Gaussian). With set_keyframe_bezier and set_expression these cover the graph editor's data side. Gaps: no multi-property selection/batch edits, no tangent-mode presets (auto/flow/ease-in-out handle types), no value-graph overlay render.
- Planar tracker robustness (real footage): quad validity gate (convex, bounded area/corner jump) and a confidence-drop rule (a sudden fall to under about 0.7-0.8 of the recent mean means occlusion: coast, do not follow). Real-footage case (SCUMBLE reference, overpass face 8.9-11.5 s): tracks the shake settle (about 15 px) at conf 0.87, then a full-frame white title slam covers the surface and the track holds the last good quad instead of latching onto the title (before the gate it diverged to -900 px). Does not re-acquire within that clip because the title stays up. Limit: real-footage coverage is one shot; no long handheld test yet.
- Text on path + randomized selector (V1): set_text_path (points or circle/ellipse/figure8/wave/arc/spiral, per-glyph tangent rotation, animated start progress, flip) and add_text_animator random=true seed=N (AE Randomize Order). Motion-blur sampling includes the path travel. Gaps: no per-character colour animator (text is one coverage mask, one colour), no "based on" modes (words/lines are available via unit), path margin/perpendicular offset not exposed.
- Beat-driven motion (kino exclusive, V1): track_beats (BPM + beat grid from any audio/video asset: onset envelope, autocorrelation tempo, grid refinement) and bind_beats (keyframes any numeric property to hit on beats: pulse, kick, swing, flash curves; every-Nth beat; explicit beat lists). Measured on the SCUMBLE reference (true 125 BPM): estimate 127 BPM, beat phase about 90 ms late versus the known hit times, so for frame-tight sync pass explicit beats or an offset. Gaps: no downbeat/bar detection, no per-band (kick vs hat) envelopes, unreliable on rubato audio.

### Mesh tracking and mesh pin (curved / waving surfaces)
- `set_mesh_pin`: layer pinned onto an nx*ny vertex mesh, per-cell perspective, motion blur. Tier: solid (verified on a synthetic waving flag).
- `track_mesh`: engine-level 3-level image pyramid (coarse-to-fine, 4x/2x/1x) per-vertex photometric Gauss-Newton against the frame-0 patch, sequential with velocity prediction, aperture-safe ridge, neighbour-regularised. Tier: BETA+.
  Measured on a synthetic waving flag (9x6 grid, 100 frames, textured, motion blur): mean vertex error 3.3 px, median 2.2 px, p95 8.6 px (was 12.3 / 7.7 / 30 before pyramid + ridge). 40 of 54 vertices stay under 2 px. 3-4 vertices on horizontal stripes (aperture problem, x unconstrained) still slip 25-110 px late in the shot. Track 14 s for 100 frames.
  Known gaps: no per-vertex occlusion, no topology change, stripes-only regions drift along the stripe.

### Plane-pose motion model (planar tracker core)
- `pose_predict`: constant-velocity extrapolation of a tracked plane in pose space (rotation vector + translation, plane size fixed), used to coast through occlusion and when the surface leaves the frame. Unit-tested exact (1e-12 px) on a synthetic rotating plane. Tier: BETA.
- Measured on the 3D overtake room: coast error at the end of the shot 1407 px -> 1179 px (the sign is fully off-frame there, so this is not visible in the composite). It does NOT extend how far the tracker survives: tracking still ends at about 3% visible, because the data is gone. Using the pose prediction as the always-on prior made tracking worse (noisy pose from 1-2 px quad noise), so it is only used while lost.
- Focal length is assumed 0.9 x frame width (env KINO_F overrides, for tests). No focal self-calibration yet. Edge-on survival below about 3% visible is a data limit, not solved.

### simulate_physics (2D layer physics baked to keyframes)
Motion-graphics physics for layers: circle-proxy bodies (gravity, drag, restitution, friction, canvas walls, collisions, spin) simulated at 480 Hz and baked into dense x / y / rotation keyframes. Scrub-safe, editable in the graph editor, motion-blurs normally. Tier: solid for circle proxies; no polygon/concave collision shapes, no joints, no pins. Example: examples/physics_demo.py (14 shapes dropping and stacking, 5 s, 74 s render at 1280x720 with 180 degree motion blur).

### apply_motion_preset (motion presets and plain-language prompts)
21 named presets (pop_in, bounce_in, elastic_in, slide_in_*, whip_in, spin_in, zoom_punch, shake, glitch_jitter, wobble, float, pulse (bpm), heartbeat, fades, drop_out, drift_zoom) baked to dense per-frame keyframes on x/y/scale/rotation/opacity relative to the clip's current values. A keyword prompt matcher maps phrases like "bounce in from above, then shake" to presets in mention order (one entrance preset max) and the result lists what was applied. Tier: solid for the preset set; the prompt matcher is keyword-based, not an LLM, and returns the available names when nothing matches. Presets are hand-tuned curves, not yet cross-checked against CapCut's exact timing.

### motion_extract / motion_diff (global motion match and compare)
`motion_extract` tracks the whole frame (inset quad, drift-free anchor tracker) and returns per-frame dx, dy, scale, rotation relative to the first frame; with `target_clip_id` it writes them as keyframes so text/graphics ride the footage (match-move), `invert` makes it a stabiliser, `gain` scales it. `motion_diff` compares two clips' global motion (RMS and correlation per channel, worst frame), e.g. a recreation vs its reference.
Measured on a synthetic handheld plate with known motion (100 frames): mean error dx 0.51 px, dy 0.73 px, scale 0.00 %, rotation 0.001 deg; max 1.1 px. Extract 7.3 s for 100 frames. Tier: BETA. Global similarity only: no parallax, no rolling shutter, no depth layers; a surface that changes appearance a lot over the shot (cuts, big occlusions) needs a restart range. Only synthetic ground truth so far, real handheld footage test still owed.

### Real handheld footage test (motion_extract), honest result
Clip: Wikimedia Commons "Wzwz munich petuelpark 09d" (CC0, 540x720, 3.7 s, handheld tilt with a sudden bright burst filling the frame near the end). motion_extract measured a large move (dy 138 px, roll 13 deg, scale 0.72). Match-moved text sits on the rock through the shot, including the burst. Inverting the motion as a stabiliser cut the residual camera motion from std dx 24.5 / dy 36.8 px / rot 3.8 deg to dx 2.7 / dy 22.6 px / rot 0.89 deg, i.e. roll and sideways motion are mostly gone, but vertical motion is only reduced by ~40 %: the burst and heavy motion blur break the whole-frame track in the second half. Tier stays BETA. Needs a restart/segment mode and a robust exposure-jump gate before it can be called solid on real footage. Track 6.3 s for 93 frames at 540x720.

### Explicit ordinary-video delivery (COLOR132)
Ordinary H264 export now converts Rec709 RGB to BT709 limited-range YUV and writes matching primaries/matrix/range plus the exact linear/sRGB/BT709 transfer tag. Disabled projects retain display-sRGB rendering; managed ordinary export rejects other gamuts/transfers instead of leaving players to guess. BT601 delivery is unsupported. High-bit HLG, like PQ, requires Rec2020. Migration: legacy MP4 delivery pixels changed from implicit conversion; re-export older MP4s for the explicit contract. Project rendering/defaults are unchanged.
