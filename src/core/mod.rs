//! 核心纯逻辑层 —— 不依赖 UI，也不直接使用 `windows` crate（可测性的根基）。

pub mod gamma;
pub mod nightlight;
pub mod temp;
pub mod util;
