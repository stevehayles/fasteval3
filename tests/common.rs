#[inline]
/// # Panics
/// Panics when values' error margin reaches the EPSILON threshold.
pub(crate) fn assert_error_margin(value_one: f64, value_two: f64) {
    assert!((value_one - value_two).abs() < f64::EPSILON);
}