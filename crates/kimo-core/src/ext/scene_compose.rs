use super::kit::*;fn read<T:serde::de::DeserializeOwned>(a:&Value,k:&str)->Result<T,String>{serde_json::from_value(a.get(k).cloned().ok_or_else(||crate::errors::arg_err(a,k,"value"))?).map_err(|e|format!("{k}: {e}"))}
pub fn commands()->Vec<Command>{vec![
 Command{name:"compile_scene3d_descriptor",description:"Compile a scene descriptor to resolved draws. Sources: primitive / sdf_contour (memoized by content hash within the call) / raw vertex stream / gltf_scene (internal node worlds baked by the glTF instantiate path). Nodes: explicit row-major affine world matrix + material index; draws carry source-local streams with UNBAKED world matrices (wgpu multi-draw contract; a CPU executor bakes via the inverse-transpose stream transform). Lights (directional/point, max64) and camera-v2 are validated and echoed for the pass level. Unreferenced sources are shape-validated, never built; empty node list compiles to zero draws. No keyframes (deferred to the Scene3D bridge), no rendering in this tool.",mutates:false,schema:obj(vec![("scene",json!({"type":"object"}))],&["scene"]),run:|_,a|{
  let scene:crate::threed::scene::Scene=read(a,"scene")?;
  let mut cache=crate::threed::scene::Cache::new();
  let out=crate::threed::scene::compile(&scene,&mut cache)?;
  Ok(serde_json::to_value(&out).map_err(|e|e.to_string())?)}},
]}
