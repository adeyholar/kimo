//! SDF contour frame fixture (CP2 render half of double verification): the CP1
//! torus+capsule smooth-union and a sharp box polygonized via DualContouring+MinimiseQEF
//! (analytic Hermite normals), then rasterized through lit_raster (ACES PBR) for visual
//! sliver/flipped-normal inspection. Deterministic FNV-1a hash at 320x180; env-gated
//! 640x360 PNG dumps (KIMO_SDF_FRAMES). Topology census asserted closed+manifold.
#![cfg(feature="surface-contour")]
use kimo_core::threed::{sdf_graph::{Field,Graph},contour_wrapped::{self,Config},lit_raster,pbr::Material,lighting_stage::Light};
use kimo_core::camera_projection::Camera;

fn fnv(bytes:&[u8])->u64{let mut h=0xcbf29ce484222325u64;for b in bytes{h^=*b as u64;h=h.wrapping_mul(0x100000001b3);}h}

fn union_scene()->Graph{Graph{nodes:vec![
 Field::Torus{center:[0.,-0.15,0.],major_radius:1.05,minor_radius:0.42},
 Field::Capsule{a:[-0.85,-0.95,0.15],b:[0.85,0.95,-0.15],radius:0.34},
 Field::SmoothUnion{a:0,b:1,k:0.35}],output:2}}

fn camera(eye_scale:f64)->Camera{Camera{eye:[2.4*eye_scale,1.9*eye_scale,-3.4*eye_scale],pan:324.79,tilt:24.55,roll:0.,sensor_height:24.,focal_length:35.,near:0.1}}

fn contour_frame(g:&Graph,cfg:&Config,eye_scale:f64,w:usize,h:usize)->(kimo_core::effects::Frame,usize,contour_wrapped::Contour){
 let m=contour_wrapped::polygonize(g,cfg).unwrap();
 let material=Material{albedo:[0.74,0.68,0.6],metallic:0.,roughness:0.55};
 let lights=[Light::Directional{travel_direction:[-0.3,-0.5,1.],radiance:[2.5;3]}];
 let out=lit_raster::render(&m.stream,&camera(eye_scale),w,h,&material,&lights).unwrap();
 (out.frame,out.covered,m)
}

fn hash_of(f:&kimo_core::effects::Frame)->u64{let mut raw=Vec::with_capacity(f.d.len()*4);for v in &f.d{raw.extend_from_slice(&v.to_le_bytes());}fnv(&raw)}

fn dump(dir:&str,name:&str,g:&Graph,cfg:&Config,eye_scale:f64){
 let(frame,covered,m)=contour_frame(g,cfg,eye_scale,640,360);
 assert!(m.topology_verified,"dump scene must verify");
 let rgba=kimo_core::render::to_rgba8(&frame);
 let mut ch=std::process::Command::new("ffmpeg").args(["-v","error","-y","-f","rawvideo","-pix_fmt","rgba","-s","640x360","-i","-","-frames:v","1",&format!("{dir}/{name}")]).stdin(std::process::Stdio::piped()).spawn().unwrap();
 std::io::Write::write_all(&mut ch.stdin.take().unwrap(),&rgba).unwrap();assert!(ch.wait().unwrap().success());
 println!("CONTOUR-FRAME {name} 640x360 covered={covered} dumped");
}

#[test]fn contour_union_frame_topology_and_hash(){
 let cfg=Config{bounds:[[-2.2;3],[2.2;3]],resolution:48,normal_epsilon:0.001};
 let(frame,covered,m)=contour_frame(&union_scene(),&cfg,2.0,320,180);
 println!("CONTOUR-FRAME union 320x180 covered={} hash={:016x} tris={} euler={} vol={:.3} residual={:.4} fb={} slivers={}",covered,hash_of(&frame),m.stream.indices.len(),m.euler_characteristic,m.signed_volume,m.sampled_field_residual_max,m.normal_fallbacks,m.sliver_triangles);
 // capsule threads the torus hole: genus-2 union, so Euler = 2-2g = -2 (measured, not assumed).
 assert!(m.topology_verified&&m.euler_characteristic==-2,"union contour census: v={} e={} b={} nm={} wind={} vol={}",m.topology_verified,m.euler_characteristic,m.boundary_edges,m.nonmanifold_edges,m.winding_consistent,m.signed_volume);
 assert_eq!((m.normal_fallbacks,m.sliver_triangles,m.clamped_to_volume),(0,0,0),"union telemetry");
 assert!(covered>2_500,"covered {covered}");
 const EXPECT:u64=0x27da6e2b916e26c1;
 if EXPECT!=0{assert_eq!(hash_of(&frame),EXPECT,"contour union frame hash drift");}
 if let Ok(dir)=std::env::var("KIMO_SDF_FRAMES"){dump(&dir,"sdf-cp2-contour-union.png",&union_scene(),&cfg,2.0);}
}

#[test]fn contour_box_frame_sharp_normals_and_hash(){
 let g=Graph{nodes:vec![Field::Box{center:[0.;3],half_size:[0.6;3]}],output:0};
 let cfg=Config{bounds:[[-1.6;3],[1.6;3]],resolution:32,normal_epsilon:0.001};
 let(frame,covered,m)=contour_frame(&g,&cfg,1.0,320,180);
 println!("CONTOUR-FRAME box 320x180 covered={} hash={:016x} tris={} euler={} vol={:.3} fb={} slivers={}",covered,hash_of(&frame),m.stream.indices.len(),m.euler_characteristic,m.signed_volume,m.normal_fallbacks,m.sliver_triangles);
 assert!(m.topology_verified&&m.euler_characteristic==2,"box contour census: v={} e={} b={} nm={} wind={} vol={}",m.topology_verified,m.euler_characteristic,m.boundary_edges,m.nonmanifold_edges,m.winding_consistent,m.signed_volume);
 assert!(covered>2_000,"covered {covered}");
 const EXPECT:u64=0x95cf094c6b7c4f75;
 if EXPECT!=0{assert_eq!(hash_of(&frame),EXPECT,"contour box frame hash drift");}
 if let Ok(dir)=std::env::var("KIMO_SDF_FRAMES"){dump(&dir,"sdf-cp2-contour-box.png",&g,&cfg,1.0);}
}
