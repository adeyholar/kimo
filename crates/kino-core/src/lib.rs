//! kino-core: project model, keyframes, command registry, CPU renderer.
//! Every user-visible operation is a registered Command; the MCP server
//! is generated from the registry, so nothing can exist without a tool.
pub mod keyframe;
pub mod model;
pub mod commands;
pub mod render;
pub mod effects;
pub mod text;
pub mod audio;
pub mod tracking;
pub mod edit;
pub mod effects2;
pub mod color;
pub mod keying;
pub mod colortools;
