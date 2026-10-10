//! Planar float GBR(A) interchange for ffmpeg EXR/PFM paths.
use crate::effects::Frame;
pub fn unpack(bytes:&[u8],w:usize,h:usize)->Result<Vec<f32>,String>{let n=w.checked_mul(h).ok_or("float dimensions overflow")?;if bytes.len()!=n.checked_mul(16).ok_or("float length overflow")?{return Err("invalid planar float frame length".into())}let mut out=vec![0.;n*4];for (plane,ch)in [1,2,0,3].iter().enumerate(){for i in 0..n{let k=(plane*n+i)*4;let v=f32::from_le_bytes(bytes[k..k+4].try_into().unwrap());if !v.is_finite(){return Err("nonfinite float source".into())}out[i*4+ch]=v;}}Ok(out)}
pub fn pack(f:&Frame)->Result<Vec<u8>,String>{let n=f.w.checked_mul(f.h).ok_or("float dimensions overflow")?;if n.checked_mul(4)!=Some(f.d.len()){return Err("invalid float frame shape".into())}let len=n.checked_mul(16).ok_or("float allocation overflow")?;crate::guard::reserve(len);let mut b=Vec::with_capacity(len);for ch in [1,2,0,3]{for i in 0..n{let v=f.d[i*4+ch];if !v.is_finite(){return Err("nonfinite float output".into())}b.extend_from_slice(&v.to_le_bytes());}}Ok(b)}
/// Bilinear, alpha-correct fit in float. Unlike swscale this does not clip float HDR.
pub fn fit(f:&Frame,w:usize,h:usize,mode:f64)->Result<Vec<f32>,String>{
 if !mode.is_finite()||f.w.checked_mul(f.h).and_then(|n|n.checked_mul(4))!=Some(f.d.len())||f.w==0||f.h==0||w==0||h==0{return Err("zero float dimensions".into())}
 let sx=w as f64/f.w as f64;let sy=h as f64/f.h as f64;
 let (sx,sy)=if mode>=1.5{(sx,sy)}else{let k=if mode>=0.5{sx.max(sy)}else{sx.min(sy)};(k,k)};
 let ox=(w as f64-f.w as f64*sx)/2.;let oy=(h as f64-f.h as f64*sy)/2.;
 let len=w.checked_mul(h).and_then(|n|n.checked_mul(4)).ok_or("float fit size overflow")?;crate::guard::reserve(len.checked_mul(4).ok_or("float fit allocation overflow")?);let mut out=vec![0.;len];for y in 0..h{for x in 0..w{let u=(x as f64+0.5-ox)/sx-0.5;let v=(y as f64+0.5-oy)/sy-0.5;if u < -0.5||v < -0.5||u>=f.w as f64-0.5||v>=f.h as f64-0.5{continue}
 let u=u.clamp(0.,(f.w-1) as f64);let v=v.clamp(0.,(f.h-1) as f64);let a=u.floor() as usize;let b=v.floor() as usize;let c=(a+1).min(f.w-1);let d=(b+1).min(f.h-1);let tx=(u-a as f64) as f32;let ty=(v-b as f64) as f32;
 let points=[(a,b,(1.-tx)*(1.-ty)),(c,b,tx*(1.-ty)),(a,d,(1.-tx)*ty),(c,d,tx*ty)];let alpha:f32=points.iter().map(|&(xx,yy,k)|f.d[(yy*f.w+xx)*4+3]*k).sum();out[(y*w+x)*4+3]=alpha;for ch in 0..3{let sum:f32=points.iter().map(|&(xx,yy,k)|{let i=(yy*f.w+xx)*4;f.d[i+ch]*f.d[i+3]*k}).sum();out[(y*w+x)*4+ch]=if alpha.abs()>1e-8{sum/alpha}else{0.};}}
 }Ok(out)
}
pub fn tiff_size(path:&str)->Result<(u32,u32),String>{let f=std::fs::File::open(path).map_err(|e|e.to_string())?;tiff::decoder::Decoder::new(std::io::BufReader::new(f)).and_then(|mut d|d.dimensions()).map_err(|e|e.to_string())}
pub fn tiff_frame(path:&str,budget:usize)->Result<Frame,String>{
 use tiff::decoder::{Decoder,DecodingResult};
 let file=std::fs::File::open(path).map_err(|e|e.to_string())?;let mut limits=tiff::decoder::Limits::default();limits.decoding_buffer_size=budget;limits.intermediate_buffer_size=budget;
 let mut dec=Decoder::new(std::io::BufReader::new(file)).map_err(|e|e.to_string())?.with_limits(limits);let(w,h)=dec.dimensions().map_err(|e|e.to_string())?;
 let channels=match dec.colortype().map_err(|e|e.to_string())?{tiff::ColorType::RGB(_)=>3,tiff::ColorType::RGBA(_)=>4,_=>return Err("TIFF supports RGB/RGBA only".into())};
 if dec.get_tag_u32(tiff::tags::Tag::Orientation).unwrap_or(1)!=1{return Err("TIFF orientation must be top-left (1)".into())}
 if channels==4&&dec.get_tag_u16_vec(tiff::tags::Tag::ExtraSamples).map(|v|v.contains(&1)).unwrap_or(false){return Err("associated-alpha TIFF unsupported; provide straight-alpha TIFF".into())}
 let n=(w as usize).checked_mul(h as usize).ok_or("TIFF dimensions overflow")?;if n.checked_mul(32).ok_or("TIFF allocation overflow")?>budget{return Err("TIFF exceeds float decode budget".into())}
 crate::guard::reserve(n*32);
 let values=match dec.read_image().map_err(|e|e.to_string())?{DecodingResult::F32(v)=>v,DecodingResult::F16(v)=>v.iter().map(|v|v.to_f32()).collect(),DecodingResult::U8(v)=>v.iter().map(|v|*v as f32/255.).collect(),DecodingResult::U16(v)=>v.iter().map(|v|*v as f32/65535.).collect(),_=>return Err("TIFF supported samples: float16/32 or unsigned8/16".into())};
 if values.len()!=n*channels||values.iter().any(|v|!v.is_finite()){return Err("invalid/nonfinite TIFF samples".into())}
 let mut d=Vec::with_capacity(n*4);for v in values.chunks_exact(channels){d.extend_from_slice(&v[..3]);d.push(if channels==4{v[3]}else{1.})}Ok(Frame{w:w as usize,h:h as usize,d})
}
/// Scene-linear RGBA with standard chromaticities and caller-declared white luminance.
pub fn write_exr(f:&Frame,path:&str,gamut:usize,white:f64)->Result<(),String>{
 use exr::{meta::attribute::Chromaticities,prelude::*};
 if gamut>7||!white.is_finite()||!(1. ..=10000.).contains(&white){return Err("invalid EXR color metadata".into())}
 if f.w.checked_mul(f.h).and_then(|v|v.checked_mul(4))!=Some(f.d.len())||f.d.iter().any(|v|!v.is_finite()){return Err("invalid EXR pixels".into())}
 let (c,w)=super::managed::chroma(gamut);let xy=|q:[f64;2]|Vec2(q[0] as f32,q[1] as f32);
 let mut attributes=LayerAttributes::named("scene-linear");attributes.white_luminance=Some(white as f32);attributes.adopted_neutral=Some(xy(w));attributes.comments=Some(Text::from("Scene-linear working RGB; whiteLuminance is caller-declared, not measured. No ACES output transform."));
 let layer=Layer::new((f.w,f.h),attributes,Encoding::SMALL_FAST_LOSSLESS,SpecificChannels::rgba(|pos:Vec2<usize>|{let i=(pos.y()*f.w+pos.x())*4;(f.d[i],f.d[i+1],f.d[i+2],f.d[i+3])}));let mut image=Image::from_layer(layer);image.attributes.chromaticities=Some(Chromaticities{red:xy(c[0]),green:xy(c[1]),blue:xy(c[2]),white:xy(w)});image.write().to_file(path).map_err(|e|e.to_string())
}
