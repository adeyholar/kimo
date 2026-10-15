// Original equations match threed::pbr::sample and texture_material channel policy.
struct Light { vector_kind:vec4<f32>, rgb_min:vec4<f32> };
struct Scene { view_proj:mat4x4<f32>, eye:vec4<f32>, ambient:vec4<f32>, time_count:vec4<f32>, lights:array<Light,64> };
struct Draw { world:mat4x4<f32>, normal:mat4x4<f32> };
struct Material { base:vec4<f32>, factors:vec4<f32>, flags:vec4<f32> };
@group(0) @binding(0) var<uniform> scene:Scene;
@group(1) @binding(0) var<uniform> draw:Draw;
@group(2) @binding(0) var<uniform> mat:Material;
@group(2) @binding(1) var albedo_tex:texture_2d<f32>;
@group(2) @binding(2) var albedo_sampler:sampler;
@group(2) @binding(3) var normal_tex:texture_2d<f32>;
@group(2) @binding(4) var normal_sampler:sampler;
@group(2) @binding(5) var mr_tex:texture_2d<f32>;
@group(2) @binding(6) var mr_sampler:sampler;
@group(2) @binding(7) var ao_tex:texture_2d<f32>;
@group(2) @binding(8) var ao_sampler:sampler;
struct V { @builtin(position) clip:vec4<f32>, @location(0) world:vec3<f32>, @location(1) normal:vec3<f32>, @location(2) uv:vec2<f32>, @location(3) tangent:vec4<f32> };
@vertex fn vs(@location(0) p:vec3<f32>,@location(1) n:vec3<f32>,@location(2) uv:vec2<f32>,@location(3) tangent:vec4<f32>)->V {
 var o:V;let world=draw.world*vec4<f32>(p,1.0);o.clip=scene.view_proj*world;o.world=world.xyz;o.normal=normalize((draw.normal*vec4<f32>(n,0.0)).xyz);
 let t=(draw.world*vec4<f32>(tangent.xyz,0.0)).xyz;o.tangent=vec4<f32>(normalize(t-o.normal*dot(t,o.normal)),tangent.w*mat.flags.y);o.uv=uv;return o;
}
const PI:f32=3.141592653589793;
fn g1(x:f32,a2:f32)->f32{return 2.0*x/(x+sqrt(a2+(1.0-a2)*x*x));}
fn brdf(albedo:vec3<f32>,metal:f32,rough:f32,n:vec3<f32>,l:vec3<f32>,v:vec3<f32>)->vec3<f32>{
 let nl=dot(n,l);let nv=dot(n,v);if nl<=0.0||nv<=0.0{return vec3<f32>(0.0);}let h=normalize(l+v);let nh=clamp(dot(n,h),0.0,1.0);let vh=clamp(dot(v,h),0.0,1.0);let alpha=rough*rough;let a2=alpha*alpha;
 let denominator=nh*nh*(a2-1.0)+1.0;let d=a2/(PI*denominator*denominator);let g=g1(nl,a2)*g1(nv,a2);let f0=mix(vec3<f32>(0.04),albedo,metal);let f=f0+(vec3<f32>(1.0)-f0)*pow(1.0-vh,5.0);
 return (1.0-metal)*(vec3<f32>(1.0)-f)*albedo/PI+f*d*g/(4.0*nl*nv);
}
@fragment fn fs(v:V,@builtin(front_facing) front:bool)->@location(0) vec4<f32>{
 // Single-sided visibility supplied via draw cull mode; double-sided frame flips.
 var n=normalize(v.normal);var sign=v.tangent.w;if dot(n,scene.eye.xyz-v.world)<0.0&&mat.flags.x>0.5{n=-n;sign=-sign;}
 let t=normalize(v.tangent.xyz-n*dot(n,v.tangent.xyz));let b=cross(n,t)*sign;
 if mat.flags.z>0.5{var sampled=textureSampleLevel(normal_tex,normal_sampler,v.uv,0.0).xyz*2.0-vec3<f32>(1.0);sampled=vec3<f32>(sampled.xy*mat.factors.w,sampled.z);let mapped=normalize(sampled);n=normalize(t*mapped.x+b*mapped.y+n*mapped.z);}
 let albedo=mat.base.rgb*textureSampleLevel(albedo_tex,albedo_sampler,v.uv,0.0).rgb;
 let mr=textureSampleLevel(mr_tex,mr_sampler,v.uv,0.0);let metal=mat.factors.x*mr.b;let rough=max(0.001,mat.factors.y*mr.g);let ao=1.0+mat.factors.z*(textureSampleLevel(ao_tex,ao_sampler,v.uv,0.0).r-1.0);
 let view=normalize(scene.eye.xyz-v.world);var color=scene.ambient.rgb*albedo*(1.0-metal)*ao/PI;
 for(var i=0u;i<u32(scene.time_count.y);i=i+1u){let light=scene.lights[i];var l=normalize(-light.vector_kind.xyz);var rgb=light.rgb_min.rgb;if light.vector_kind.w>0.5{let delta=light.vector_kind.xyz-v.world;let distance=length(delta);if distance==0.0{continue;}l=delta/distance;let limited=max(distance,light.rgb_min.w);rgb=rgb/(limited*limited);}color+=brdf(albedo,metal,rough,n,l,view)*rgb*max(dot(n,l),0.0);}
 return vec4<f32>(color,1.0);
}
