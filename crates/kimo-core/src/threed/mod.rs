//! Opt-in MCP-first 3D foundation. Original core math. No legacy render changes.
pub mod mesh;
pub mod spatial;
pub mod flat_sah;
pub mod sdf_graph;
pub mod contour_wrapped;
pub mod node_ingest;
pub mod offscreen_wgpu;
pub mod texture_material;
pub mod mikk_adapter;
pub mod gltf_surface;
pub mod textured_bvh;
pub mod textured_fixture;
pub mod sdf;
pub mod pbr;
pub mod cpu;
pub mod backend;
pub mod import;

pub mod primitives;
pub mod perspective;
pub mod vertex_stream;
pub mod ingest;
pub mod lighting_stage;
pub mod lit_raster;
