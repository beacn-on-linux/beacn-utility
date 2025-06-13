/// Define Whether a type is a Float
pub trait NumericType {
    const IS_FLOAT: bool;
}
impl NumericType for f32 {
    const IS_FLOAT: bool = true;
}
impl NumericType for f64 {
    const IS_FLOAT: bool = true;
}
impl NumericType for u8 {
    const IS_FLOAT: bool = false;
}
impl NumericType for u16 {
    const IS_FLOAT: bool = false;
}
impl NumericType for u32 {
    const IS_FLOAT: bool = false;
}
impl NumericType for u64 {
    const IS_FLOAT: bool = false;
}
impl NumericType for usize {
    const IS_FLOAT: bool = false;
}
impl NumericType for i8 {
    const IS_FLOAT: bool = false;
}
impl NumericType for i16 {
    const IS_FLOAT: bool = false;
}
impl NumericType for i32 {
    const IS_FLOAT: bool = false;
}
impl NumericType for i64 {
    const IS_FLOAT: bool = false;
}
impl NumericType for isize {
    const IS_FLOAT: bool = false;
}
