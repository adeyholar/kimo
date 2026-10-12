# Architecture and v2 stack: branch manifest
Each remote branch is a single-commit snapshot on top of main (web upload); each branch is built on the previous one in this list. SHAs are the REMOTE snapshot commits (git ls-remote). Per-unit format-patch files live in patches/ on the branch that introduced them (older branches keep theirs; this branch carries only the latest). Merging anything to main needs the user's explicit word.

| branch | remote sha | summary | tests at the time |
|---|---|---|---|
| arch-g4 | 4db8fdc | streamed save_project history, no JSON tree (fixes OOM at 1000+ clips) | history proofs, fuzz, golden |
| arch-stepb | 820295c | Track.clips is Vec<Arc<Clip>>, structural sharing (set_property 3000 clips 9.73 to 3.84 ms, peak RSS 1023 to 39 MB) | arc_sharing, arc_property_proof, arc_bench |
| arch-gab | 47ace15 | atomic save, project_sha history header, stale .history removed | history_persist_proof |
| arch-s2 | 3a2f14e | fsync before rename, old history removed before write, extended property proof | arc_property_proof |
| arch-s3 | 82f1982 | no-op COW in shift_after, skip_serializing_if refract_dispersion, v1 compat fixture | v1_compat_proof, golden |
| arch-s5 | 9b9b340 | ext tools use find_clip_mut, auto_duck compare-first | fuzz, golden |
| arch-u9 .. u9g | 102b648 ec86db2 1f78bd8 8120d1e f98533c bb39999 f7f4b44 | tool_contracts rows, insert_gap validation, freeze_frame (hold_from_here), smooth_curve pin_ends, dangling render rows | tool_contracts 27 |
| arch-u9h | a8917ff | auto_duck unity envelope stays keyless; WAV sharing test (ignored, needs ffmpeg) | tool_contracts 28 with --include-ignored |
| arch-u9i | 986ca8c | clear_refs_to / dangling_refs / cleared_refs on delete (BEHAVIOUR CHANGE) | tool_contracts 31, fuzz 0 failures |
| arch-u9j | 33a74d6 | grade scopes cleared on delete (was rejected), duplicate_clip keeps binding (BUG FIXES) | tool_contracts 35 |
| arch-u9k | 80ee975 | rows for set_track, remove_silence_cuts, set_camera, add_counter, add_prompt_box, get_scopes | tool_contracts 41 |
| arch-u9l | ea79a0d | central track lock enforcement in dispatch (BEHAVIOUR CHANGE) | tool_contracts 42, fuzz 0 failures |
| arch-u9m | a4ecc8d | tests/track_diag_proof.py first pass (translation plate) | proof OK, mutation checked |
| arch-u9n | 16a682e | track_diag_proof: similarity plate, negative object plate, error field, corner disagreement | proof OK, 2 mutation checks |
| arch-u10 | 8f4e335 | new tools loop_clip, distribute_clips; proof threshold tightening | see report |
| arch-u11 | da4d6d7 | swap_clips (+ mutation checks for loop_clip, distribute_clips, swap_clips) | tool_contracts 45, fuzz 263 tools 0 failures |
| arch-u12 | d3c6144 | error text golden baseline ONLY (41 bad calls) | error_table 2 pass |
| arch-u12-1 | b87fbba | baseline + commit A: ToolError, classify, dispatch_ex (text unchanged; duplicate remote name, real name for commit A) | fuzz 0 failures |
| arch-u13 | 209f53d | commit B: bad_arg_type, unified no such clip/track 'ID', io_error, invalid_value hints; 10 intentional text rows | fuzz 263/9545/0, golden, tool_contracts 45, error_table 2 |
| arch-u14 | f998adc | commit C: trim_clip/set_camera/add_shape validation; unknown-argument warnings (KIMO_STRICT_ARGS=1 strict, KIMO_AUDIT_ARGS=file audit) | fuzz 263/9545/0, golden, tool_contracts 45, error_table 2 |
| arch-u15 | b5f154d | set_property rejects time (invalid_argument_combo), did-you-mean on missing_arg, 6 frosted proofs + golden scene drop ignored time=0 | fuzz 263/9545/0, golden frames unchanged, contracts 45, error_table 2 |
| arch-u16 | 3ffee44 | set_text_style honors line_mask (was silently dropped), tests/text_line_mask_proof.py | proof OK, fuzz 0 failures |
| arch-u17 | efa0230 | ARCHITECTURE_AUDIT build/test timings + dead-arg candidates, tests/run_proofs.sh (stray-file guard), tests/dead_arg_report.py, undo_coverage_proof writes /tmp only | no engine change |
| arch-u18 | 4bc9f3c | tests/render_bench.py + render baseline table in ARCHITECTURE_AUDIT (unit 8 step 0, measure only) | no engine change |

Tool count: 263 on arch-u10 (README says 255, update at merge time).
| v2 | this branch | main + whole stack u1-u18 + test fix (grade_scopes: Arc<Clip>, delete clears refs) | consolidated, main untouched |

## v3 (single moving branch, from the v2 tip)
| unit | commit | what | notes |
|---|---|---|---|
| 6a | bcf8d88 | RenderCtx replaces WORK_LINEAR; per-call Cancel token, guard::spawn_with_token / scope_spawn; render_purity (fails-first, 227/400 wrong on old code), render_ctx_key, thread_hygiene | no output change; golden 30 frames unchanged |
| neg-time | b582933 | keyframe tools reject negative time (out_of_range) | also on v2 as dc6d4ba |
| merge | merge of v2 3d26215 | export memory (ceiling, budget, low_memory, export_plan), audio:false, join_export, AAC -shortest fix | one conflict (guard.rs) resolved |
| M3 | e1b4d99 | frame_buffers(project) 4/9/12 planner, frame_live_peak (8 scenes, worst precomp depth 3 = 8.0), render_bench | simple 1080p: 1 -> 2 workers on the 2 GB box |
| M1/M5c | 9af7178 | Frame::try_new (Err not abort), worker_identical_proof, ranged-export proof | decode buffers still plain Vec |
| V3-3 | 6e36799, e3bb0f7 | rect shapes rasterize only inside their bbox + margin (tests/shape_bound_equiv, 240 scenes, kill switch set_shape_bound); layer alpha bbox taken from that region when no effects/mask/transition/pin/color pipeline (tests/shape_hint_equiv, 375 scenes, set_shape_hint) | 1080p same-session A/B, ms/frame off -> on: small rect 70.4 -> 53.2 (-24.4%), +blur -12.9%, +exposure -17.6%, +glow -20.3%; bbox-from-region 62.0 -> 57.8 (-6.8%, mean of 3 rounds). Ellipse excluded (approximate SDF leaks past its box, found by the test). Mutations: margin -12 px and region -6 px both fail their tests. |
| V3-3 negative | none (reverted) | (a) bbox-only blend: no gain (-0.9..+4.9%, noise), retracted. (b) calloc-style Frame zeroing: no gain. (c) pooled source buffers: bit-identical but no steady-state gain; its mutation check did not fail (test mostly hit the transform path), so not shipped | The ~22-27 ms "Frame::new" seen in profiles was first-touch page faults on a cold heap; after glibc recycles the 31 MB block the cost is ~0-3 ms. Do not retry allocation tricks. |
| V3-3 backlog | - | blend for layers that are not bbox-bounded (~9 ms/frame at 1080p for a small layer) | measured with KIMO_PROF "blend" |
