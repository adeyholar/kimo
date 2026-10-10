//! Opt-in project color pipeline. Legacy absent/default is unchanged.
use serde::{Serialize,Deserialize};
use crate::{effects::Frame,model::{Project,Clip}};
#[derive(Clone,Debug,Serialize,Deserialize,PartialEq)]
#[serde(default)]pub struct Pipeline {
    pub linear_resample:bool,pub mastering:Option<super::metadata::Mastering>,pub enabled:bool,pub working_gamut:usize,pub input_gamut:usize,pub input_transfer:usize,
    pub output_gamut:usize,pub output_transfer:usize,pub reference_white:f64,pub tone_map:bool,pub gamut_compress:bool,
}
impl Default for Pipeline{fn default()->Self{Self{linear_resample:false,mastering:None,enabled:false,working_gamut:3,input_gamut:0,input_transfer:1,output_gamut:0,output_transfer:1,reference_white:100.,tone_map:false,gamut_compress:false}}}
impl Pipeline{pub fn validate(&self)->Result<(),String>{if self.working_gamut>7||self.input_gamut>7||self.output_gamut>7||self.input_transfer>8||self.output_transfer>8||!self.reference_white.is_finite()||!(1. ..=10000.).contains(&self.reference_white){return Err("invalid project color pipeline".into())}if let Some(m)=&self.mastering{m.validate()?;if !self.enabled||self.output_gamut!=1||self.output_transfer!=6{return Err("static mastering metadata requires enabled Rec2020 PQ output".into())}}Ok(())}}
fn transform(f:&mut Frame,from:usize,to:usize,a:usize,b:usize,white:f64,tone:bool,compress:bool)->Result<(),String>{
    let mut params=crate::effects::Params::new();for(k,v)in[("from_gamut",from as f64),("to_gamut",to as f64),("from_transfer",a as f64),("to_transfer",b as f64),("reference_white",white),("tone_map",if tone{1.}else{0.}),("gamut_compress",if compress{1.}else{0.})]{params.insert(k.into(),crate::keyframe::Prop::new(v));}super::managed::apply(&params,f,0.)
}
pub fn input(p:&Project,c:&Clip,f:&mut Frame)->Result<(),String>{let q=&p.color_pipeline;if !q.enabled{return Ok(())}q.validate()?;
    let is_media=matches!(c.source,crate::model::Source::Media{..});let (g,t)=if is_media {let g=c.props.get("input_gamut").map(|v|v.base).unwrap_or(q.input_gamut as f64);let t=c.props.get("input_transfer").map(|v|v.base).unwrap_or(q.input_transfer as f64);if g.fract()!=0.||t.fract()!=0.||!(0. ..=7.).contains(&g)||!(0. ..=8.).contains(&t){return Err("invalid clip input color".into())}(g as usize,t as usize)}else{(0,1)};
    transform(f,g,q.working_gamut,t,0,q.reference_white,false,false)
}
pub fn output(p:&Project,f:&mut Frame)->Result<(),String>{let q=&p.color_pipeline;if !q.enabled{return Ok(())}q.validate()?;transform(f,q.working_gamut,q.output_gamut,0,q.output_transfer,q.reference_white,q.tone_map,q.gamut_compress)}
pub fn rgba16(f:&Frame)->Result<Vec<u8>,String>{let mut b=Vec::with_capacity(f.d.len()*2);for v in &f.d{if !v.is_finite(){return Err("nonfinite output".into())}b.extend_from_slice(&((v.clamp(0.,1.)*65535.+0.5) as u16).to_le_bytes())}Ok(b)}

/// High-bit streaming export. PQ: Rec2020/PQ 10-bit HEVC tagged HDR10-compatible,
/// but no dynamic metadata and no implied HDR mastering certification.
pub fn export_high(p:&Project,path:&str,format:&str,crf:u32)->Result<f64,String>{
    use std::process::{Command,Stdio};use std::io::Write;
    let (codec,pix)=match format{"hevc10"=>("libx265","yuv420p10le"),"ffv1_16"=>("ffv1","gbrp16le"),_=>return Err("format must be hevc10 or ffv1_16".into())};
    if crf>51{return Err("crf 0..51 required".into())}p.color_pipeline.validate()?;
    let hdr=p.color_pipeline.enabled&&p.color_pipeline.output_transfer==6;
    if hdr&&p.color_pipeline.output_gamut!=1{return Err("PQ export requires Rec2020 output gamut=1".into())}
    if format=="ffv1_16"&&!path.to_lowercase().ends_with(".mkv"){return Err("ffv1_16 requires .mkv".into())}
    let mut cmd=Command::new("ffmpeg");cmd.args(["-v","error","-y","-f","rawvideo","-pix_fmt","rgba64le","-s",&format!("{}x{}",p.width,p.height),"-r",&p.fps.to_string(),"-i","-","-c:v",codec,"-pix_fmt",pix]);
    if p.color_pipeline.mastering.is_some()&&format!="hevc10"{return Err("static mastering metadata supported by HEVC10 only".into())}
    let mut encoder_params="pools=2:frame-threads=2:log-level=error".to_string();if let Some(m)=&p.color_pipeline.mastering{encoder_params.push(':');encoder_params.push_str(&m.x265()?);}
    if format=="hevc10"{cmd.args(["-crf",&crf.to_string(),"-x265-params",&encoder_params,"-tag:v","hvc1"]);}
    if hdr{if format=="hevc10" {cmd.args(["-vf","scale=out_color_matrix=bt2020:out_range=tv","-color_range","tv"]);}
    cmd.args(["-color_primaries","bt2020","-color_trc","smpte2084","-colorspace",if format=="hevc10"{"bt2020nc"}else{"gbr"}]);}else if p.color_pipeline.enabled&&p.color_pipeline.output_transfer==7&&p.color_pipeline.output_gamut==1{if format=="hevc10" {cmd.args(["-vf","scale=out_color_matrix=bt2020:out_range=tv","-color_range","tv"]);}
    cmd.args(["-color_primaries","bt2020","-color_trc","arib-std-b67","-colorspace",if format=="hevc10"{"bt2020nc"}else{"gbr"}]);}
    let mut child=cmd.arg(path).stdin(Stdio::piped()).spawn().map_err(|e|e.to_string())?;let mut input=child.stdin.take().ok_or("no encoder input")?;let mut cache=crate::render::Cache::new();let count=(p.duration()*p.fps).round() as usize;
    let render_result=(||->Result<(),String>{for i in 0..count{crate::guard::check();let t=i as f64/p.fps;crate::render::prepare_at(p,&mut cache,Some(t))?;let mut f=crate::render::render_frame(p,t,&cache)?;output(p,&mut f)?;input.write_all(&rgba16(&f)?).map_err(|e|e.to_string())?}Ok(())})();drop(input);
    if let Err(e)=render_result{let _=child.kill();let _=child.wait();return Err(e)}let st=child.wait().map_err(|e|e.to_string())?;if !st.success(){return Err("high-bit encoder failed".into())}
    // Keep PCM audio intact in FFV1/MKV; AAC for MP4 HEVC.
    let pcm=crate::audio::mix(p);if pcm.iter().any(|v|v.abs()>1e-6){let wav=format!("{path}.tmp.wav");let video=format!("{path}.tmp.{}",if format=="ffv1_16"{"mkv"}else{"mp4"});crate::audio::write_wav(&wav,&pcm)?;std::fs::rename(path,&video).map_err(|e|e.to_string())?;let st=Command::new("ffmpeg").args(["-v","error","-y","-i",&video,"-i",&wav,"-c:v","copy","-c:a",if format=="ffv1_16"{"pcm_s16le"}else{"aac"},"-shortest",path]).status().map_err(|e|e.to_string())?;let _=std::fs::remove_file(video);let _=std::fs::remove_file(wav);if !st.success(){return Err("high-bit audio mux failed".into())}}
    Ok(count as f64/p.fps)
}
