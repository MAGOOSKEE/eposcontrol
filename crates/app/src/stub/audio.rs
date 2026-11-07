//! Stand-ins for the Windows audio endpoint helpers.

pub fn init_com() {}
pub fn render_volume() -> Option<f32> {
    None
}
pub fn set_render_volume(_v: f32) {}
pub fn capture_volume() -> Option<f32> {
    None
}
pub fn set_capture_volume(_v: f32) {}
pub fn capture_muted() -> Option<bool> {
    None
}
pub fn toggle_capture_mute() -> Option<bool> {
    None
}
