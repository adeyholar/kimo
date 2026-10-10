# kino feature checklist

Target: every useful Premiere Pro editing tool and the full After Effects effect range, each callable as an MCP tool.
Legend: [x] shipped and tested, [~] partial / approximate, [ ] pending. Counts are from the live server (`tools/list`, `list_effects`).

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
- [ ] track targeting, sync lock, linked audio+video clips, link/unlink, group clips
- [ ] nested sequences / compositions (pre-compose), sub-sequence edit
- [ ] multicam: sync by audio/timecode, angle switching, multicam to flat
- [ ] subclips, clip labels/colors, bins and metadata, search
- [ ] replace clip (keep effects), paste attributes, effect presets (save/apply), copy/paste keyframes
- [ ] auto reframe (cover-fit decode), scene edit detection, auto-cut to beats / to speech
- [ ] proxy workflow (generate proxy, toggle), offline media relink
- [ ] adjustment layers [x], track mattes [ ], sequence settings per sequence, multiple sequences
- [ ] rolling/regular edit by timecode text ("+10", "-5 frames") helpers
- [ ] captions: SRT import [x], speech-to-text [ ] (needs whisper), caption styling presets [ ], burn-in [x via text]
- [ ] essential graphics templates (parametric titles, lower thirds, animated MOGRT-like presets)

### Transitions (16 now)
- [x] crossfade, wipes x4, slides x4, zoom, spin, flash, iris, blinds, checker, diagonal
- [ ] dip to black/white, film dissolve, push, whip pan, light leak, glitch, luma wipe (custom matte), page peel, morph cut-like, cross zoom, spin blur, ripple dissolve, shape wipes, clock wipe, gradient wipe, split, barn doors, random blocks
- [ ] transition curves/easing and alignment (centered, start, end)

### Color (Lumetri-style)
- [x] exposure, contrast, saturation, temperature, tint, color wheels (lift/gamma/gain), curves (luma S-curve), HSL secondary, LUT (.cube), teal/orange, vignette, color isolate, levels, brightness/contrast, channel mixer, gradient map, colorize, shadow/highlight, threshold
- [x] scopes (waveform/vectorscope/histogram data via get_scopes)
- [ ] RGB per-channel curves, hue-vs-sat/hue-vs-hue/luma-vs-sat curves, qualifier with mask+tracking, auto color match between shots, shot match, auto tone
- [ ] HDR / wide gamut working space, color management (ACES, OCIO), LUT export, film emulation looks, ARRI/Log input transforms
- [ ] denoise (temporal/spatial), sharpen [x] unsharp, dehaze, clarity, lens profile correction

### Audio
- [x] per-clip volume + keyframes, fades, mute, pitch, high/low pass, compressor, echo, denoise flags, reverse, beat detection, export_audio
- [ ] essential-sound style: dialogue loudness normalize, auto-duck music under voice, EQ bands, de-esser, de-reverb, limiter, reverb, noise gate, stereo pan keyframes, surround, audio transitions (constant power/exponential)
- [ ] voiceover record, audio sync by waveform, loudness meter (LUFS) readout

### Titles / text
- [x] text clips with typewriter, font size/color, keyframed transforms, masks
- [ ] text animators (per-char/word/line range selectors: position, scale, rotation, opacity, blur, tracking, color), path text, text presets, rich style (stroke, shadow, background box), variable fonts, 3D extrusion

### Delivery
- [x] mp4 export (parallel, streaming), gif, audio export, still frame
- [ ] export presets (YouTube, TikTok, ProRes, DNxHR, image sequence, WebM/VP9, AV1, HEVC), render queue, background render, in/out range export, alpha export, burn-in timecode, watermark, chapter markers

## B. After Effects: compositing and motion graphics core
- [x] layers: video, image, text, shape (rect, ellipse, polygon, line), adjustment layer, solid-like via shapes
- [x] transform keyframes: position, scale, rotation, opacity with ease (linear, in, out, in_out, hold)
- [x] blend modes (set_blend), masks (animated, feather), corner pin, track point, stabilize
- [ ] anchor point, per-axis scale, 3D layers, camera + lights, depth of field, parenting / null objects
- [ ] bezier keyframe handles (graph editor), spatial paths for motion, auto-orient, wiggle / loop / expression engine (wiggle, loopOut, time, linear/ease, index, valueAtTime), expression controls
- [ ] pre-compose / nested comps with own time, time remap on comps, collapse transformations, continuous rasterization
- [ ] track mattes (alpha, inverted, luma), set matte, stencil/silhouette alpha
- [ ] shape layer operators: fill, stroke, gradient fill, trim paths, repeater, merge/offset paths, zig zag, pucker/bloat, wiggle paths, round corners, twist; bezier paths
- [ ] roto: AI object matte / roto brush (needs ML model), mask tracking, refine edge
- [ ] puppet / mesh warp, bezier warp, liquify, content-aware fill / object removal
- [ ] motion blur (shutter angle, per-layer), frame blending, time-stretch, posterize time
- [ ] camera tracker (3D), planar tracking (corner-pin track), face tracking
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
- [x] brightness/contrast, curves (luma), levels (master), exposure, hue/sat (HSL), tint, colorize, color balance via wheels, gamma/pedestal/gain, photo filter-like via temperature, shadow/highlight, vibrance-like via saturation
- [ ] levels per channel, curves per channel, color balance (HLS), black & white mixer, change to color, change color, selective color, tritone [x via gradient_map], leave color [x color_isolate], auto levels/contrast/color, Lumetri full, vibrance, CC tools (toner), color link, equalize, easy levels, output (clip), PS arbitrary map, color stabilizer, match color

### Distort
- [x] bulge, twirl, ripple, wave warp, turbulent displace, polar coordinates, offset, mirror, motion tile, flip (transform), lens distortion (optics compensation), kaleidoscope-style, corner pin, glitch blocks, pixel sort
- [ ] bezier warp, mesh warp, displacement map (using another layer), liquify, magnify, reshape, smear, spherize, warp (presets: arc, flag, fish...), warp stabilizer (stabilize exists via tracking) , detail-preserving upscale, transform effect, rolling shutter repair, CC (bend it, ripple pulse, page turn, lens, power pin, slant...), wave presets

### Expression controls
- [ ] slider, checkbox, color, point, angle, layer, dropdown controls feeding any param

### Generate
- [x] checkerboard, grid, gradient ramp (linear/radial), fractal noise (via fractal_noise), lightning / energy beam, lens flare, circle/ellipse/rect via shape layers
- [ ] 4-color gradient, advanced lightning variants (strike, branch decay), audio spectrum, audio waveform, beam (laser), cell pattern, eyedropper fill, fill, fractal (Mandelbrot/Julia), paint bucket, radio waves, scribble, stroke, vegas (animated outline), write-on, caustics

### Immersive / VR
- [ ] VR to sphere, VR rotate sphere, VR blur/sharpen/denoise/glow, VR chromatic aberrations, VR plane to sphere, VR convert projection

### Keying
- [x] chroma key (color, tolerance, softness), luma key, spill suppress, choker, feather alpha
- [ ] Keylight-grade keyer (screen gain, balance, clip black/white, despill), color difference key, color range, difference matte, extract, inner/outer key, linear color key, spill suppressor for any color

### Matte
- [x] matte choker (choker), feather alpha
- [ ] refine soft matte, refine hard matte, simple choker variants, matte tools for tracked mattes

### Noise & Grain
- [x] film grain, noise via fractal noise, film damage (scratches, dust, flicker)
- [ ] add grain with presets, match grain, remove grain, dust & scratches (cleanup), median, noise HLS, noise alpha, noise (additive/mono), motion/temporal denoise

### Perspective
- [x] corner pin, drop shadow (alpha), long shadow
- [ ] 3D camera, bevel alpha, bevel edges, radial shadow, CC cylinder/sphere/environment, extrude

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
- [ ] echo (temporal blend), posterize time, time displacement, pixel motion blur / optical flow frame interpolation, frame blending, time warp, timewarp slowmo, CC force motion blur, CC wide time

### Transition effects (as effects, not clip transitions)
- [ ] block dissolve, card wipe, gradient wipe, iris wipe, linear wipe, radial wipe, venetian blinds, CC glass wipe, CC grid wipe, CC image wipe, CC jaws, CC light wipe, CC line sweep, CC radial scale wipe, CC scale wipe, CC twister, CC warpo

### Utility
- [x] letterbox (cinema bars)
- [ ] Cineon converter, color profile converter, grow bounds, HDR compander, apply color LUT [x as lut], tone mapping, compositing for log

### Obsolete / classic still used by pros
- [ ] lens blur, basic 3D, basic text, path text, lightning (classic), channel blur, bevel, bulge-style CC effects, reduce noise

### Third-party style looks (Hollywood staples, implement natively)
- [~] light leaks, anamorphic streaks, lens flare, film grain, halation, bloom [glow], chromatic aberration, lens distortion, VHS, glitch
- [ ] halation (red edge glow), film gate weave, 35mm film emulation, bleach bypass, day-for-night, volumetric light, fog / atmosphere, heat haze [x turbulent_displace], energy shield, hologram, screen replacement (corner pin + tracking), sky replacement (needs segmentation), digital clone/duplicate, speed-line, rain on lens, camera shake presets [x shake], rolling shutter, lens dirt

## D. Infrastructure / performance
- [x] windowed streaming decode (24-frame window), eviction, parallel export, preview cache
- [~] CPU f32 renderer (about 2 fps at 720p for heavy stacks)
- [ ] wgpu pipeline: WGSL effect kernels, GPU compositing, frame textures in VRAM, async readback, hardware encode via ffmpeg/NVENC
- [ ] u8/half-res fast path for glow/blur, SIMD, tile-based rendering, dirty-region cache
- [ ] windowed tracking/stabilization, proxy-based preview, scrubbing from cached low-res
- [ ] expression evaluator (small interpreter) shared by all params
- [ ] project format v2 with compositions, versioned migrations
- [ ] MCP: batch tool (apply many ops atomically), describe_project summary for LLM context, templates (create_title, create_lower_third, create_trailer_titles, make_montage_from_folder)
- [ ] GUI (deferred by the user)

## E. Quality tiers (honest audit)
User bar: every feature best in class. Tiers: **PRO** = matches or beats the pro tool, **V2** = correct maths, missing refinements, **V1** = works, simple maths. Nothing is PRO until verified on real footage against the reference tool's behavior.

| Area | Tier | What is missing for PRO |
|---|---|---|
| Timeline edit tools (ripple/roll/slip/slide/lift/extract) | V2 | linked A/V, track targeting, sync locks, multi-track ripple |
| Color primaries / log wheels / curves / hue curves | V2 | float pipeline is f32 per-pixel and display-referred by default; no scene-linear working space switch per project, curves have 9 knots (no free-form points), no gamut mapping |
| Color space transforms (ACEScct, LogC3, S-Log3, sRGB, Rec.709) | V2 | transfer curves only: no primaries/gamut conversion (AP0/AP1, ARRI WG, S-Gamut3), no RRT/ODT, no OCIO config import |
| Qualifier (HSL key) + Power window | V2 | no pen/bezier window, no mask tracking binding, matte has no denoise/shrink/grow controls |
| Scopes (waveform, parade, vectorscope, histogram) | V2 | no IRE/nits scale labels, no 10-bit/HDR scopes, no CIE chromaticity |
| Film print, halation, bleach bypass | V1 | no measured print-stock LUT/density curves (Kodak 2383 etc.), halation is box-blur based |
| White balance picker, color match | V1 | RGB statistical transfer (not Lab/optimal transport), no shot-to-shot auto match with masks |
| Chroma key (old `chroma_key`) | V1 | superseded by `keyer` |
| Keyer (`keyer`: screen gain+balance, clip black/white, core shrink, edge soften, despill + luma replace, edge unmix, matte view) | V2 | per-pixel screen color variation map (clean plate), garbage matte binding, motion-adaptive matte, hair-detail refinement (guided filter), tested on synthetic screen only |
| Luma key | V1 | |
| Stabilization (two-pass: 24-patch robust similarity fit with outlier rejection, Gaussian path smoothing, lock mode, adaptive zoom, translation+rotation+scale, windowed decode) | V2 | no rolling-shutter correction, no 3D/perspective (homography) model, no inpainting of borders instead of zoom, no tripod/pan-aware modes. Measured: frame-to-frame jitter 9.8 -> 3.5 on a synthetic shaky clip |
| Tracking (ring of 9 patches, translation + rotation + scale, windowed decode, refit from frame 0 each frame) | V2 | no planar/homography tracker, no re-acquire after occlusion, no per-patch confidence UI, SAD matching (no ZNCC / optical flow refinement) |
| Motion blur (`set_motion_blur`: shutter-angle multi-sample of x/y/scale/rotation, 4-16 samples) | V2 | no per-effect/param blur, no optical-flow blur for source footage, adjustment layers/text animators not covered |
| Renderer | V1 | alpha-correct bilinear + 2x2..4x4 supersampling on transforms (done); still CPU f32, no mipmaps/Lanczos for large downscales, no GPU |
| Transitions | V2 | 29 kinds, 10 easings, soft angled wipes, clock, split, iris, noise dissolve, dips, light leak, glitch, whip pan, zoom blur. Missing: outgoing-clip push/slide-out, luma-matte wipes from image, GPU-shader transitions |
| Particles | V2 (emitter) | add_emitter: 11 presets (incl. dust, smoke), forces, turbulence, attractor, floor bounce, trails, fade_in, sprites soft/hard/mote/puff/streak/ring/star/spark; no layer collisions, no texture sprites, no fluid sim |
| Text | V2 | kerned layout, word wrap, AE-style char/word/line animators with range selectors (6 shapes, keyframable) + 9 presets, stroke, blurred shadow, glow, gradient, rounded bg box. Missing: per-span rich text, text on path, variable fonts, emoji |
| Masks | V2 | no bezier paths with per-vertex keyframes, mask expansion |
| Audio mixer | V1 | no EQ bands, no loudness normalize, no ducking |

- [x] HUD tools: add_detection_box, add_counter, add_prompt_box (V1)
- [x] shatter transition V2: radial + concentric spiderweb cracks that grow from an impact point, domain-warped irregular shards, impact flash and shockwave ring, shards of the outgoing frame tumble in true 3D perspective with refraction offset, edge glints and fade. Gaps: no depth-of-field blur on near shards, no secondary micro-fragment cascade, no sound. ("shatter_cells" keeps the old Voronoi reveal.)
- [x] 3D layer system V1 (three_d.rs): any layer becomes a perspective plane via rot_x, rot_y, rotation(z), pos_z, x, y, scale, backface; project camera via set_camera / set_camera_keyframe (cam_x/y/z, pan/tilt/roll, fov). Exact inverse perspective mapping with 2x2 supersampling. Shatter shards use the same Projector. Gaps: layers are composited in track order (no depth sort yet), no motion blur on 3D layers, no lights/shadows, no extruded 3D text, no mesh import.

## Tests
`python3 tests/tool_fuzz.py` calls every MCP tool with empty, null, wrong-type, negative, huge and real-id arguments (about 2400 calls) and fails on crash, hang or malformed reply. export/render tools are skipped (slow).

## Shard burst (add_shard_burst) - V2
3D glass-shard cloud generator: hundreds of prism shards, project-camera projection, internal depth sort, DOF softness, rim glints with dispersion, 4-sample motion blur. Gaps: no true refraction of the background, no shadows/lights, not GPU.

## 3D suite status
Done: layer transforms, camera, shard-internal depth sort/DOF/motion blur. Todo: layer depth sorting, DOF and motion blur on 3D layers, materials, lights, shadows, nulls/parenting, extrusion, camera rigs (orbit, look-at).

## Rigid-body physics (physics.rs, used by add_shard_burst physics:true) - V1
Deterministic 240 Hz baked sim: mass from size, linear/angular velocity, gravity, linear and angular drag, floor plane with restitution + Coulomb friction (spin from contact), body-body impulse collisions (sphere proxies, spatial-hash broadphase). Scrub-safe via cached trajectory. Gaps: sphere proxies not true convex hulls (no resting stability on faces), no joints/constraints/soft bodies/fluids, no general-layer physics tool yet (only shard bursts), CPU.
Shard shading: Schlick fresnel and depth falloff params. Gaps: no real background refraction (generator layer has no access to what is behind it), no caustics, no area lights.

## SCUMBLE hero shatter recreation (examples/scumble_shatter.py) - V1 (recreation, not parity)
Composed from existing tools: keyboard bg with push-in and blur ramp, finger/key sprites, orange spark emitter, physics shard burst with camera fly-in and zoom-blur streaks, detection-box HUD. Gaps vs reference: finger/key/keyboard are drawn stand-ins (no real macro footage or 3D model), shards lack true refraction of the scene, no caustics, lighting is fake, no rigid-body interaction with the finger.

## Background-aware refraction (shard burst refract:px) - V1
The shard pass writes a screen-space displacement field (tilt x thickness, stronger on side walls, motion-blurred with the shards); the compositor bends the canvas behind the layer before blending. Gaps: single offset (no per-channel dispersion of the background), displacement clamped, shard layer only (not a general refractive material for any layer), no caustics.

Shard geometry V1.5: 4-6 vertex irregular convex chunks with varied aspect and thickness. Gaps: still extruded polygons, not fractured volumes (no Voronoi-fractured solids, no bevel normals).

## Scene3D (add_scene3d) - V1
Real geometry renderer: z-buffered triangles, box/rounded-box/cylinder/capsule/sphere/plane/null, parent transforms (nulls), keyframed pos/rot/scale per object, PBR-style materials (albedo, roughness, metal, emissive, opacity, Schlick fresnel, Blinn-Phong GGX-like spec), directional and point lights, ambient, 2x2 SSAA, depth-of-field from the depth buffer, transparent objects sorted back to front. Seen through the project camera, composes with shard bursts and everything else. Gaps: no shadows or ambient occlusion, no global illumination/reflections/env maps, no textures/UVs, no mesh import (OBJ/glTF), no extrusion of text/shapes, no motion blur on scene geometry, transparency is per-object sorted (no per-pixel OIT), CPU only.

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
- [ ] 1. Track mattes (alpha/luma/inverted), collapse transforms. DONE V1: precomps (group_clips / ungroup_clip: nested tracks on a local clock, group transform/effects/mask/opacity/keyframes apply to the whole group; gaps: no editing of grouped clips in place, no time remap of the group, no continuous rasterization, nested audio not mixed)
- [~] 2. Expressions V1 DONE: op stack per property/effect param (wiggle w/ octaves+seed, loop_in/out cycle|pingpong|offset|continue, sin, inertia, posterize_time, clamp, round) via set_expression; cubic-bezier key handles via set_keyframe_bezier (overshoot ok). Gaps: no free-form expression interpreter (linkage, valueAtTime, index), no per-axis 2D wiggle correlation, no handle UI/ separate in/out influence. TODO still: spatial motion paths with auto-orient, text animators polish
- [~] 3. Echo V1 DONE (set_echo: composite/add/screen/average frame-blend ghosts, +/- time; set_posterize_time on whole clip). Gaps: no optical-flow interpolation/blur, no time displacement, no rolling-shutter repair. Original: Echo, posterize time, frame blending, optical-flow interpolation and slow-mo, optical-flow motion blur for footage, time displacement, rolling-shutter repair
- [ ] 4. Motion blur for Scene3D geometry and 3D layers, per-effect blur
- [ ] 5. Planar/homography tracker, mask tracking, occlusion re-acquire, 3D camera solve, face tracking, warp stabilizer parity, object removal
- [ ] 6. Roto/segmentation (ML model), puppet/mesh warp, liquify, Resolve-style smart reframe, speed warp, optical-flow retime
- [ ] 7. Kino-exclusive ideas: beat-driven motion (bind any property to audio::analyze beats), motion-by-prompt presets (AI writes the curve set), physics-driven layers (any 2D layer joins the rigid-body sim), motion "diff" tool (compare two clips' motion curves and match one to the other), auto motion-match to reference footage via tracker.
- [ ] Infra: HUD timecode as one dynamic text source (currently 2 clips per frame, slow export)

- [x] impact landing for detection boxes (impact:true), add_box_expand (constant-speed expansion + 2-frame flash), add_camera_shake / impact_shake (damped spring, measured from SCUMBLE), add_hud_frame, dynamic text tokens {tc} {bar} {t} {pct}. V1: measured from one reference, not generalized beyond it; see examples/scumble_*.py for known diffs.

## Sandbox layer (guard.rs, built into every tool call)
- [x] V1: argument sanitizer (finite numbers, magnitude +-1e8, array/string size, nesting), project limits checked after every mutation with automatic rollback (canvas pixels, fps, clip count, timeline duration), memory budget via counting allocator (default 70% of RAM or cgroup limit; Frame allocations and the render loop fail the call instead of OOMing; hard backstop refuses allocations far above budget), cooperative cancel plus wall-clock time limit per call (default 600 s, export 7200 s) with a last-resort clean server exit if a call cannot be stopped, panics caught with project rollback. Env overrides: KINO_MEM_MB, KINO_TIMEOUT_S, KINO_EXPORT_TIMEOUT_S, KINO_MAX_PIXELS, KINO_MAX_FPS, KINO_MAX_CLIPS, KINO_MAX_DURATION_S, KINO_MAX_NUM. Test: tests/sandbox_test.py.
- Gaps (honest): in-process, not OS-level isolation (no seccomp/namespaces/cgroups; a native-code bug that corrupts memory still kills the server); memory accounting covers Rust heap only, not ffmpeg child processes or GPU memory; cancellation is checked per clip and per exported frame, so a single very slow effect on one frame can overrun the limit by that frame's time; ffmpeg children get no separate CPU/memory limits yet; file paths are not jailed (tools can read/write any path the user can).

- add_speed_lines (manga radial rays, flicker/spin/bg). Gaps: flat-colored rays only, no per-ray gradient or noise streak variant, CPU O(pixels x rays) (~0.1 s/frame at 70 rays).
- add_camera_shake now uses a measured per-frame kick table ([-19,-10],[-11,-6],[-4,-2] px @720p, discrete frames, same as the SCUMBLE reference; pass profile for others, profile='spring' for the old damped spring). Verified frame by frame with tools/bracket_track.py + examples/shake_align.py (kick frames and values identical to the reference at 4.12/4.60/5.08). Gaps: per-hit amplitude differences in the ref (6.04 hit is ~1.3x) and later hits 6.5-7.5 not yet verified because the HUD bracket is covered there.

- `silhouette_fill` (effect): flat-color silhouette of a clip's alpha, mix amount, optional outline ring. Tier: solid. Gap: no pattern fills, no auto-segmentation (mask comes from an RGBA cutout).
- `add_glitch_confetti` (generator): random flat squares + horizontal tear lines, reshuffled at flicker_fps. Tier: solid. Gap: no block displacement of the plate under it (use glitch_blocks).
- Masking stack (masks.rs): see docs/MASKING_ROADMAP.md for the full table and honest tiers (ML roto, mask tracking, per-point feather QUEUED).
- [x] 2b. Spatial motion paths (set_motion_path / remove_motion_path): bezier or through-points (auto-smooth Catmull-Rom) or presets (circle, ellipse, figure8, wave, arc, spiral), constant-speed arc-length progress via keyframable property `path` (ease, bezier handles, expressions work), auto-orient with offset and corner smoothing, correct under motion blur and echo. Gaps: no per-point speed handles (use eased `path` keys), no 3D paths, no path editing UI.

### Planar tracker (mocha-style) - built, tiers
- `track_planar` (homography, 4 corners) + `pin_layer` (perspective corner pin, keyframed, 3x3 supersampled AA edges, premultiplied) + motion blur on the pinned layer from the tracked motion + `light` brightness match.
- PARITY with Mocha/AE/Resolve: sub-pixel planar track, perspective pin, occluder-robust (Tukey weights), exposure-invariant (zero-mean/unit-variance photometric model).
- BEYOND (measured): drift-free by re-aligning to the frame-0 anchor each frame; coasts through full occlusion and re-acquires with a coarse search. Synthetic perspective move + h264 + 180deg blur + occluder: 0.33 px mean / 0.39 px p95 corner error vs ground truth (examples/planar_demo.py, tools/wall_plate.py).
- NOT YET: mesh/grid tracking for curved or deforming surfaces, tracking backward from a middle frame, multiple anchors for long appearance change, occluder auto-matte (use mask tools), grain/lens match on the pinned layer. Marked queued, not done.
- Occluder auto-matte (`track_planar occluder_matte=true`): at render time the live frame is compared with the frame-0 surface (warped by the tracked quad, brightness-matched); wherever they differ the pinned layer is cut, with 2 px grow and feather. Tier: parity with a Mocha insert-matte workflow minus manual roto; limits: occluder with the same colour as the surface is not detected, anchor lives in the session (re-run track_planar after reload), no temporal smoothing of the matte yet.
- Off-frame prediction (planar): homography-extrapolated motion prior pulls corners toward the predicted quad in proportion to how much of the surface has left the frame. Overtake room test (sign leaves frame left), mean corner error vs ground truth: 33% visible 14.6 -> 5.6 px, 22% 18 -> 7.6 px, 12% 20 -> 14.6 px, 4% 61 -> 34 px. Track still declared lost at about 3% visible (frame 120 of 138). Not solved: true edge-on (width -> 0) needs a camera-motion model, not just a homography prior.
- Graph editor data tools (V1): get_curve (keys, ease, bezier handles + dense value and SPEED samples for a speed graph), move_key, remove_key, copy_curve (offset, time scale, value scale/offset, reverse, invert), simplify_curve (RDP to a tolerance, e.g. 101 tracker keys -> 45 within 1.0), smooth_curve (Gaussian). With set_keyframe_bezier and set_expression these cover the graph editor's data side. Gaps: no multi-property selection/batch edits, no tangent-mode presets (auto/flow/ease-in-out handle types), no value-graph overlay render.
- Planar tracker robustness (real footage): quad validity gate (convex, bounded area/corner jump) and a confidence-drop rule (a sudden fall to under about 0.7-0.8 of the recent mean means occlusion: coast, do not follow). Real-footage case (SCUMBLE reference, overpass face 8.9-11.5 s): tracks the shake settle (about 15 px) at conf 0.87, then a full-frame white title slam covers the surface and the track holds the last good quad instead of latching onto the title (before the gate it diverged to -900 px). Does not re-acquire within that clip because the title stays up. Limit: real-footage coverage is one shot; no long handheld test yet.
