use emath::Numeric;
use std::ops::RangeInclusive;

pub mod drag_value;
pub mod slider;

/// Rounds a raw `f64` into a Num
pub(crate) fn to_num<Num: Numeric>(value: f64) -> Num {
    Num::from_f64(if Num::INTEGRAL { value.round() } else { value })
}

/// This isn't part of emath, but clamps a value to a range.
pub(crate) fn clamp_to_range(x: f64, range: &RangeInclusive<f64>) -> f64 {
    let (mut min, mut max) = (*range.start(), *range.end());
    if min.total_cmp(&max) == std::cmp::Ordering::Greater {
        std::mem::swap(&mut min, &mut max);
    }

    match x.total_cmp(&min) {
        std::cmp::Ordering::Less | std::cmp::Ordering::Equal => min,
        std::cmp::Ordering::Greater => match x.total_cmp(&max) {
            std::cmp::Ordering::Greater | std::cmp::Ordering::Equal => max,
            std::cmp::Ordering::Less => x,
        },
    }
}
