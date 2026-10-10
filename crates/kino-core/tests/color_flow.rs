use kino_core::effects::Frame;
#[test]fn float_flow_preserves_constant_radiance(){let c=[4.,0.18,-0.03,0.6];let f=Frame{w:32,h:32,d:(0..1024).flat_map(|_|c).collect()};let b=kino_core::flow::flow_blur(&f,&f,0.5,3);for (a,b)in f.d.iter().zip(&b.d){assert!((a-b).abs()<1e-6)}}
