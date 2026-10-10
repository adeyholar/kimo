use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Ease { Linear, In, Out, InOut, Hold }

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Key { pub t: f64, pub v: f64, #[serde(default = "d_ease")] pub ease: Ease }
fn d_ease() -> Ease { Ease::Linear }

/// A property that is either constant or keyframed over clip-local time (seconds).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Prop { pub base: f64, #[serde(default)] pub keys: Vec<Key> }

impl Prop {
    pub fn new(v: f64) -> Self { Prop { base: v, keys: vec![] } }
    pub fn at(&self, t: f64) -> f64 {
        if self.keys.is_empty() { return self.base; }
        let k = &self.keys;
        if t <= k[0].t { return k[0].v; }
        if t >= k[k.len() - 1].t { return k[k.len() - 1].v; }
        for w in k.windows(2) {
            if t >= w[0].t && t <= w[1].t {
                if w[0].ease == Ease::Hold { return w[0].v; }
                let u = (t - w[0].t) / (w[1].t - w[0].t).max(1e-9);
                let u = match w[0].ease {
                    Ease::Linear | Ease::Hold => u,
                    Ease::In => u * u * u,
                    Ease::Out => 1.0 - (1.0 - u).powi(3),
                    Ease::InOut => if u < 0.5 { 4.0 * u * u * u } else { 1.0 - (-2.0 * u + 2.0).powi(3) / 2.0 },
                };
                return w[0].v + (w[1].v - w[0].v) * u;
            }
        }
        self.base
    }
    pub fn set_key(&mut self, t: f64, v: f64, ease: Ease) {
        self.keys.retain(|k| (k.t - t).abs() > 1e-6);
        self.keys.push(Key { t, v, ease });
        self.keys.sort_by(|a, b| a.t.partial_cmp(&b.t).unwrap());
    }
}
