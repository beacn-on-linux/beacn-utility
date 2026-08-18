mod audio;
pub mod device;
mod ffi;
mod pod;
pub mod spectrum;

const TO_U32: fn(&String) -> Option<u32> = |s: &String| s.parse::<u32>().ok();
const TO_BOOL: fn(&String) -> Option<bool> = |s: &String| s.parse::<bool>().ok();
