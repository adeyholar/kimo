#!/usr/bin/env python3
"""Author the 16 demo scenarios (one per mockup) as engine events.

Writes demo/scenarios.json. Each scenario is the event stream an engine would
send for that screen: a session reset, the mode, the panel state (view
patches), stats, and the tool calls with their real durations. The viewer
frames are crops of the mockups under demo/frames/ (demo only; never part of
the shipped ui/ bundle).
"""
import json, os
HERE = os.path.dirname(os.path.abspath(__file__))
F = lambda n: f"../demo/frames/{n}.jpg"

def K(k, v, **kw):
    d = {"k": k, "v": v}; d.update(kw); return d
def S(t, **kw):
    d = {"sec": t}; d.update(kw); return d

SC = []
def scen(id, mode, project, agent, elapsed, calls, views, stats=None, activity=None, final=None):
    SC.append(dict(id=id, mode=mode, project=project, agent=agent, elapsed=elapsed, calls=calls,
                   views=views, stats=stats or {}, activity=activity, final=final or {}))

def hms(s):
    h, m, x = s.split(":"); return int(h) * 3600 + int(m) * 60 + int(x)

# ---------------------------------------------------------------- m16 edit
scen("m16", "edit", "dune_film.kimo", "codex", hms("00:12:48"),
  [["open_project", ["dune_film"], 200], ["import_media", ["dune_aerial.mov"], 400], ["import_media", ["walk_ridge.mov"], 400],
   ["cut", ["clip_02", {"at": "00:06:12"}], 100], ["trim", ["clip_03", {"in": "00:02.10"}], 100], ["add_transition", ["cross", "12f"], 200],
   ["mix_audio", ["music", "-14 LUFS"], 600], ["caption", [{"track": "en"}], 1800], ["grade", ["clip_04", {"warmth": "+18"}], 800]],
  {"viewer": {"src": F("m16"), "tag": "grade - clip_04", "tc": "01:00:14:07", "res": "1920 x 1080 24p"},
   "insp": {"sub": "clip_04 · dune_ridge", "blocks": [K("Position", "0, 0"), K("Scale", "104%"), K("Rotation", "0.0"), K("Opacity", "100%"),
            S("Grade"), K("Temperature", "6050K", **{"from": "5600K"}), K("Tint", "+2"), K("Saturation", "1.08"), K("Contrast", "1.12")]},
   "tl": {"ruler": {"t0": 0, "t1": 21.9, "step": 5, "last": 20, "minor": 1, "align": "left"}, "playhead": 9.5, "tracks": [
      {"label": "V2", "clips": [{"s": 0, "e": 9.95, "label": "dune_aerial", "thumb": F("m16_t1")}, {"s": 10.1, "e": 21.9, "label": "sky_wide", "thumb": F("m16_t2")}]},
      {"label": "V1", "clips": [{"s": 0, "e": 7.6, "label": "walk_ridge", "thumb": F("m16_t3")}, {"s": 7.72, "e": 14.37, "label": "dune_wide", "thumb": F("m16_t4")}, {"s": 14.54, "e": 21.9, "label": "dune_ridge", "thumb": F("m16_t5"), "cls": "sel"}]},
      {"label": "A1", "area": {"seed": 3, "amp": .45}},
      {"label": "A2", "area": {"seed": 8, "amp": .35, "base": .6}},
      {"label": "CAP", "clips": [{"s": 0, "e": 5.0, "label": "The desert keeps time.", "cls": "cap"}, {"s": 7.95, "e": 11.3, "label": "Every step erased.", "cls": "cap"}, {"s": 14.9, "e": 18.8, "label": "Walk anyway.", "cls": "cap"}]}]}},
  stats={"cpu": 61, "ffmpeg": "7.1", "saved_s": 2}, final={"count": 41})

# ---------------------------------------------------------------- m01 captions
scen("m01", "captions", "dune_film.kimo", "codex", hms("00:18:36"),
  [["transcribe", ["clip_03", {"lang": "en"}], 2400], ["transcript_to_captions", [{"track": "CAP"}], 300], ["set_caption_style", ["kinetic_pop"], 100],
   ["add_title", ['"DUNE"', {"t": "00:00"}], 100], ["add_text_animator", ["title_01", "fade_up"], 200], ["set_text_path", ["title_02", "arc_01"], 100],
   ["validate_captions", ["CAP"], 400], ["transcript_edit_word", ["w_142", '"erased"'], 600]],
  {"viewer": {"src": F("m01"), "tc": "01:00:18:02"},
   "insp": {"sub": "title_01 · DUNE", "blocks": [K("Font", "Geist 500"), K("Size", "96"), K("Tracking", "+40"), K("Animator", "fade_up"), K("Range", "0 – 100%"),
            K("Offset Y", "24px"), K("Opacity", "0 – 100"), K("Stagger", "2f"), S("Captions"), K("Style", "kinetic_pop"), K("Max chars", "32"), K("Check", "0 issues")]},
   "trans": {"lines": [{"spk": "S1", "tc": "00:09", "text": "The desert keeps time."},
                       {"spk": "S1", "tc": "00:14", "html": "Every step <s>erase</s> <span class=\"fix\">erased</span> ."},
                       {"spk": "S1", "tc": "00:19", "text": "Walk anyway."}]},
   "tl": {"ruler": {"t0": 0, "t1": 20, "step": 5, "ticks": False, "edge": True}, "playhead": 9.67, "tracks": [
      {"label": "V2", "clips": [{"s": 0, "e": 10.17, "label": "walk_ridge", "cls": "mauve", "thumb": F("m01_t1")}, {"s": 10.36, "e": 20, "label": "dune_ridge", "cls": "mauve", "thumb": F("m01_t2")}]},
      {"label": "V1", "lane": "#252422", "clips": []},
      {"label": "A1", "area": {"pts": [1.0, 0.83, 0.49, 0.34, 0.57, 0.63, 0.43, 0.23, 0.4, 0.66, 0.86, 0.86, 0.91, 1.0], "bg": "#252422", "fill": "#5f5f59"}},
      {"label": "CAP", "clips": [{"s": 0, "e": 1.64, "label": "The", "cls": "cap"}, {"s": 1.78, "e": 4.03, "label": "desert", "cls": "cap"}, {"s": 4.19, "e": 6.33, "label": "keeps", "cls": "cap"}, {"s": 6.47, "e": 8.38, "label": "time.", "cls": "cap"},
                                 {"s": 8.52, "e": 10.58, "label": "Every", "cls": "cap"}, {"s": 10.74, "e": 12.52, "label": "step", "cls": "cap"}, {"s": 12.68, "e": 15.12, "label": "erased.", "cls": "cap emb"},
                                 {"s": 15.29, "e": 17.21, "label": "Walk", "cls": "cap"}, {"s": 17.34, "e": 20.0, "label": "anyway.", "cls": "cap"}]}]}},
  stats={"cpu": 58, "ffmpeg": "7.1", "saved_s": 1})

# ---------------------------------------------------------------- m02 batch
scen("m02", "batch", "dune_film.kimo", "codex", hms("00:31:20"),
  [["history_list", [], 100], ["checkpoint", ['"before_recipe"'], 200], ["relink_media", ["/footage/dune"], 600], ["recipe_apply", ["film_look", {"clips": 6}], 1400],
   ["template_apply", ["lower_third_01"], 300], ["project_diff", ["cp_11", "cp_12"], 200], ["undo", [], 100], ["batch", ["grade", {"clips": 6}], 2100]],
  {"viewer": {"src": F("m02"), "tc": "01:00:09:12", "thumbs": [{"src": F(f"m02_t{i}"), "check": i < 5, "sel": i == 5} for i in range(1, 7)]},
   "hist": {"items": [{"id": "cp_12", "name": "after_recipe", "ago": "now", "live": True, "src": F("m02_h1"), "sel": True},
                      {"id": "cp_11", "name": "before_recipe", "ago": "2m", "src": F("m02_h2")}, {"id": "cp_10", "name": "captions_done", "ago": "9m", "src": F("m02_h3")},
                      {"id": "cp_09", "name": "rough_cut", "ago": "18m", "src": F("m02_h4")}, {"id": "cp_08", "name": "import", "ago": "31m", "src": F("m02_h5")}]},
   "diff": {"title": "Diff", "titleSmall": "cp_11 to cp_12", "diff": [["+", "node look clip_02"], ["~", "  saturation 1.00 -> 1.10"], ["~", "  contrast   1.00 -> 1.14"], ["~", "  grain 0 -> 12"], ["-", " lut old_rec709.cube"], ["+", "title lower_third_01"]],
            "batch": {"title": "Batch grade", "meta": "4 / 6", "rows": [["clip_01", "film_look", "done"], ["clip_02", "film_look", "done"], ["clip_03", "film_look", "done"], ["clip_04", "film_look", "done"], ["clip_05", "film_look", "grading"], ["clip_06", "film_look", "queued"]]}},
   "media": {"items": [{"name": "dune_aerial.mov", "note": "relinked", "link": True, "src": F("m02_m1")}, {"name": "walk_ridge.mov", "note": "relinked", "link": True, "src": F("m02_m2")}, {"name": "sky_wide.mov", "note": "relinked", "link": True, "src": F("m02_m3")}, {"name": "wind_sfx.wav", "note": "relinked", "link": True, "src": F("m02_m4")}],
             "foot": "collect_project: 14 files, 6.2 GB"}},
  final={"progress": {"done": 4, "total": 6}, "running": True})

# ---------------------------------------------------------------- m03 graph editor
def curve(f, a, b, n=60):
    return [[a + (b - a) * i / n, f(a + (b - a) * i / n)] for i in range(n + 1)]
import math
scen("m03", "graph", "skate_edit.kimo", "codex", hms("00:27:40"),
  [["add_null", ["ctrl"], 100], ["set_parent", ["title", "ctrl"], 100], ["set_motion_path", ["ctrl"], 200], ["bind_beats", ["scale", 2], 300], ["set_expression", ["rot"], 100], ["set_keyframe_bezier", ["y"], 200]],
  {"viewer": {"src": F("m03"), "left": "00:00:03:12", "res": "1920 x 1080 24p"},
   "insp": {"sub": "ctrl", "blocks": [S("Transform"), K("Position", "960, 412"), K("Scale", "104%"), K("Rotation", "12.0"), S("Expression"), {"code": "wiggle(2, 8)\n+ beat * 6"}, S("Motion blur"), K("Shutter", "180")]},
   "graph": {"meta": "beats 120 bpm", "ruler": {"t0": 0, "t1": 5.79, "step": 1, "fmt": "s", "align": "left", "ticks": False}, "beats": {"t0": 0.01, "step": 0.1}, "playhead": 3.06,
      "lanes": ["position y", "scale", "rotation"],
      "curves": [{"smooth": True, "color": "#e9e4d8", "pts": [[0, .188], [1, .837], [2, .656], [3, .276], [4, .388], [5, .419], [5.79, .419]],
                  "keys": [[0, .188], [1, .837], [2, .656], [3, .276], [4, .388], [5, .419]]},
                 {"smooth": True, "color": "#8b877f", "pts": [[0, .188], [1, .613], [1.5, .70], [2, .66], [3, .419], [4, .47], [5, .625], [5.79, .681]],
                  "keys": [[1, .613], [3, .419], [5, .625]]},
                 {"smooth": True, "color": "#66625b", "pts": [[0, .188], [1, .55], [2, .188]], "keys": [[1, .55], [2, .188]]},
                 {"em": True, "smooth": True, "pts": [[2, .188], [2.3, .22], [2.6, .42], [2.85, .70], [3, .818], [3.2, .893], [3.5, .84], [3.8, .66], [4, .594], [4.5, .63], [5, .656], [5.79, .613]],
                  "handles": [[[2.716, .651], [3.254, 1.0]]], "keys": [[3, .818], [4, .594], [5, .656]]}]}},
  )

# ---------------------------------------------------------------- m04 mixing
scen("m04", "mixing", "dune_film.kimo", "codex", hms("00:21:09"),
  [["measure_loudness", ["mix"], 900], ["detect_beats", ["music_01"], 700], ["audio_transients", ["sfx_wind"], 700], ["set_audio", ["dialogue_01", {"gain": "+2 dB"}], 100],
   ["set_audio_pan", ["sfx_wind", "-0.30"], 100], ["add_audio_effect", ["dialogue_01", "eq"], 200], ["normalize_loudness", ["mix", "-14 LUFS"], 1100], ["auto_duck", ["music_01", {"under": "dialogue"}], 800]],
  {"viewer": {"src": F("m04"), "tc": "01:00:21:14"},
   "audio": {"sub": "dialogue_01", "blocks": [{"widget": "eq", "h": 92, "bands": [{"f": 90, "g": -9, "type": "lowshelf"}, {"f": 1800, "g": 4, "q": 1.2, "sel": True}, {"f": 7000, "g": -2, "q": 1.5}]},
              {"widget": "comp", "h": 76, "threshold": -18, "ratio": 3, "style": "border:none;background:none"}, K("Attack", "10 ms"), K("Release", "120 ms"), K("Pan", "C")]},
   "tracks": {"ruler": {"t0": 0, "t1": 18, "step": 5}, "playhead": 9.2, "tracks": [
      {"label": "Dialogue", "lane": "#272521", "wave": {"seed": 21, "amp": .8, "color": "#d9d1bf"}},
      {"label": "Music", "lane": "#272521", "wave": {"seed": 34, "amp": .7, "color": "#d9d1bf", "beats": [0.5 + i * 0.5 for i in range(35)], "env": [[0, .8], [5.6, .8], [6.4, .45], [12.4, .45], [13.2, .8], [18, .8]]}},
      {"label": "SFX", "lane": "#272521", "wave": {"seed": 55, "amp": .5, "color": "#d9d1bf"}}]},
   "loud": {"lufs": "-14.2", "target": "-14", "rows": [["True peak", "-1.3 dBTP"], ["LRA", "6.1 LU"]], "meters": [{"l": "L", "v": .72, "peak": .8}, {"l": "R", "v": .66, "peak": .76}]}})

# ---------------------------------------------------------------- m05 color science
scen("m05", "color", "dune_film.kimo", "codex", hms("00:26:51"),
  [["analyze_exposure", ["clip_07"], 500], ["render_scope", ["clip_07", "waveform"], 200], ["render_scope", ["clip_07", "vectorscope"], 200], ["render_exposure_false_color", ["clip_07"], 300],
   ["render_ocio_display_view", ["ACES2", "PQ"], 600], ["render_scope", ["clip_07", "cie1931"], 300], ["hdr_validate", ["clip_07", "PQ 1000"], 400], ["render_gamut_excursions", ["clip_07"], 900]],
  {"viewer": {"src": F("m05"), "tags": [{"text": "source"}, {"text": "false color", "pos": "tr"}], "tc": "01:00:42:19", "res": "3840 x 2160 24p"},
   "pipe": {"blocks": [K("input", "ARRI LogC4"), K("working", "ACEScg"), K("view", "ACES 2.0", emb=True, bar=True), K("output", "Rec.2100 PQ"), S("HDR"),
             K("MaxCLL", "812 nits"), K("MaxFALL", "214 nits"), K("Peak", "1000 nits"), K("Out of gamut", "2.4%"), K("PQ check", "pass")]},
   "scopes": {}, "cie": {"gamuts": [{"pts": [[.68, .32], [.265, .69], [.15, .06]], "label": "P3", "color": "#d9d4c9"}, {"pts": [[.64, .33], [.30, .60], [.15, .06]], "label": "Rec.709", "color": "#8a867d"}], "points": [[.44, .40]]}})

# ---------------------------------------------------------------- m06 effects
scen("m06", "effects", "neon_run.kimo", "claude", hms("00:18:22"),
  [["add_effect", ["clip_07", "glitch"], 300], ["set_effect_param", [{"amount": "0.42"}], 100], ["add_effect", ["clip_07", "chromatic"], 200], ["set_effect_mask", ["chromatic"], 200],
   ["add_adjustment_layer", ["V3"], 100], ["set_posterize_time", ["12fps"], 100], ["add_transition", ["whip", "10f"], 200], ["add_speed_lines", ["clip_07"], 700]],
  {"viewer": {"src": F("m06"), "tag": "fx - clip_07", "tc": "01:00:09:14", "res": "1920 x 1080 24p"},
   "insp": {"sub": "clip_07 · night_street", "blocks": [K("Position", "0, 0"), K("Scale", "104%"), K("Rotation", "0.0"), K("Opacity", "100%"), S("Effect stack"),
            {"list": ["glitch", "chromatic", "speed lines"], "sel": 2}, S("speed lines"), K("Density", "64"), K("Length", "0.35"), K("Center", "960, 540"), K("Amount", "0.60", **{"from": "0.20"}), K("Blend", "screen")]},
   "tl": {"ruler": {"t0": 4.21, "t1": 14.14, "step": 2, "first": 6, "minor": 2, "minorFrom": 5, "ticks": False}, "playhead": 9.77, "tracks": [
      {"label": "V2", "h": 36, "lane": "#39352c", "laneInset": 0, "clips": [{"s": 8.0, "e": 11.99, "label": "adjustment", "cls": "mauve"}]},
      {"label": "V1", "h": 80, "cls": "tall", "clips": [{"s": 4.21, "e": 8.68, "label": "alley_rain", "thumb": F("m06_t1")}, {"s": 8.76, "e": 9.71, "cls": "ghost haspill", "pill": "whip 10f"},
                                     {"s": 9.80, "e": 14.14, "label": "night_street", "thumb": F("m06_t2"), "cls": "sel", "fx": ["glitch", "chromatic", "speed lines"]}]},
      {"label": "A1", "h": 35, "area": {"pts": [0.7, 0.45, 0.62, 0.38, 0.55, 0.42, 0.6, 0.35, 0.5, 0.3, 0.48, 0.4, 0.58, 0.45, 0.62], "bg": "#252422", "fill": "#4f4e49"}}]}},
  stats={"cpu": 74, "ffmpeg": "7.1", "saved_s": 1}, final={"count": 57})

# ---------------------------------------------------------------- m07 tracking
import random
random.seed(4)
res = [0.6 + 0.5 * abs(math.sin(i / 5)) + random.random() * .5 + (1.2 if 30 < i < 40 else 0) for i in range(73)]
scen("m07", "tracking", "city_billboard.kimo", "claude", hms("00:44:51"),
  [["undistort_clip", ["clip_05", {"k1": "-0.08"}], 3200], ["stabilize", ["clip_05", {"smooth": "0.5"}], 4100], ["camera_solve", ["clip_05", {"pan": "false"}], 9800],
   ["track_planar", ["clip_05", "screen_quad"], 6300], ["track_mesh", ["clip_05", "flag", "8x6"], 5000], ["validate_planar_track", ["screen_quad"], 2100]],
  {"viewer": {"src": F("m07"), "tag": "track · screen_quad", "left": "solve 0.70 px RMS", "tc": "01:00:02:21", "res": "1920 x 1080 24p"},
   "insp": {"sub": "screen_quad · planar", "blocks": [K("Confidence", "0.85"), K("Lost frames", "0"), K("Corner p95", "1.8 px"), K("Corner max", "2.6 px", **{"from": "3.4"}), K("Anchors", "4"),
            S("Camera solve"), {"widget": "solve", "h": 150, "seed": 9}, {"cap": ["focal 28.4 mm", "96/96 frames"]}]},
   "res": {"ruler": {"t0": 0, "t1": 72, "step": 12, "fmt": "n"}, "playhead": 42, "graph": {"pts": res, "max": 4, "thr": 2, "thrLabel": "2 px"},
           "lanes": [{"label": "screen_quad", "bar": [[0, 72, "#6b665d"]]}, {"label": "flag mesh", "bar": [[0, 72, "#57534c"]]}]}},
  stats={"cpu": 88, "ffmpeg": "7.1", "saved_s": 2}, final={"count": 72})

# ---------------------------------------------------------------- m08 modelling
scen("m08", "modelling", "amphora_study.kimo", "claude", hms("00:34:12"),
  [["generate_mesh_primitive", [], 100], ["mesh_lathe", ["profile", 96], 400], ["mesh_subdivide", ["vase", 2], 900], ["inspect_sdf_graph", ["vase"], 100], ["contour_sdf_graph", ["vase"], 100], ["mesh_boolean", ["vase", "slot"], 600]],
  {"viewer": {"src": F("m08"), "left": "wireframe on shaded", "res": "persp"},
   "stack": {"nodes": [{"name": "lathe"}, {"name": "subdivide"}, {"name": "boolean diff", "em": True}, {"name": "contour_sdf"}]},
   "insp": {"sub": "vase", "blocks": [S("Mesh"), K("Vertices", "24,576"), K("Triangles", "49,152", ch=True), K("Normals", "smooth"), K("Manifold", "yes"), S("Bounds"), K("Height", "1.42 m"), K("Radius", "0.38 m"), S("Export"), K("Format", "gltf")]},
   "prof": {"pts": [[0, .1], [.15, .55], [.3, .8], [.45, .62], [.6, .3], [.75, .35], [.9, .7], [1, .9]], "cursor": .55,
            "thumbs": [{"label": "cylinder", "src": F("m08_t1")}, {"label": "lathe", "src": F("m08_t2")}, {"label": "subdiv", "src": F("m08_t3")}, {"label": "boolean", "src": F("m08_t4"), "sel": True}]}})

# ---------------------------------------------------------------- m09 graphics
scen("m09", "graphics", "drone_hud.kimo", "cursor", hms("00:27:40"),
  [["add_hud_frame", ["V4", "corners"], 200], ["add_detection_box", ["car", "0.94"], 100], ["add_detection_box", ["person"], 100], ["add_counter", ["speed", "0-72"], 200],
   ["add_prompt_box", ["V2"], 100], ["add_path_shape", ["route", "6 pts"], 100], ["add_shape_modifier", ["trim"], 100], ["set_layer_style", ["hud", "stroke"], 400]],
  {"viewer": {"src": F("m09"), "tag": "graphics · V4", "tc": "01:00:06:03", "res": "1920 x 1080 30p"},
   "insp": {"sub": "hud · shape layer", "blocks": [S("Layer style"), K("Stroke", "2 px", **{"from": "1"}), K("Stroke color", "cream"), K("Drop shadow", "off"), K("Inner glow", "off"), K("Opacity", "90%"),
            S("Shape layers"), {"list": ["hud", "det_car", "det_person", "counter_speed", "prompt_1", "route"], "sel": 0}]},
   "tl": {"ruler": {"t0": 1.3, "t1": 10.6, "step": 2}, "playhead": 6.0, "tracks": [
      {"label": "V4", "clips": [{"s": 1.6, "e": 5.4, "label": "hud", "cls": "mauve sel"}, {"s": 5.5, "e": 10.6, "label": "counter_speed", "cls": "mauve"}]},
      {"label": "V3", "clips": [{"s": 1.6, "e": 3.9, "label": "det_car", "thumb": F("m09_t1")}, {"s": 4.0, "e": 6.4, "label": "det_person", "thumb": F("m09_t2")}, {"s": 6.5, "e": 10.6, "cls": "ghost"}]},
      {"label": "V2", "clips": [{"s": 1.6, "e": 5.9, "label": "prompt_1", "cls": "ghost"}, {"s": 6.0, "e": 10.6, "label": "- - - -  route", "cls": "ghost"}]},
      {"label": "V1", "clips": [{"s": 1.6, "e": 10.6, "label": "coast_drone", "thumb": F("m09_t3")}]}]}},
  stats={"cpu": 54, "ffmpeg": "7.1", "saved_s": 1}, final={"count": 48})

# ---------------------------------------------------------------- m10 particles
scen("m10", "particles", "glass_break.kimo", "claude", hms("00:42:51"),
  [["add_emitter", ["dust", 4000], 100], ["sample_advection_field", [], 300], ["add_shard_burst", ["pane", 220], 500], ["shards_project", ["pane"], 200], ["simulate_physics", ["1-96"], 3800]],
  {"viewer": {"src": F("m10"), "tc": "00:00:01:14", "res": "frame 38 / 96"},
   "emit": {"rows": [{"k": "dust", "v": "4,000/s"}, {"k": "sparks", "v": "600/s"}, {"k": "shards", "v": "220"}]},
   "insp": {"sub": "shard_burst", "blocks": [S("Fracture"), K("Shards", "220", chip=True), K("Seed", "7", chip=True), S("Physics"), K("Gravity", "-9.81", chip=True), K("Restitution", "0.30", chip=True), K("Drag", "0.04", chip=True),
            S("Particles"), K("Alive", "18,402", chip=True, emb=True), K("Lifetime", "2.0 s", chip=True)]},
   "sim": {"metaChip": "cache 38 / 96", "ruler": {"t0": 0, "t1": 96, "step": 24, "fmt": "n"}, "playhead": 38,
           "lanes": [{"label": "cache", "cache": [38, 96]}, {"label": "shards", "dots": [6, 14, 22, 30, 38, 52, 60, 66, 74, 82, 90]}, {"label": "dust", "dots": [3 + i * 4.6 for i in range(20)], "r": 2}]}})

# ---------------------------------------------------------------- m11 lighting
scen("m11", "lighting", "perfume_launch.kimo", "codex", hms("00:18:05"),
  [["light_rig", ["rembrandt", 4], 200], ["set_material", ["bottle", "glass"], 100], ["set_material", ["cap", "metal"], 100], ["preview_material", ["cap"], 300], ["set_relight", ["product"], 400], ["render_wgpu_pbr", [48], 1200]],
  {"viewer": {"src": F("m11"), "tag": "relight rig_key"},
   "lights": {"rows": [{"icon": "bulb", "k": "rig_key", "v": "5600K"}, {"icon": "bulb", "k": "rig_fill", "v": "6200K"}, {"icon": "bulb", "k": "rig_rim", "v": "7500K", "dim": True}]},
   "insp": {"sub": "cap", "blocks": [{"widget": "swatches", "h": 58, "style": "border:none;background:none", "items": [
              {"hi": "#ffffff", "mid": "#8d8d8d", "lo": "#1a1a1a"}, {"hi": "#f4efe6", "mid": "#b9b2a6", "lo": "#3a3631", "sel": True}, {"hi": "#bdb8ae", "mid": "#6f6b64", "lo": "#2a2825"}, {"hi": "#5a5a5a", "mid": "#202020", "lo": "#050505"}]},
            S("Material"), K("Albedo", "0.62"), K("Roughness", "0.34", ch=True), K("Metallic", "1.00"), K("IOR", "1.45"), K("Clearcoat", "0.00")]},
   "rig": {"lights": [{"name": "key", "a": 90, "em": True}, {"name": "fill", "a": 215}, {"name": "rim", "a": 325}],
           "rows": [["rig_key", "5600K", "2.4", .8, True, "1.0"], ["rig_fill", "6200K", "0.6", .25, False, "1.0"], ["rig_rim", "7500K", "1.8", .6, False, "0.8"]],
           "thumbs": [{"label": "ratio 2", "src": F("m11_t1")}, {"label": "ratio 4", "src": F("m11_t2"), "sel": True}, {"label": "ratio 8", "src": F("m11_t3")}, {"label": "ratio 16", "src": F("m11_t4")}]}})

# ---------------------------------------------------------------- m12 roto
def dense(a, b, n): return [a + (b - a) * i / (n - 1) for i in range(n)]
scen("m12", "roto", "studio_portrait.kimo", "codex", hms("00:31:05"),
  [["chroma_key", ["clip_02", "green"], 200], ["spill_suppress", ["clip_02", "0.6"], 600], ["add_mask", ["clip_02", "bezier", "14 pts"], 400], ["set_mask_path_key", ["mask_1", {"f": 112}], 100],
   ["roto_propagate", ["mask_1", "112-180"], 200], ["set_track_matte", ["V2", "luma"], 1800], ["refine_matte", ["mask_1", {"feather": 6}], 1100]],
  {"viewer": {"src": F("m12"), "tag": "mask_1 · clip_02", "tc": "01:00:04:16", "res": "1920 x 1080 25p"},
   "insp": {"sub": "mask_1 · bezier", "blocks": [K("Position", "0, 0, 0"), K("Scale", "104%"), K("Rotation", "0.0"), K("Opacity", "100%"), S("Refine matte"), K("Feather", "6.0 px", **{"from": "2.0"}), K("Choke", "-1.5"), K("Edge contrast", "0.40"),
            K("Decontaminate", "on"), K("Bg model", "auto"), S("Key"), K("Screen", "green"), K("Spill", "0.60"), S("Quality"), K("Matte MAE", "6.70")]},
   "keys": {"ruler": {"t0": 100, "t1": 152, "step": 10, "fmt": "n", "align": "left"}, "playhead": 118,
            "lanes": [{"label": "<span class=cream>mask_1</span> <span class=dim>path</span>", "html": True, "keys": dense(101, 151, 26), "size": 4.5},
                      {"label": "<span class=cream>mask_1</span> <span class=dim>feather</span>", "html": True, "keys": [104, 116, 128, 140, 150]},
                      {"label": "<span class=cream>mask_1</span> <span class=dim>opacity</span>", "html": True, "area": [[101, .2], [112, .25], [126, .55], [140, .9], [151, .95]], "color": "#4a4741"},
                      {"label": "<span class=cream>track</span> <span class=dim>matte</span>", "html": True, "area": [[101, .9], [120, .9], [120, .5], [135, .5], [135, .9], [151, .9]], "color": "#8a867d"}]}},
  stats={"cpu": 68, "ffmpeg": "7.1", "saved_s": 3}, final={"count": 63})

# ---------------------------------------------------------------- m13 scene3d
scen("m13", "scene3d", "helix_study.kimo", "cursor", hms("00:00:04"),
  [["scene_add", ["helix_plates"], 2100], ["light_add", ["key", "4200K"], 200], ["light_add", ["rim", "7500K"], 200], ["camera_set", [{"fov": 35}], 100], ["keyframe", ["camera", "pos", "4.0s"], 300]],
  {"viewer": {"src": F("m13"), "left": "00:00:04:00", "res": "1920 x 1080 24p"},
   "scene": {"items": [{"name": "camera_main", "icon": "cam", "sel": True}, {"name": "key_light", "icon": "light"}, {"name": "rim_light", "icon": "light"}, {"name": "fill_light", "icon": "light"}, {"name": "helix_plates", "icon": "mesh"}, {"name": "floor", "icon": "plane"}]},
   "insp": {"sub": "camera_main", "blocks": [S("Transform"), {"html": "<div class=\"row\"><span class=\"k\">Position</span><span class=\"v\"><span class=dim>X</span> 1.20 <span class=dim>Y</span> 0.85 <span class=dim>Z</span> <span class=em>-3.40</span></span></div>"},
            K("Rotation", "0, 18, 0"), K("FOV", "35"), S("Lens"), K("Focus", "3.6 m"), K("Aperture", "f/2.8"), K("Motion blur", "on")]},
   "keys": {"ruler": {"t0": 0, "t1": 6.6, "step": 1, "fmt": "s", "align": "left"}, "playhead": 4.0,
            "strip": [{"em": True, "pts": [[0.05, 0], [0.4, .15], [0.7, .7], [1.0, 1]]}, {"pts": [[3.0, 1], [3.6, .85], [4.3, .2], [5.0, 0]]}],
            "lanes": [{"label": "<span class=cream>camera_main</span> <span class=dim>position</span>", "html": True, "sel": True, "keys": [0, 1, 2, 3, {"t": 4, "em": True}, 5, 6]},
                      {"label": "<span class=cream>camera_main</span> <span class=dim>rotation</span>", "html": True, "keys": [0, 2, 4, 6]},
                      {"label": "<span class=cream>key_light</span> <span class=dim>intensity</span>", "html": True, "keys": [0, 3, 6]}]}},
  activity="animating")
SC[-1]["elapsed"] = hms("00:34:12")

# ---------------------------------------------------------------- m14 export
scen("m14", "export", "dune_film.kimo", "codex", hms("00:16:55"),
  [["trim", ["clip_09", {"out": "01:30:00"}], 100], ["grade", ["clip_04", {"warmth": "+18"}], 800], ["caption", [{"track": "en"}], 1800], ["mix_audio", ["music", "-14 LUFS"], 900],
   ["normalize", ["dialogue"], 400], ["validate_project", [], 300], ["set_export", [{"preset": "web_1080"}], 100], ["export", ["1920x1080", "h264", {"crf": 18}], 38400]],
  {"agent": {"footer": "project saved 2s ago"},
   "viewer": {"src": F("m14"), "progress": {"pct": 64, "left": "frame 1382 / 2160", "right": "42 fps   eta 00:18"}, "thumbs": [{"src": F(f"m14_t{i}"), "sel": i == 5, "dim": i > 5} for i in range(1, 9)]},
   "exp": {"blocks": [K("Format", "MP4"), K("Video", "H.264 high"), K("Resolution", "1920 x 1080"), K("Frame rate", "24"), K("Quality", "CRF 18"), K("Audio", "AAC 320 kbps"), K("Loudness", "-14 LUFS"),
            K("Captions", "en.srt"), K("Output", "~/films/dune_film.mp4"), K("Size so far", "41.2 MB")]},
   "renders": {"head": ["name", "preset", "length", "status", "started"], "foot": "cpu 92%   ffmpeg 7.1   disk 212 GB free",
               "rows": [["dune_film.mp4", "1080p h264", "01:30", "● rendering 64%", "now"], ["dune_film_square.mp4", "1080x1080", "01:30", "queued", "-"],
                        ["dune_teaser.mp4", "1080p h264", "00:15", "✓ done", "4m ago"], ["dune_film_v1.mp4", "1080p h264", "01:28", "✓ done", "32m ago"]]}})

# ---------------------------------------------------------------- m15 grading
scen("m15", "grading", "dune_film.kimo", "codex", hms("00:14:02"),
  [["import_media", ["neon_walk.mov"], 400], ["trim", ["clip_07", {"out": "00:04.20"}], 100], ["analyze_frame", ["clip_07"], 600], ["match_shot", ["clip_07", {"ref": "clip_06"}], 900],
   ["set_white_balance", ["clip_07", "5600K"], 100], ["grade", ["clip_07", {"lift": "-0.02"}], 100], ["grade", ["clip_07", {"gamma": "+0.01"}], 100], ["grade", ["clip_07", {"sat": "1.10"}], 100], ["grade", ["clip_07", {"gain": "+0.06"}], 1200]],
  {"agent": {"footer": "following grade"},
   "viewer": {"src": F("m15"), "tags": [{"text": "source"}, {"text": "kimo grade", "pos": "tr"}], "tc": "01:00:42:19"},
   "grade": {"sub": "clip_07  neon_walk", "blocks": [K("Temperature", "5600K"), K("Tint", "-3"), K("Contrast", "1.14"), K("Pivot", "0.435"), K("Saturation", "1.10"), K("Gain", "+0.06", ch=True), K("Highlights", "-12"), K("Shadows", "+6"),
             S("Nodes"), {"list": ["01  balance", "02  contrast", "03  look"], "sel": 2}, {"widget": "thumbs", "h": 70, "style": "border:none;background:none", "items": [{"src": F("m15_t1")}, {"src": F("m15_t2"), "sel": True, "label": "clip_07"}, {"src": F("m15_t3")}]}]},
   "bottom": {"wheels": [{"name": "Lift", "dot": [-.12, .05], "vals": ["-0.02", "0.00", "+0.03"]}, {"name": "Gamma", "dot": [.05, .02], "vals": ["+0.01", "0.00", "-0.01"], "cols": [None, None, "#6f8fe0"]},
                         {"name": "Gain", "em": True, "dot": [0, 0], "ring": 45, "vals": ["+0.06", "+0.04", "+0.02"]}]}})

for s in SC:
    s["order"] = int(s["id"][1:])
SC.sort(key=lambda s: s["order"])
json.dump(SC, open(os.path.join(HERE, "scenarios.json"), "w"), indent=1)
print(f"wrote {len(SC)} scenarios")
