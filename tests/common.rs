#[inline]
/// # Panics
/// Panics when values' error margin reaches the EPSILON threshold.
pub(crate) fn assert_error_margin(value_one: f32, value_two: f32) {
    assert!((value_one - value_two).abs() < f32::EPSILON);
}