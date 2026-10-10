use crate::keyframe::Prop;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Source {
    /// Media file decoded through ffmpeg.
    Media { asset: String },
    /// Flat color generator.
    Solid { color: [f64; 3] },
    /// Procedural gradient generator (c1 -> c2 vertical).
    Gradient { c1: [f64; 3], c2: [f64; 3] },
    /// Rendered text (TTF/OTF via ab_glyph). Animate with the `reveal` prop (typewriter) plus transform props.
    Text { text: String, font: String, size: f64, color: [f64; 3], letter_spacing: f64, stroke_width: f64, stroke_color: [f64; 3], shadow: f64, align: String },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Mask {
    /// rect | ellipse | linear (left-to-right gradient edge) ; coordinates normalized 0..1 of canvas.
    pub shape: String,
    pub props: BTreeMap<String, Prop>, // cx, cy, w, h, feather, rotation
    pub invert: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Effect {
    pub id: String,
    pub kind: String,
    pub params: BTreeMap<String, Prop>,
    #[serde(default)]
    pub strings: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Clip {
    pub id: String,
    pub source: Source,
    /// Position on the timeline, seconds.
    pub start: f64,
    pub duration: f64,
    /// Offset into the source media, seconds.
    pub in_point: f64,
    pub speed: f64,
    pub props: BTreeMap<String, Prop>, // opacity, x, y, scale, rotation
    pub effects: Vec<Effect>,
    pub blend: String,
    #[serde(default)]
    pub mask: Option<Mask>,
    /// Transition INTO this clip from the previous clip on the track.
    pub transition_in: Option<Transition>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Transition { pub kind: String, pub duration: f64 }

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Track { pub id: String, pub clips: Vec<Clip> }

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Asset { pub id: String, pub path: String, pub width: u32, pub height: u32, pub duration: f64, pub fps: f64 }

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Project {
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub fps: f64,
    pub assets: BTreeMap<String, Asset>,
    /// Bottom to top.
    pub tracks: Vec<Track>,
    #[serde(skip)]
    pub next_id: u64,
}

impl Default for Project {
    fn default() -> Self {
        Project { name: "untitled".into(), width: 1280, height: 720, fps: 30.0, assets: BTreeMap::new(), tracks: vec![], next_id: 1 }
    }
}

impl Project {
    pub fn gen_id(&mut self, p: &str) -> String { let i = self.next_id; self.next_id += 1; format!("{p}{i}") }
    pub fn duration(&self) -> f64 {
        self.tracks.iter().flat_map(|t| t.clips.iter()).map(|c| c.start + c.duration).fold(0.0, f64::max)
    }
    pub fn find_clip_mut(&mut self, id: &str) -> Option<&mut Clip> {
        self.tracks.iter_mut().flat_map(|t| t.clips.iter_mut()).find(|c| c.id == id)
    }
    pub fn find_clip(&self, id: &str) -> Option<&Clip> {
        self.tracks.iter().flat_map(|t| t.clips.iter()).find(|c| c.id == id)
    }
}

pub fn default_props() -> BTreeMap<String, Prop> {
    let mut m = BTreeMap::new();
    m.insert("opacity".into(), Prop::new(1.0));
    m.insert("x".into(), Prop::new(0.0));      // pixels offset
    m.insert("y".into(), Prop::new(0.0));
    m.insert("scale".into(), Prop::new(1.0));
    m.insert("rotation".into(), Prop::new(0.0)); // degrees
    m.insert("reveal".into(), Prop::new(1.0)); // text typewriter 0..1
    m
}
