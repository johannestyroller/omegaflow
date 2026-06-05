//! Tests for core/numerics.rs

use nebra_core::numerics::*;
use nebra_core::types::CoreError;

#[test]
fn test_wrap_360_happy_path() {
    assert_eq!(wrap_360(90.0).value, Some(90.0));
    assert_eq!(wrap_360(0.0).value, Some(0.0));
    assert_eq!(wrap_360(360.0).value, Some(0.0));
    assert_eq!(wrap_360(-90.0).value, Some(270.0));
}

#[test]
fn test_wrap_360_honor_of_zero() {
    let res = wrap_360(-0.0).value.unwrap();
    assert!(res == 0.0);
    assert!(!res.is_sign_negative());
}

#[test]
fn test_wrap_360_nan_inf() {
    assert!(matches!(
        wrap_360(f64::NAN).error,
        Some(CoreError::DomainViolation { .. })
    ));
    assert!(matches!(
        wrap_360(f64::INFINITY).error,
        Some(CoreError::DomainViolation { .. })
    ));
}

#[test]
fn test_wrap_180_era_anpm() {
    assert_eq!(wrap_180(90.0).value, Some(90.0));
    assert_eq!(wrap_180(-180.0).value, Some(180.0)); // ERFA convention
    assert_eq!(wrap_180(270.0).value, Some(-90.0));
}

#[test]
fn test_safe_acos_domain() {
    assert!(safe_acos(1.0).value.is_some());
    assert!(safe_acos(1.0000001).error.is_some());
    assert!(safe_acos(f64::NAN).error.is_some());
}

#[test]
fn test_safe_asin_domain() {
    assert!(safe_asin(-1.0).value.is_some());
    assert!(safe_asin(-1.0000001).error.is_some());
}
