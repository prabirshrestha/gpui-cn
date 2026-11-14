//! Runs the unit tests of the build script's modules.

#[path = "../build/compiler_rt.rs"]
#[allow(dead_code)]
mod compiler_rt;

#[path = "../build/source.rs"]
#[allow(dead_code)]
mod source;

#[path = "../build/targets.rs"]
#[allow(dead_code)]
mod targets;
