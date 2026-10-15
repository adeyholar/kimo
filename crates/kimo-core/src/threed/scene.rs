//! Scene descriptor + compile-to-draws (scene composition CP1). Original code, zero new deps.
//! A Scene binds named geometry sources (SDF contour, primitive, raw stream, glTF scene) to
//! nodes carrying an explicit row-major affine world matrix and a material index, plus the
//! pass-level context (lights, camera-v2). compile() resolves each referenced source once,
//! then emits one Draw per (node, source instance): stream coordinates are SOURCE-local,
//! draw.world applies on top (matches the wgpu multi-draw contract; CPU executors bake via
//! node_ingest::transform). glTF scene sources arrive with their internal node worlds already
//! baked by instantiate(); the descriptor node world composes on top.
//! SDF contour sources are memoized by content hash (fnv1a-64 of the canonical JSON of
//! graph+config) in a caller-supplied Cache, so repeated identical sources and repeated
//! compiles with the same cache pay for one polygonization. The MCP tool uses a fresh cache
//! per call (stateless); executors may carry a Cache across frames.
//! Empty is a result: a scene with no nodes compiles to zero draws without error.
//! Keyframed animation is deliberately NOT here (deferred to the Scene3D bridge, CP4).
//! Nodes reference sources by flat index only - node->node edges do not exist in the
//! descriptor, so cyclic graphs are inexpressible by construction (the timeline bridge
//! owns hierarchy). The contour cache is BOUNDED: at most CONTOUR_CACHE_CAP entries with
//! strict FIFO eviction (a cache hit does NOT refresh position; updating an existing key
//! keeps its slot). The MCP tool builds a fresh cache per call.
use serde::{Serialize,Deserialize};
use super::{mesh::Matrix,primitives::{generate,Primitive},vertex_stream::Stream,sdf_graph::Graph,contour_wrapped,lighting_stage,lighting_stage::Light,texture_material::Surface};
use crate::camera_projection::Camera;

#[derive(Clone,Debug,Serialize,Deserialize)]#[serde(tag="kind",rename_all="snake_case",deny_unknown_fields)]
pub enum Source{Primitive{primitive:Primitive},SdfContour{graph:Graph,config:contour_wrapped::Config},Stream{stream:Stream},GltfScene{bytes:Vec<u8>,buffers:Vec<Vec<u8>>,derive_tangents:bool,scene_index:usize}}

#[derive(Clone,Debug,Serialize,Deserialize)]#[serde(deny_unknown_fields)]
pub struct Node{pub source:usize,pub world:Matrix,pub material:usize}

#[derive(Clone,Debug,Serialize,Deserialize)]#[serde(deny_unknown_fields)]
pub struct Scene{pub sources:Vec<Source>,pub nodes:Vec<Node>,pub materials:Vec<Surface>,pub lights:Vec<Light>,pub camera:Camera}

/// One compiled draw call: source-local stream + world transform + surface. Same shape as
/// the wgpu multi-draw contract; CP3 converts field-by-field.
#[derive(Clone,Debug,Serialize,Deserialize)]#[serde(deny_unknown_fields)]
pub struct Draw{pub stream:Stream,pub world:Matrix,pub surface:Surface}

#[derive(Clone,Debug,Serialize)]pub struct Stats{pub sources:usize,pub sources_resolved:usize,pub nodes:usize,pub draws:usize,pub vertices:usize,pub triangles:usize,pub contour_polygonized:usize,pub contour_cache_hits:usize}

#[derive(Clone,Debug,Serialize)]pub struct Compiled{pub draws:Vec<Draw>,pub lights:Vec<Light>,pub camera:Camera,pub stats:Stats}

fn fnv1a64(bytes:&[u8])->u64{let mut h=0xcbf29ce484222325u64;for b in bytes{h^=*b as u64;h=h.wrapping_mul(0x100000001b3);}h}

/// Max memoized SDF contours. Conservative: 64 x worst-case res-64 contour (~24MB stream)
/// is already ~1.5GB; realistic scenes are far smaller. Strict FIFO, no hit-refresh.
pub const CONTOUR_CACHE_CAP:usize=64;
/// Content-hash memo for SDF contour sources, bounded FIFO. Reusable across compiles
/// (executors); the MCP tool constructs a fresh one per call.
#[derive(Default)]pub struct Cache{contours:std::collections::HashMap<u64,Stream>,order:std::collections::VecDeque<u64>}
impl Cache{pub fn new()->Self{Self::default()} pub fn contour_entries(&self)->usize{self.contours.len()}
 fn contour_get(&self,key:u64)->Option<&Stream>{self.contours.get(&key)}
 fn contour_insert(&mut self,key:u64,stream:Stream){if self.contours.contains_key(&key){self.contours.insert(key,stream);return;}while self.order.len()>=CONTOUR_CACHE_CAP{if let Some(old)=self.order.pop_front(){self.contours.remove(&old);}}self.order.push_back(key);self.contours.insert(key,stream);}}

fn check_affine(m:&Matrix,what:&str)->Result<(),String>{if m[3]!=[0.,0.,0.,1.]||m.iter().flatten().any(|v|!v.is_finite()){return Err(format!("{what}: finite affine transform required (last row 0,0,0,1)"));}Ok(())}

/// Structural validation only - no geometry is built. Unreferenced sources are
/// shape-checked (graph validate / stream validate), not compiled.
pub fn validate(s:&Scene)->Result<(),String>{
 if s.sources.len()>64||s.nodes.len()>256||s.materials.len()>64{return Err("max64 sources/materials, max256 nodes".into());}
 lighting_stage::validate(&s.lights)?;s.camera.validate()?;
 for(i,src)in s.sources.iter().enumerate(){match src{
  Source::Primitive{..}|Source::GltfScene{..}=>{}
  Source::Stream{stream}=>stream.validate().map_err(|e|format!("source {i}: {e}"))?,
  Source::SdfContour{graph,..}=>graph.validate().map_err(|e|format!("source {i}: {e}"))?,}}
 for(i,n)in s.nodes.iter().enumerate(){
  if n.source>=s.sources.len(){return Err(format!("node {i} references source {} of {}",n.source,s.sources.len()));}
  if n.material>=s.materials.len(){return Err(format!("node {i} references material {} of {}",n.material,s.materials.len()));}
  check_affine(&n.world,&format!("node {i}"))?;}
 Ok(())}

/// Resolve referenced sources and emit draws. Sources are resolved lazily (only when a
/// node references them) and memoized per index within the call.
pub fn compile(s:&Scene,cache:&mut Cache)->Result<Compiled,String>{
 validate(s)?;
 let mut resolved:Vec<Option<Vec<Stream>>>=(0..s.sources.len()).map(|_|None).collect();
 let mut polygonized=0usize;let mut hits=0usize;
 for n in &s.nodes{
  if resolved[n.source].is_none(){
   let streams=match &s.sources[n.source]{
    Source::Primitive{primitive}=>{let g=generate(primitive)?;vec![Stream{vertices:g.vertices,indices:g.indices}]}
    Source::Stream{stream}=>vec![stream.clone()],
    Source::SdfContour{graph,config}=>{
     let key=fnv1a64(&serde_json::to_vec(&(graph,config)).map_err(|e|e.to_string())?);
     if let Some(cached)=cache.contour_get(key){hits+=1;vec![cached.clone()]}
     else{
      #[cfg(feature="surface-contour")]
      {let m=contour_wrapped::polygonize(graph,config)?;polygonized+=1;cache.contour_insert(key,m.stream.clone());vec![m.stream]}
      #[cfg(not(feature="surface-contour"))]
      {let _=(graph,config);return Err("surface-contour feature not compiled".into());}}}
    Source::GltfScene{bytes,buffers,derive_tangents,scene_index}=>{
     #[cfg(feature="mesh-import")]
     {super::node_ingest::instantiate(bytes,buffers,*derive_tangents,*scene_index)?.streams}
     #[cfg(not(feature="mesh-import"))]
     {let _=(bytes,buffers,derive_tangents,scene_index);return Err("mesh-import feature not compiled".into());}}};
   resolved[n.source]=Some(streams);}}
 let mut draws=Vec::new();let mut vertices=0usize;let mut triangles=0usize;
 for n in &s.nodes{let streams=resolved[n.source].as_ref().ok_or("internal: source resolution skipped")?;
  for st in streams{vertices=vertices.checked_add(st.vertices.len()).ok_or("aggregate vertex overflow")?;triangles+=st.indices.len();
   if vertices>4_000_000{return Err("scene aggregate max4M vertices".into());}
   draws.push(Draw{stream:st.clone(),world:n.world,surface:s.materials[n.material].clone()});}}
 let resolved_count=resolved.iter().filter(|r|r.is_some()).count();
 crate::guard::check();let draw_count=draws.len();
 Ok(Compiled{draws,lights:s.lights.clone(),camera:s.camera,stats:Stats{sources:s.sources.len(),sources_resolved:resolved_count,nodes:s.nodes.len(),draws:draw_count,vertices,triangles,contour_polygonized:polygonized,contour_cache_hits:hits}})}

#[cfg(test)]mod tests{use super::*;
 fn cam()->Camera{Camera{eye:[0.,0.,4.],pan:0.,tilt:0.,roll:0.,sensor_height:24.,focal_length:35.,near:0.1}}
 fn surf()->Surface{Surface{albedo:[0.8,0.2,0.2],metallic:0.,roughness:0.5,base:None,metal_rough:None,occlusion:None,normal:None,ao_strength:1.,normal_scale:1.,double_sided:false}}
 fn ident()->Matrix{[[1.,0.,0.,0.],[0.,1.,0.,0.],[0.,0.,1.,0.],[0.,0.,0.,1.]]}
 fn sphere_source()->Source{Source::Primitive{primitive:Primitive::Sphere{radius:1.,longitude_slices:12,latitude_slices:6}}}
 #[test]fn empty_scene_is_zero_draws_not_error(){let s=Scene{sources:vec![],nodes:vec![],materials:vec![],lights:vec![],camera:cam()};let c=compile(&s,&mut Cache::new()).unwrap();assert!(c.draws.is_empty());assert_eq!((c.stats.vertices,c.stats.triangles,c.stats.sources_resolved),(0,0,0));}
 #[test]fn unreferenced_sources_not_resolved(){let s=Scene{sources:vec![sphere_source()],nodes:vec![],materials:vec![surf()],lights:vec![],camera:cam()};let c=compile(&s,&mut Cache::new()).unwrap();assert_eq!(c.stats.sources_resolved,0,"no node -> no generate");assert!(c.draws.is_empty());}
 #[test]fn dangling_refs_and_bad_matrix_are_typed_errors(){let base=Scene{sources:vec![sphere_source()],nodes:vec![],materials:vec![surf()],lights:vec![],camera:cam()};
  let mut s=base.clone();s.nodes.push(Node{source:1,world:ident(),material:0});assert!(compile(&s,&mut Cache::new()).unwrap_err().contains("node 0 references source 1 of 1"));
  let mut s=base.clone();s.nodes.push(Node{source:0,world:ident(),material:7});assert!(compile(&s,&mut Cache::new()).unwrap_err().contains("node 0 references material 7 of 1"));
  let mut s=base;let mut m=ident();m[0][0]=f64::NAN;s.nodes.push(Node{source:0,world:m,material:0});assert!(compile(&s,&mut Cache::new()).unwrap_err().contains("node 0: finite affine"));}
 #[test]fn primitive_node_draw_carries_unbaked_world_and_surface(){let mut w=ident();w[0][3]=2.5;let s=Scene{sources:vec![sphere_source()],nodes:vec![Node{source:0,world:w,material:0}],materials:vec![surf()],lights:vec![Light::Directional{travel_direction:[0.,0.,-1.],radiance:[1.;3]}],camera:cam()};
  let c=compile(&s,&mut Cache::new()).unwrap();assert_eq!(c.draws.len(),1);let d=&c.draws[0];assert_eq!(d.world,w,"world echoed unbaked");assert_eq!(d.surface.albedo,[0.8,0.2,0.2]);
  let g=generate(&Primitive::Sphere{radius:1.,longitude_slices:12,latitude_slices:6}).unwrap();assert_eq!(d.stream.vertices.len(),g.vertices.len());assert_eq!(d.stream.vertices[0].position,g.vertices[0].position,"stream source-local, bit-identical to direct generate");
  assert_eq!(c.stats.draws,1);assert_eq!(c.lights.len(),1);}
 #[cfg(feature="surface-contour")]
 #[test]fn sdf_contour_cached_by_content_hash(){let graph=Graph{nodes:vec![crate::threed::sdf_graph::Field::Sphere{center:[0.;3],radius:1.}],output:0};let config=contour_wrapped::Config{bounds:[[-1.5;3],[1.5;3]],resolution:12,normal_epsilon:0.001};
  // two byte-identical SDF sources + three nodes -> ONE polygonization, one hash hit
  let s=Scene{sources:vec![Source::SdfContour{graph:graph.clone(),config:config.clone()},Source::SdfContour{graph,config}],nodes:vec![Node{source:0,world:ident(),material:0},Node{source:1,world:ident(),material:0},Node{source:0,world:ident(),material:0}],materials:vec![surf()],lights:vec![],camera:cam()};
  let mut cache=Cache::new();let c=compile(&s,&mut cache).unwrap();assert_eq!((c.stats.contour_polygonized,c.stats.contour_cache_hits),(1,1),"one polygonize + one hit, got {:?}",c.stats.draws);assert_eq!(c.draws.len(),3);assert_eq!(cache.contour_entries(),1);
  // second compile with the same cache: pure hits, zero polygonization
  let c2=compile(&s,&mut cache).unwrap();assert_eq!((c2.stats.contour_polygonized,c2.stats.contour_cache_hits),(0,2),"second compile: both sources hash-hit");
  for(a,b)in c.draws[0].stream.vertices.iter().zip(&c.draws[1].stream.vertices){assert_eq!(a.position,b.position);}}
 #[test]fn camera_required_and_degenerate_rejected_no_fallback(){
  // missing camera: typed serde error - there is NO default-camera fallback
  let j=serde_json::json!({"sources":[],"nodes":[],"materials":[],"lights":[]});
  assert!(serde_json::from_value::<Scene>(j).is_err(),"camera is a required field");
  // degenerate camera (near=0): typed validation error
  let mut s=Scene{sources:vec![],nodes:vec![],materials:vec![],lights:vec![],camera:cam()};s.camera.near=0.;
  assert!(compile(&s,&mut Cache::new()).unwrap_err().contains("positive sensor_height/focal_length/near"));}
 #[cfg(feature="surface-contour")]
 #[test]fn contour_cache_fifo_eviction_order_and_recontour(){
  let mk=|i|Source::SdfContour{graph:Graph{nodes:vec![crate::threed::sdf_graph::Field::Sphere{center:[0.;3],radius:0.5+i as f64*0.01}],output:0},config:contour_wrapped::Config{bounds:[[-2.;3],[2.;3]],resolution:8,normal_epsilon:0.001}};
  let run=|src:Source,cache:&mut Cache|{let s=Scene{sources:vec![src],nodes:vec![Node{source:0,world:ident(),material:0}],materials:vec![surf()],lights:vec![],camera:cam()};compile(&s,cache).unwrap().stats};
  let mut cache=Cache::new();
  for i in 0..CONTOUR_CACHE_CAP{let st=run(mk(i),&mut cache);assert_eq!(st.contour_polygonized,1,"first request contours");}
  assert_eq!(cache.contour_entries(),CONTOUR_CACHE_CAP);
  // a HIT does not refresh FIFO position: hit entry 0, then insert one more -> 0 still evicted first
  let st=run(mk(0),&mut cache);assert_eq!((st.contour_polygonized,st.contour_cache_hits),(0,1),"re-request before cap overflow hits");
  let st=run(mk(CONTOUR_CACHE_CAP),&mut cache);assert_eq!(st.contour_polygonized,1);assert_eq!(cache.contour_entries(),CONTOUR_CACHE_CAP,"cap holds");
  // exactly the front was evicted: entry 1 is still a pure hit
  let st=run(mk(1),&mut cache);assert_eq!((st.contour_polygonized,st.contour_cache_hits),(0,1),"FIFO: only the front was evicted");
  // entry 0 was evicted despite the earlier hit (strict FIFO, no refresh): re-request re-contours
  let st=run(mk(0),&mut cache);assert_eq!((st.contour_polygonized,st.contour_cache_hits),(1,0),"evicted entry re-contours");}
 #[test]fn scene_json_roundtrip_deterministic(){let s=Scene{sources:vec![sphere_source()],nodes:vec![Node{source:0,world:ident(),material:0}],materials:vec![surf()],lights:vec![Light::Point{position:[0.,0.,3.],intensity:[1.;3],min_distance:0.01}],camera:cam()};
  let j=serde_json::to_string(&s).unwrap();let back:Scene=serde_json::from_str(&j).unwrap();assert_eq!(serde_json::to_string(&back).unwrap(),j,"JSON round-trip stable");}
}
