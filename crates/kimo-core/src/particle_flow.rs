//! Field-driven particle flow: deterministic emitter with birth schedule and
//! lifetime recycling through a preallocated ring buffer (the pool is allocated
//! once; advancing and rendering perform no per-frame heap allocation),
//! smoothstep fade-in/out, cubic-eased size, multi-stop gradient color,
//! advection coupling to particle_advect fields, and a camera-v2 depth-splat
//! renderer with a distance-attenuated footprint, radial gaussian kernel of
//! configurable hardness, additive or premultiplied-alpha blending, a tiled
//! inner loop, and z-buffer occlusion against mesh geometry depth.
//! Original code. Correctness-first CPU path: no motion blur, trails, or fluid
//! coupling claim.
use serde::{Deserialize, Serialize};
use crate::camera_projection::Camera;
use crate::effects::Frame;
use crate::particle_advect::Advect;

pub const MAX_PARTICLES: u32 = 100_000;

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Blend {
    Additive,
    PremultipliedAlpha,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ColorStop {
    /// Normalized lifespan offset in [0,1]; stops must be ascending, first 0, last 1.
    pub offset: f64,
    pub rgb: [f64; 3],
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Emitter {
    pub seed: u64,
    /// Ring capacity: maximum simultaneously simulated particles (total births are unbounded).
    pub count: u32,
    pub origin: [f64; 3],
    pub spread: [f64; 3],
    pub base_velocity: [f64; 3],
    pub velocity_jitter: f64,
    pub birth_rate: f64,
    pub lifetime: [f64; 2],
    pub color_stops: Vec<ColorStop>,
    pub alpha_start: f64,
    pub alpha_end: f64,
    /// Lifespan fractions for the smoothstep fade-in / fade-out envelope, each in (0, 0.5].
    pub fade_in: f64,
    pub fade_out: f64,
    /// World-space splat diameter at birth/death; screen footprint attenuates with distance.
    pub size_start: f64,
    pub size_end: f64,
    pub blend: Blend,
    /// Gaussian kernel hardness > 0; larger = tighter core.
    pub hardness: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Flow {
    pub emitter: Emitter,
    pub field: Advect,
    pub gravity: [f64; 3],
    pub drag: f64,
    /// 1/s relaxation of particle velocity toward the field velocity; 0 disables.
    pub coupling: f64,
    pub dt: f64,
}

/// One ring-pool particle slot (public for read-only inspection; mutate via Simulation).
#[derive(Clone, Copy, Debug)]
pub struct Slot {
    pub position: [f64; 3],
    pub velocity: [f64; 3],
    pub birth: f64,
    pub lifetime: f64,
    pub alive: bool,
}

fn smoothstep(e0: f64, e1: f64, x: f64) -> f64 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}
fn ease_in_out_cubic(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    if t < 0.5 { 4.0 * t * t * t } else { 1.0 - (-2.0 * t + 2.0).powi(3) / 2.0 }
}
fn jitter(seed: u64, index: u64, lane: u8) -> f64 {
    let mut h = 0xcbf29ce484222325u64;
    for b in seed.to_le_bytes().into_iter().chain(index.to_le_bytes()).chain([lane]) {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x100000001b3);
    }
    h ^= h.rotate_left(21);
    h ^= h >> 31;
    ((h >> 11) as f64 + 0.5) / 9007199254740992.0
}

impl Emitter {
    pub fn validate(&self) -> Result<(), String> {
        if self.count == 0 || self.count > MAX_PARTICLES {
            return Err("emitter count 1..100000".into());
        }
        if !self.origin.iter().chain(&self.spread).chain(&self.base_velocity).all(|v| v.is_finite() && v.abs() <= 1e9) {
            return Err("finite bounded origin/spread/base_velocity required".into());
        }
        if !self.velocity_jitter.is_finite() || !(0.0..=1e6).contains(&self.velocity_jitter) {
            return Err("velocity_jitter 0..1e6".into());
        }
        if !self.birth_rate.is_finite() || self.birth_rate <= 0.0 || self.birth_rate > 1e6 {
            return Err("birth_rate (0, 1e6]".into());
        }
        if !self.lifetime.iter().all(|v| v.is_finite()) || self.lifetime[0] <= 0.0 || self.lifetime[1] < self.lifetime[0] || self.lifetime[1] > 1e4 {
            return Err("lifetime 0<min<=max<=1e4".into());
        }
        if self.color_stops.len() < 2 || self.color_stops.len() > 8 {
            return Err("color_stops 2..=8".into());
        }
        let first = self.color_stops.first().unwrap();
        let last = self.color_stops.last().unwrap();
        if first.offset != 0.0 || last.offset != 1.0 || !self.color_stops.windows(2).all(|w| w[1].offset > w[0].offset) {
            return Err("color stop offsets strictly ascending, first 0, last 1".into());
        }
        if !self.color_stops.iter().all(|s| s.rgb.iter().all(|v| v.is_finite() && (0.0..=1.0).contains(v))) {
            return Err("color stop rgb 0..1".into());
        }
        if !(0.0..=1.0).contains(&self.alpha_start) || !(0.0..=1.0).contains(&self.alpha_end) {
            return Err("alpha 0..1".into());
        }
        if !(0.0..=0.5).contains(&self.fade_in) || !(0.0..=0.5).contains(&self.fade_out) || self.fade_in <= 0.0 || self.fade_out <= 0.0 {
            return Err("fade_in/fade_out (0, 0.5]".into());
        }
        if !self.size_start.is_finite() || !self.size_end.is_finite() || self.size_start <= 0.0 || self.size_end <= 0.0 || self.size_start.max(self.size_end) > 1e6 {
            return Err("sizes (0, 1e6] world units".into());
        }
        if !self.hardness.is_finite() || self.hardness <= 0.0 || self.hardness > 1e4 {
            return Err("hardness (0, 1e4]".into());
        }
        Ok(())
    }
    pub fn birth_time(&self, spawn_index: u64) -> f64 {
        spawn_index as f64 / self.birth_rate
    }
    fn lifetime_of(&self, spawn_index: u64) -> f64 {
        self.lifetime[0] + (self.lifetime[1] - self.lifetime[0]) * jitter(self.seed, spawn_index, 10)
    }
    fn spawn_state(&self, spawn_index: u64) -> ([f64; 3], [f64; 3]) {
        let p = std::array::from_fn(|j| self.origin[j] + (jitter(self.seed, spawn_index, j as u8) - 0.5) * 2.0 * self.spread[j]);
        let v = std::array::from_fn(|j| self.base_velocity[j] + (jitter(self.seed, spawn_index, 3 + j as u8) - 0.5) * 2.0 * self.velocity_jitter);
        (p, v)
    }
    /// Multi-stop gradient color at normalized age t in [0,1].
    pub fn color_at(&self, t: f64) -> [f64; 3] {
        let t = t.clamp(0.0, 1.0);
        if t <= 0.0 {
            return self.color_stops[0].rgb;
        }
        if t >= 1.0 {
            return self.color_stops[self.color_stops.len() - 1].rgb;
        }
        for w in self.color_stops.windows(2) {
            if t <= w[1].offset {
                let u = (t - w[0].offset) / (w[1].offset - w[0].offset);
                return std::array::from_fn(|j| w[0].rgb[j] + (w[1].rgb[j] - w[0].rgb[j]) * u);
            }
        }
        self.color_stops.last().unwrap().rgb
    }
    /// Alpha at normalized age: linear start/end ramp under a smoothstep fade-in/out envelope.
    pub fn alpha_at(&self, t: f64) -> f64 {
        let t = t.clamp(0.0, 1.0);
        let env = smoothstep(0.0, self.fade_in, t) * (1.0 - smoothstep(1.0 - self.fade_out, 1.0, t));
        (self.alpha_start + (self.alpha_end - self.alpha_start) * t) * env
    }
    /// World-space size at normalized age: cubic-eased bump envelope - spawns at
    /// size_start, expands to size_end by GROW, holds, then shrinks back to
    /// size_start after SHRINK so particles are small again before death.
    pub fn size_at(&self, t: f64) -> f64 {
        const GROW: f64 = 0.3;
        const SHRINK: f64 = 0.7;
        let t = t.clamp(0.0, 1.0);
        let env = if t < GROW {
            ease_in_out_cubic(t / GROW)
        } else if t <= SHRINK {
            1.0
        } else {
            1.0 - ease_in_out_cubic((t - SHRINK) / (1.0 - SHRINK))
        };
        self.size_start + (self.size_end - self.size_start) * env
    }
    /// Radial gaussian kernel weight, 1 at center and 0 at d=1; hardness tightens the core.
    pub fn kernel(&self, d: f64) -> f64 {
        if d >= 1.0 {
            return 0.0;
        }
        let e = (-self.hardness).exp();
        (((-self.hardness * d * d).exp() - e) / (1.0 - e)).max(0.0)
    }
}

/// Fixed-step simulation over a preallocated ring pool. `advance` allocates nothing.
pub struct Simulation {
    flow: Flow,
    pool: Vec<Slot>,
    cursor: usize,
    time: f64,
    spawned: u64,
    total_born: u64,
    total_died: u64,
    /// Reusable render scratch (projected splats, radii, flags, tile buckets), cleared per render.
    scratch: Vec<(f64, f64, f64, f64)>,
    radii: Vec<f64>,
    flags: Vec<u8>,
    buckets: Vec<Vec<usize>>,
}

impl Simulation {
    pub fn new(flow: Flow) -> Result<Self, String> {
        flow.emitter.validate()?;
        if !flow.gravity.iter().all(|v| v.is_finite() && v.abs() <= 1e9) {
            return Err("finite bounded gravity required".into());
        }
        if !flow.drag.is_finite() || flow.drag < 0.0 || !flow.coupling.is_finite() || flow.coupling < 0.0 || flow.coupling > 1e6 {
            return Err("drag/coupling nonnegative finite (coupling<=1e6)".into());
        }
        if !flow.dt.is_finite() || flow.dt <= 0.0 || flow.dt > 1.0 {
            return Err("dt (0, 1]".into());
        }
        crate::guard::reserve(flow.emitter.count as usize * 64);
        let slot = Slot { position: [0.0; 3], velocity: [0.0; 3], birth: 0.0, lifetime: 1.0, alive: false };
        Ok(Self { pool: vec![slot; flow.emitter.count as usize], cursor: 0, time: 0.0, spawned: 0, total_born: 0, total_died: 0, scratch: Vec::new(), radii: Vec::new(), flags: Vec::new(), buckets: Vec::new(), flow })
    }
    pub fn time(&self) -> f64 {
        self.time
    }
    pub fn pool(&self) -> &[Slot] {
        &self.pool
    }
    pub fn alive_count(&self) -> usize {
        self.pool.iter().filter(|s| s.alive).count()
    }
    /// Advance to `to_time` with exact fixed dt (last partial step). No heap allocation.
    pub fn advance(&mut self, to_time: f64) -> Result<usize, String> {
        if !to_time.is_finite() || to_time < self.time || to_time > 1e4 {
            return Err("advance requires current<=to_time<=1e4".into());
        }
        let mut steps = 0usize;
        while self.time < to_time {
            crate::guard::check();
            let h = flow_dt(self.flow.dt, to_time - self.time);
            let target = self.time + h;
            // Recycle: kill expired, then spawn all births due by target into ring slots.
            for s in self.pool.iter_mut() {
                if s.alive && target - s.birth >= s.lifetime {
                    s.alive = false;
                    self.total_died += 1;
                }
            }
            while self.flow.emitter.birth_time(self.spawned) < target {
                let idx = self.spawned;
                let birth = self.flow.emitter.birth_time(idx);
                let (p, v) = self.flow.emitter.spawn_state(idx);
                let slot = &mut self.pool[self.cursor];
                if slot.alive {
                    self.total_died += 1; // ring overwrite of the oldest live slot
                }
                *slot = Slot { position: p, velocity: v, birth, lifetime: self.flow.emitter.lifetime_of(idx), alive: true };
                self.cursor = (self.cursor + 1) % self.pool.len();
                self.spawned += 1;
                self.total_born += 1;
            }
            for s in self.pool.iter_mut() {
                if !s.alive {
                    continue;
                }
                let field = self.flow.field.velocity(s.position)?;
                for j in 0..3 {
                    let a = self.flow.gravity[j] + self.flow.coupling * (field[j] - s.velocity[j]) - self.flow.drag * s.velocity[j];
                    s.velocity[j] += a * h;
                    s.position[j] += s.velocity[j] * h;
                }
                if s.position.iter().chain(s.velocity.iter()).any(|v| !v.is_finite() || v.abs() > 1e12) {
                    return Err("particle state overflow".into());
                }
            }
            self.time = target;
            steps += 1;
        }
        Ok(steps)
    }
}

fn flow_dt(dt: f64, remaining: f64) -> f64 {
    if remaining < dt { remaining } else { dt }
}

#[derive(Debug, Serialize)]
pub struct RenderOut {
    pub frame_pixels: usize,
    pub active: usize,
    pub drawn: usize,
    pub occluded_by_mesh: usize,
    pub behind_camera: usize,
}

/// Depth-splat render at the simulation's current time. Optional mesh supplies the
/// background frame and the occluding z-buffer from the camera-v2 perspective path.
pub fn render(sim: &mut Simulation, camera: &Camera, w: usize, h: usize, mesh: Option<&crate::threed::primitives::Geometry>, depth_range: [f64; 2]) -> Result<(Frame, RenderOut), String> {
    camera.validate()?;
    if w == 0 || h == 0 || w > 1920 || h > 1080 {
        return Err("target 1..1920x1080".into());
    }
    let (mut frame, depth) = if let Some(g) = mesh {
        let o = crate::threed::perspective::render(g, camera, w, h, crate::threed::perspective::Shading::Normal, depth_range)?;
        (o.frame, o.depth)
    } else {
        (Frame::try_new(w, h)?, vec![f64::INFINITY; w * h])
    };
    let mut out = RenderOut { frame_pixels: w * h, active: 0, drawn: 0, occluded_by_mesh: 0, behind_camera: 0 };
    let e = &sim.flow.emitter;
    let focal_px = h as f64 * (camera.focal_length / camera.sensor_height);
    // Project into reusable scratch: (px, py, z, age_t, index).
    sim.scratch.clear();
    for s in &sim.pool {
        if !s.alive {
            continue;
        }
        out.active += 1;
        match camera.project(s.position, w as u32, h as u32)? {
            Some(o) => sim.scratch.push((o[0], o[1], o[2], (sim.time - s.birth) / s.lifetime)),
            None => out.behind_camera += 1,
        }
    }
    // Back-to-front for premultiplied alpha-over; additive is order-independent.
    sim.scratch.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal).then_with(|| a.0.total_cmp(&b.0)).then_with(|| a.1.total_cmp(&b.1)));
    // Tiled inner loop: bucket splat indices into 64px tiles by bounding box.
    const TILE: usize = 64;
    let tw = w.div_ceil(TILE);
    let th = h.div_ceil(TILE);
    if sim.buckets.len() != tw * th {
        sim.buckets.resize_with(tw * th, Vec::new);
    }
    for b in sim.buckets.iter_mut() {
        b.clear();
    }
    sim.radii.clear();
    for (i, &(px, py, z, t)) in sim.scratch.iter().enumerate() {
        let world = e.size_at(t);
        let r = (0.5 * world * focal_px / z).clamp(0.5, 64.0);
        sim.radii.push(r);
        let (x0, x1) = (((px - r).floor().max(0.0) as usize).min(w - 1), ((px + r).ceil() as usize).min(w - 1));
        let (y0, y1) = (((py - r).floor().max(0.0) as usize).min(h - 1), ((py + r).ceil() as usize).min(h - 1));
        for ty in y0 / TILE..=y1 / TILE {
            for tx in x0 / TILE..=x1 / TILE {
                sim.buckets[ty * tw + tx].push(i);
            }
        }
    }
    sim.flags.clear();
    sim.flags.resize(sim.scratch.len() * 2, 0); // [drawn..., occluded...]
    for ty in 0..th {
        for tx in 0..tw {
            crate::guard::check();
            let ys = ty * TILE..((ty + 1) * TILE).min(h);
            let xs = tx * TILE..((tx + 1) * TILE).min(w);
            for &i in &sim.buckets[ty * tw + tx] {
                let (px, py, z, t) = sim.scratch[i];
                let r = sim.radii[i];
                let color = e.color_at(t);
                let alpha = e.alpha_at(t);
                for y in ys.clone() {
                    for x in xs.clone() {
                        let d = ((x as f64 + 0.5 - px).hypot(y as f64 + 0.5 - py)) / r;
                        if d >= 1.0 {
                            continue;
                        }
                        let id = y * w + x;
                        // Relative depth bias: near-ties resolve to the particle (deterministic, no z-fighting).
                        if z >= depth[id] * (1.0 + 1e-6) + 1e-9 {
                            sim.flags[sim.scratch.len() + i] = 1;
                            continue;
                        }
                        let k = e.kernel(d) * alpha;
                        if k <= 0.0 {
                            continue;
                        }
                        match e.blend {
                            Blend::Additive => {
                                for j in 0..3 {
                                    frame.d[id * 4 + j] = (frame.d[id * 4 + j] + (color[j] * k) as f32).min(1.0);
                                }
                            }
                            Blend::PremultipliedAlpha => {
                                for j in 0..3 {
                                    frame.d[id * 4 + j] = frame.d[id * 4 + j] * (1.0 - k as f32) + (color[j] * k) as f32;
                                }
                            }
                        }
                        frame.d[id * 4 + 3] = 1.0;
                        sim.flags[i] = 1;
                    }
                }
            }
        }
    }
    let n = sim.scratch.len();
    out.drawn = sim.flags[..n].iter().filter(|v| **v == 1).count();
    out.occluded_by_mesh = (0..n).filter(|&i| sim.flags[n + i] == 1 && sim.flags[i] == 0).count();
    Ok((frame, out))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::threed::primitives::{generate, Primitive};

    fn flow() -> Flow {
        Flow {
            emitter: Emitter {
                seed: 11,
                count: 64,
                origin: [0.0; 3],
                spread: [0.2, 0.1, 0.2],
                base_velocity: [0.0, 1.5, 0.0],
                velocity_jitter: 0.3,
                birth_rate: 40.0,
                lifetime: [1.0, 2.0],
                color_stops: vec![
                    ColorStop { offset: 0.0, rgb: [1.0, 0.8, 0.2] },
                    ColorStop { offset: 0.5, rgb: [1.0, 0.3, 0.05] },
                    ColorStop { offset: 1.0, rgb: [0.2, 0.1, 0.6] },
                ],
                alpha_start: 0.9,
                alpha_end: 0.6,
                fade_in: 0.1,
                fade_out: 0.15,
                size_start: 0.02,
                size_end: 0.08,
                blend: Blend::Additive,
                hardness: 4.0,
            },
            field: Advect::Curl3 { noise: "perlin".into(), seed: 3, frequency: 0.8, amplitude: 1.0, octaves: 2 },
            gravity: [0.0, -0.5, 0.0],
            drag: 0.1,
            coupling: 2.0,
            dt: 0.01,
        }
    }
    #[test]
    fn ring_recycling_and_birth_schedule() {
        let mut sim = Simulation::new(flow()).unwrap();
        sim.advance(1.0).unwrap();
        assert_eq!(sim.spawned, 40); // 40/sec; birth strictly before target
        assert!(sim.alive_count() <= 64);
        // Long run: births exceed capacity, ring recycles, alive never exceeds capacity.
        sim.advance(1.0 + 64.0 / 40.0 + 3.0).unwrap();
        assert!(sim.spawned > 64);
        assert_eq!(sim.total_born, sim.spawned);
        assert!(sim.alive_count() <= 64);
        assert!(sim.total_died > 0);
        // Conservation: every spawned particle is either alive or counted dead.
        let mut sim2 = Simulation::new(Flow { emitter: Emitter { birth_rate: 10.0, ..flow().emitter }, ..flow() }).unwrap();
        sim2.advance(9.0).unwrap();
        assert_eq!(sim2.total_died + sim2.alive_count() as u64, sim2.spawned);
    }
    #[test]
    fn lifecycle_curves_endpoints() {
        let e = flow().emitter;
        // Smoothstep envelope: zero alpha at birth and death.
        assert_eq!(e.alpha_at(0.0), 0.0);
        assert_eq!(e.alpha_at(1.0), 0.0);
        let peak = e.alpha_at(0.5);
        assert!(peak > 0.0 && peak <= e.alpha_start.max(e.alpha_end));
        // Cubic bump: small at birth and death, peak mid-life.
        assert_eq!(e.size_at(0.0), e.size_start);
        assert_eq!(e.size_at(1.0), e.size_start, "shrinks back before death");
        assert!((e.size_at(0.5) - e.size_end).abs() < 1e-12, "peak at mid-life");
        assert!(e.size_at(0.9) < e.size_end && e.size_at(0.9) > e.size_start);
        // Multi-stop gradient: exact stops, midpoint blends.
        assert_eq!(e.color_at(0.0), [1.0, 0.8, 0.2]);
        assert_eq!(e.color_at(1.0), [0.2, 0.1, 0.6]);
        let c = e.color_at(0.25);
        assert!((c[0] - 1.0).abs() < 1e-12 && (c[1] - 0.55).abs() < 1e-12, "halfway between stop0/stop1");
        // Kernel: 1 at center, 0 at rim, monotonic.
        assert!((e.kernel(0.0) - 1.0).abs() < 1e-12);
        assert_eq!(e.kernel(1.0), 0.0);
        assert!(e.kernel(0.2) > e.kernel(0.6));
    }
    #[test]
    fn deterministic_repeat_and_no_alloc_advance() {
        let mut a = Simulation::new(flow()).unwrap();
        let mut b = Simulation::new(flow()).unwrap();
        a.advance(0.75).unwrap();
        b.advance(0.75).unwrap();
        for (x, y) in a.pool().iter().zip(b.pool()) {
            assert_eq!(x.position, y.position);
            assert_eq!(x.velocity, y.velocity);
            assert_eq!(x.alive, y.alive);
        }
        let cap = a.pool().len();
        let ptr = a.pool().as_ptr();
        a.advance(5.0).unwrap();
        assert_eq!(a.pool().len(), cap);
        assert_eq!(a.pool().as_ptr(), ptr, "pool never reallocates across advance");
    }
    #[test]
    fn depth_occlusion_against_mesh() {
        let mut f = flow();
        f.emitter.count = 8;
        f.emitter.origin = [0.0, 0.0, 2.0];
        f.emitter.spread = [0.1; 3];
        let camera = Camera { eye: [0.0, 0.0, -4.0], pan: 0.0, tilt: 0.0, roll: 0.0, sensor_height: 24.0, focal_length: 35.0, near: 0.1 };
        let cube = generate(&Primitive::Cube { size: [6.0, 6.0, 0.5], subdivisions: 1 }).unwrap();
        let mut sim = Simulation::new(f.clone()).unwrap();
        sim.advance(0.05).unwrap();
        let (_, out) = render(&mut sim, &camera, 64, 36, Some(&cube), [0.5, 10.0]).unwrap();
        assert!(out.active > 0);
        assert_eq!(out.drawn, 0, "all particles behind the cube");
        assert!(out.occluded_by_mesh > 0);
        let (_, out2) = render(&mut sim, &camera, 64, 36, None, [0.5, 10.0]).unwrap();
        assert!(out2.drawn > 0);
    }
    #[test]
    fn validation_rejections() {
        let mut f = flow();
        f.emitter.count = 0;
        assert!(Simulation::new(f).is_err());
        let mut f = flow();
        f.dt = 0.0;
        assert!(Simulation::new(f).is_err());
        let mut f = flow();
        f.emitter.color_stops = vec![ColorStop { offset: 0.2, rgb: [0.0; 3] }, ColorStop { offset: 1.0, rgb: [1.0; 3] }];
        assert!(Simulation::new(f).is_err());
        let mut f = flow();
        f.emitter.fade_in = 0.0;
        assert!(Simulation::new(f).is_err());
        let mut sim = Simulation::new(flow()).unwrap();
        assert!(sim.advance(-1.0).is_err());
    }
}
