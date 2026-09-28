//! Numeric semantics: every case in `fixtures/arith.rs` gives the same result
//! (or panics where the TS throws) in Rust and in the generated package.

#[macro_use]
mod support;

purecrate_canon::fixture!(mod arith = "fixtures/arith.rs");

const SOURCE: &str = arith::SOURCE;

#[test]
fn generated_arithmetic_matches_rust_debug_build() {
    let cases = support::quietly(|| {
        vec![
            case!(arith::add_i32(1i32, 2i32)),
            case!(arith::add_i32(i32::MAX, 1i32)),
            case!(arith::sub_i32(i32::MIN, 1i32)),
            case!(arith::mul_i32(46341i32, 46341i32)),
            case!(arith::mul_i32(-46340i32, 46340i32)),
            case!(arith::mul_i32(65536i32, 65536i32)),
            case!(arith::div_i32(7i32, 2i32)),
            case!(arith::div_i32(-7i32, 2i32)),
            case!(arith::div_i32(7i32, 0i32)),
            case!(arith::div_i32(i32::MIN, -1i32)),
            case!(arith::div_i32(-1i32, 2i32)),
            case!(arith::rem_i32(-7i32, 2i32)),
            case!(arith::rem_i32(7i32, -2i32)),
            case!(arith::rem_i32(-4i32, 2i32)),
            case!(arith::rem_i32(7i32, 0i32)),
            case!(arith::rem_i32(i32::MIN, -1i32)),
            case!(arith::neg_i32(5i32)),
            case!(arith::neg_i32(0i32)),
            case!(arith::neg_i32(i32::MIN)),
            case!(arith::half_i32(7i32)),
            case!(arith::half_i32(-7i32)),
            case!(arith::min_i32()),
            case!(arith::add_u8(200u8, 55u8)),
            case!(arith::add_u8(200u8, 56u8)),
            case!(arith::sub_u8(0u8, 1u8)),
            case!(arith::div_i8(i8::MIN, -1i8)),
            case!(arith::rem_i8(i8::MIN, -1i8)),
            case!(arith::mul_u32(65535u32, 65537u32)),
            case!(arith::mul_u32(65536u32, 65536u32)),
            case!(arith::mul_u32(4294967295u32, 4294967295u32)),
            case!(arith::add_i64(i64::MAX, 0i64)),
            case!(arith::add_i64(i64::MAX, 1i64)),
            case!(arith::mul_i64(3037000499i64, 3037000499i64)),
            case!(arith::mul_i64(3037000500i64, 3037000500i64)),
            case!(arith::div_i64(-7i64, 2i64)),
            case!(arith::div_i64(1i64, 0i64)),
            case!(arith::div_i64(i64::MIN, -1i64)),
            case!(arith::rem_i64(-7i64, 2i64)),
            case!(arith::rem_i64(i64::MIN, -1i64)),
            case!(arith::neg_i64(i64::MIN)),
            case!(arith::sub_u64(0u64, 1u64)),
            case!(arith::sub_u64(u64::MAX, 1u64)),
            case!(arith::affine_i64(4i64)),
            case!(arith::affine_i64(3074457345618258602i64)),
            case!(arith::affine_i64(3074457345618258603i64)),
            case!(arith::small_i64(9i64)),
            case!(arith::small_i64(10i64)),
            case!(arith::scaled(1i32)),
            case!(arith::scaled(-1i32)),
            case!(arith::grouped_i32(1i32, 2i32, 3i32)),
            case!(arith::avg_f32(0.1f32, 0.2f32)),
            case!(arith::avg_f32(f32::MAX, f32::MAX)),
            case!(arith::third_f32(1.0f32)),
            case!(arith::tie_f32()),
            case!(arith::tie_f32_inferred(0.0f32)),
            case!(arith::poly_f64(3.0f64)),
            case!(arith::poly_f64(0.1f64)),
            case!(arith::div_f64(1.0f64, 0.0f64)),
            case!(arith::div_f64(-1.0f64, 3.0f64)),
        ]
    });
    support::assert_equivalent("arith", SOURCE, &cases);
}
