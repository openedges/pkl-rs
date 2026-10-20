//! Portable numeric candidates for M1; not a complete Pkl numeric implementation.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NumericError {
    IntegerOverflow,
    DivisionByZero,
}

pub fn add(a: i64, b: i64) -> Result<i64, NumericError> {
    a.checked_add(b).ok_or(NumericError::IntegerOverflow)
}
pub fn subtract(a: i64, b: i64) -> Result<i64, NumericError> {
    a.checked_sub(b).ok_or(NumericError::IntegerOverflow)
}
pub fn multiply(a: i64, b: i64) -> Result<i64, NumericError> {
    a.checked_mul(b).ok_or(NumericError::IntegerOverflow)
}
pub fn divide(a: i64, b: i64) -> Result<i64, NumericError> {
    if b == 0 {
        Err(NumericError::DivisionByZero)
    } else {
        a.checked_div(b).ok_or(NumericError::IntegerOverflow)
    }
}
pub fn remainder(a: i64, b: i64) -> Result<i64, NumericError> {
    if b == 0 {
        Err(NumericError::DivisionByZero)
    } else {
        // Java's MIN_VALUE % -1 is zero, unlike Rust's overflowing `%`.
        Ok(a.wrapping_rem(b))
    }
}

/// Java 19+ decimal selection/layout candidate. Must remain differentially tested.
/// Rust's shortest formatter supplies digits; one-digit results are rounded to
/// two significant digits, as required by Double.toString's selection rule.
pub fn double_to_string(value: f64) -> String {
    if value.is_nan() {
        return "NaN".into();
    }
    if value == f64::INFINITY {
        return "Infinity".into();
    }
    if value == f64::NEG_INFINITY {
        return "-Infinity".into();
    }
    if value == 0.0 {
        return if value.is_sign_negative() {
            "-0.0"
        } else {
            "0.0"
        }
        .into();
    }
    let negative = value.is_sign_negative();
    let magnitude = value.abs();
    let mut scientific = format!("{magnitude:e}");
    let (coefficient, _) = scientific.split_once('e').expect("scientific format");
    // Shortest formatting may choose either equidistant decimal. Explicit
    // precision selects the even decimal required by Java (e.g. ...6382/6383).
    let precision = coefficient.bytes().filter(|b| *b != b'.').count().max(2) - 1;
    scientific = format!("{magnitude:.precision$e}");
    let (coefficient, exponent) = scientific.split_once('e').expect("scientific format");
    let exponent: i32 = exponent.parse().expect("scientific exponent");
    let digits: String = coefficient.chars().filter(|c| *c != '.').collect();
    let mut result = if negative {
        "-".to_owned()
    } else {
        String::new()
    };
    if !(-3..7).contains(&exponent) {
        result.push(digits.as_bytes()[0] as char);
        result.push('.');
        result.push_str(if digits.len() == 1 { "0" } else { &digits[1..] });
        result.push('E');
        result.push_str(&exponent.to_string());
    } else if exponent < 0 {
        result.push_str("0.");
        result.extend(std::iter::repeat_n('0', (-exponent - 1) as usize));
        result.push_str(&digits);
    } else {
        let point = (exponent + 1) as usize;
        if point >= digits.len() {
            result.push_str(&digits);
            result.extend(std::iter::repeat_n('0', point - digits.len()));
            result.push_str(".0");
        } else {
            result.push_str(&digits[..point]);
            result.push('.');
            result.push_str(&digits[point..]);
        }
    }
    // A second digit zero is needed for selection, but not for plain fractions.
    if (-3..0).contains(&exponent) && result.ends_with('0') {
        result.pop();
    }
    result
}

// The following cube-root algorithm is adapted from fdlibm s_cbrt.c 1.3
// https://netlib.org/fdlibm/s_cbrt.c (source hash in docs/m1-semantics.md).
// Copyright (C) 1993 by Sun Microsystems, Inc. All rights reserved.
// Developed at SunSoft, a Sun Microsystems, Inc. business.
// Permission to use, copy, modify, and distribute this
// software is freely granted, provided that this notice
// is preserved.
/// fdlibm cube root with explicit word operations and unchanged arithmetic order.
/// Unlike the newer musl approximation, this matches the sampled StrictMath bits.
pub fn strict_cbrt(value: f64) -> f64 {
    let bits = value.to_bits();
    let sign = bits & (1_u64 << 63);
    let magnitude = bits & !(1_u64 << 63);
    let high = (magnitude >> 32) as u32;
    if high >= 0x7ff00000 || magnitude == 0 {
        return value + value;
    }
    let x = f64::from_bits(magnitude);
    let mut t = if high < 0x00100000 {
        let scaled = f64::from_bits(0x4350000000000000) * x;
        let high = (scaled.to_bits() >> 32) as u32 / 3 + 696219795;
        f64::from_bits((u64::from(high) << 32) | (scaled.to_bits() & 0xffffffff))
    } else {
        f64::from_bits(u64::from(high / 3 + 715094163) << 32)
    };
    let c = f64::from_bits(0x3fe15f15f15f15f1);
    let d = f64::from_bits(0xbfe691de2532c834);
    let e = f64::from_bits(0x3ff6a0ea0ea0ea0f);
    let f = f64::from_bits(0x3ff9b6db6db6db6e);
    let g = f64::from_bits(0x3fd6db6db6db6db7);
    let r = t * t / x;
    let s = c + r * t;
    t *= g + f / (s + e + d / s);
    t = f64::from_bits((t.to_bits() & 0xffffffff00000000) + (1_u64 << 32));
    let s = t * t;
    let r = x / s;
    let w = t + t;
    let r = (r - t) / (w + r);
    t = t + t * r;
    f64::from_bits(t.to_bits() | sign)
}

/// Software libm is a portability candidate, not yet a StrictMath guarantee.
pub fn math(operation: &str, value: f64) -> Option<f64> {
    Some(match operation {
        "sqrt" => libm::sqrt(value),
        "sin" => fdlibm_trig::sin(value),
        "cos" => fdlibm_trig::cos(value),
        "tan" => libm::tan(value),
        "exp" => libm::exp(value),
        "log" => libm::log(value),
        "cbrt" => strict_cbrt(value),
        "rint" => value.round_ties_even(),
        "ceil" => value.ceil(),
        "floor" => value.floor(),
        _ => return None,
    })
}

/// Tab-separated M1 primitive probe. Float inputs/outputs use IEEE-754 hex bits.
/// This stable test protocol is not a Pkl evaluator or diagnostic renderer.
pub fn probe(input: &str) -> Result<String, String> {
    let fields: Vec<_> = input.split('\t').collect();
    let parse_error = || "invalid numeric probe input".to_owned();
    match fields.as_slice() {
        [op @ ("add" | "sub" | "mul" | "div" | "rem"), a, b] => {
            let a = a.parse().map_err(|_| parse_error())?;
            let b = b.parse().map_err(|_| parse_error())?;
            let result = match *op {
                "add" => add(a, b),
                "sub" => subtract(a, b),
                "mul" => multiply(a, b),
                "div" => divide(a, b),
                _ => remainder(a, b),
            };
            Ok(match result {
                Ok(value) => format!("{value}\n"),
                Err(NumericError::IntegerOverflow) => "error:integerOverflow\n".into(),
                Err(NumericError::DivisionByZero) => "error:divisionByZero\n".into(),
            })
        }
        [op, bits] => {
            let value = f64::from_bits(u64::from_str_radix(bits, 16).map_err(|_| parse_error())?);
            if *op == "format" {
                return Ok(format!("{}\n", double_to_string(value)));
            }
            let result = math(op, value).ok_or_else(parse_error)?;
            // Double.doubleToLongBits canonicalizes NaN as part of this protocol;
            // formatting and NaN classification are tested separately.
            let bits = if result.is_nan() {
                0x7ff8000000000000
            } else {
                result.to_bits()
            };
            Ok(format!("{bits:016x}\n"))
        }
        _ => Err(parse_error()),
    }
}

// Trigonometric kernels below preserve fdlibm's evaluation order. The reducers
// are adapted from libm 0.2.16, using checked indexing and only the binary64
// constant table. Original sources/notices are retained in the fixture references.
#[allow(
    clippy::excessive_precision,
    clippy::approx_constant,
    clippy::assign_op_pattern,
    clippy::needless_range_loop,
    clippy::explicit_counter_loop
)]
mod fdlibm_trig {
    // Copyright (C) 1993 by Sun Microsystems, Inc. All rights reserved.
    // Developed at SunSoft, a Sun Microsystems, Inc. business.
    // Permission to use, copy, modify, and distribute this
    // software is freely granted, provided that this notice
    // is preserved.
    macro_rules! i {
        ($array:expr, $index:expr) => {
            $array[$index]
        };
        ($array:expr, $index:expr, =, $rhs:expr) => {
            $array[$index] = $rhs
        };
        ($array:expr, $index:expr, +=, $rhs:expr) => {
            $array[$index] += $rhs
        };
        ($array:expr, $index:expr, -=, $rhs:expr) => {
            $array[$index] -= $rhs
        };
        ($array:expr, $index:expr, &=, $rhs:expr) => {
            $array[$index] &= $rhs
        };
        ($array:expr, $index:expr, ==, $rhs:expr) => {
            $array[$index] == $rhs
        };
    }
    fn kernel_sin(x: f64, y: f64, tail: bool) -> f64 {
        if x.abs().to_bits() >> 32 < 0x3e400000 {
            return x;
        }
        let s1 = f64::from_bits(0xbfc5555555555549);
        let s2 = f64::from_bits(0x3f8111111110f8a6);
        let s3 = f64::from_bits(0xbf2a01a019c161d5);
        let s4 = f64::from_bits(0x3ec71de357b1fe7d);
        let s5 = f64::from_bits(0xbe5ae5e68a2b9ceb);
        let s6 = f64::from_bits(0x3de5d93a5acfd57c);
        let z = x * x;
        let v = z * x;
        let r = s2 + z * (s3 + z * (s4 + z * (s5 + z * s6)));
        if tail {
            x - ((z * (0.5 * y - v * r) - y) - v * s1)
        } else {
            x + v * (s1 + z * r)
        }
    }
    fn kernel_cos(x: f64, y: f64) -> f64 {
        let high = (x.to_bits() >> 32) & 0x7fffffff;
        if high < 0x3e400000 {
            return 1.0;
        }
        let c1 = f64::from_bits(0x3fa555555555554c);
        let c2 = f64::from_bits(0xbf56c16c16c15177);
        let c3 = f64::from_bits(0x3efa01a019cb1590);
        let c4 = f64::from_bits(0xbe927e4f809c52ad);
        let c5 = f64::from_bits(0x3e21ee9ebdb4b1c4);
        let c6 = f64::from_bits(0xbda8fae9be8838d4);
        let z = x * x;
        let r = z * (c1 + z * (c2 + z * (c3 + z * (c4 + z * (c5 + z * c6)))));
        if high < 0x3fd33333 {
            return 1.0 - (0.5 * z - (z * r - x * y));
        }
        let qx = if high > 0x3fe90000 {
            0.28125
        } else {
            f64::from_bits((high - 0x00200000) << 32)
        };
        let hz = 0.5 * z - qx;
        let a = 1.0 - qx;
        a - (hz - (z * r - x * y))
    }
    pub(super) fn sin(x: f64) -> f64 {
        if !x.is_finite() {
            return f64::NAN;
        }
        if (x.abs().to_bits() >> 32) <= 0x3fe921fb {
            return kernel_sin(x, 0.0, false);
        }
        let (n, a, b) = reduce::rem_pio2(x);
        match n & 3 {
            0 => kernel_sin(a, b, true),
            1 => kernel_cos(a, b),
            2 => -kernel_sin(a, b, true),
            _ => -kernel_cos(a, b),
        }
    }
    pub(super) fn cos(x: f64) -> f64 {
        if !x.is_finite() {
            return f64::NAN;
        }
        if (x.abs().to_bits() >> 32) <= 0x3fe921fb {
            return kernel_cos(x, 0.0);
        }
        let (n, a, b) = reduce::rem_pio2(x);
        match n & 3 {
            0 => kernel_cos(a, b),
            1 => -kernel_sin(a, b, true),
            2 => -kernel_cos(a, b),
            _ => kernel_sin(a, b, true),
        }
    }

    mod reduce {
        // origin: FreeBSD /usr/src/lib/msun/src/e_rem_pio2.c
        //
        // ====================================================
        // Copyright (C) 1993 by Sun Microsystems, Inc. All rights reserved.
        //
        // Developed at SunPro, a Sun Microsystems, Inc. business.
        // Permission to use, copy, modify, and distribute this
        // software is freely granted, provided that this notice
        // is preserved.
        // ====================================================
        //
        // Optimized by Bruce D. Evans. */
        use super::large::rem_pio2_large;

        // #if FLT_EVAL_METHOD==0 || FLT_EVAL_METHOD==1
        // #define EPS DBL_EPSILON
        const EPS: f64 = f64::EPSILON;
        // #elif FLT_EVAL_METHOD==2
        // #define EPS LDBL_EPSILON
        // #endif

        // TODO: Support FLT_EVAL_METHOD?

        const TO_INT: f64 = 1.5 / EPS;
        /// 53 bits of 2/pi
        const INV_PIO2: f64 = 6.36619772367581382433e-01; /* 0x3FE45F30, 0x6DC9C883 */
        /// first 33 bits of pi/2
        const PIO2_1: f64 = 1.57079632673412561417e+00; /* 0x3FF921FB, 0x54400000 */
        /// pi/2 - PIO2_1
        const PIO2_1T: f64 = 6.07710050650619224932e-11; /* 0x3DD0B461, 0x1A626331 */
        /// second 33 bits of pi/2
        const PIO2_2: f64 = 6.07710050630396597660e-11; /* 0x3DD0B461, 0x1A600000 */
        /// pi/2 - (PIO2_1+PIO2_2)
        const PIO2_2T: f64 = 2.02226624879595063154e-21; /* 0x3BA3198A, 0x2E037073 */
        /// third 33 bits of pi/2
        const PIO2_3: f64 = 2.02226624871116645580e-21; /* 0x3BA3198A, 0x2E000000 */
        /// pi/2 - (PIO2_1+PIO2_2+PIO2_3)
        const PIO2_3T: f64 = 8.47842766036889956997e-32; /* 0x397B839A, 0x252049C1 */

        // return the remainder of x rem pi/2 in y[0]+y[1]
        // use rem_pio2_large() for large x
        //
        // caller must handle the case when reduction is not needed: |x| ~<= pi/4 */
        pub(crate) fn rem_pio2(x: f64) -> (i32, f64, f64) {
            let x1p24 = f64::from_bits(0x4170000000000000);

            let sign = (f64::to_bits(x) >> 63) as i32;
            let ix = (f64::to_bits(x) >> 32) as u32 & 0x7fffffff;

            fn medium(x: f64, ix: u32) -> (i32, f64, f64) {
                /* rint(x/(pi/2)), Assume round-to-nearest. */
                let tmp = x * INV_PIO2 + TO_INT;
                // force rounding of tmp to it's storage format on x87 to avoid
                // excess precision issues.
                let f_n = tmp - TO_INT;
                let n = f_n as i32;
                let mut r = x - f_n * PIO2_1;
                let mut w = f_n * PIO2_1T; /* 1st round, good to 85 bits */
                let mut y0 = r - w;
                let ui = f64::to_bits(y0);
                let ey = (ui >> 52) as i32 & 0x7ff;
                let ex = (ix >> 20) as i32;
                if ex - ey > 16 {
                    /* 2nd round, good to 118 bits */
                    let t = r;
                    w = f_n * PIO2_2;
                    r = t - w;
                    w = f_n * PIO2_2T - ((t - r) - w);
                    y0 = r - w;
                    let ey = (f64::to_bits(y0) >> 52) as i32 & 0x7ff;
                    if ex - ey > 49 {
                        /* 3rd round, good to 151 bits, covers all cases */
                        let t = r;
                        w = f_n * PIO2_3;
                        r = t - w;
                        w = f_n * PIO2_3T - ((t - r) - w);
                        y0 = r - w;
                    }
                }
                let y1 = (r - y0) - w;
                (n, y0, y1)
            }

            if ix <= 0x400f6a7a {
                /* |x| ~<= 5pi/4 */
                if (ix & 0xfffff) == 0x921fb {
                    /* |x| ~= pi/2 or 2pi/2 */
                    return medium(x, ix); /* cancellation -- use medium case */
                }
                if ix <= 0x4002d97c {
                    /* |x| ~<= 3pi/4 */
                    if sign == 0 {
                        let z = x - PIO2_1; /* one round good to 85 bits */
                        let y0 = z - PIO2_1T;
                        let y1 = (z - y0) - PIO2_1T;
                        return (1, y0, y1);
                    } else {
                        let z = x + PIO2_1;
                        let y0 = z + PIO2_1T;
                        let y1 = (z - y0) + PIO2_1T;
                        return (-1, y0, y1);
                    }
                } else if sign == 0 {
                    let z = x - 2.0 * PIO2_1;
                    let y0 = z - 2.0 * PIO2_1T;
                    let y1 = (z - y0) - 2.0 * PIO2_1T;
                    return (2, y0, y1);
                } else {
                    let z = x + 2.0 * PIO2_1;
                    let y0 = z + 2.0 * PIO2_1T;
                    let y1 = (z - y0) + 2.0 * PIO2_1T;
                    return (-2, y0, y1);
                }
            }
            if ix <= 0x401c463b {
                /* |x| ~<= 9pi/4 */
                if ix <= 0x4015fdbc {
                    /* |x| ~<= 7pi/4 */
                    if ix == 0x4012d97c {
                        /* |x| ~= 3pi/2 */
                        return medium(x, ix);
                    }
                    if sign == 0 {
                        let z = x - 3.0 * PIO2_1;
                        let y0 = z - 3.0 * PIO2_1T;
                        let y1 = (z - y0) - 3.0 * PIO2_1T;
                        return (3, y0, y1);
                    } else {
                        let z = x + 3.0 * PIO2_1;
                        let y0 = z + 3.0 * PIO2_1T;
                        let y1 = (z - y0) + 3.0 * PIO2_1T;
                        return (-3, y0, y1);
                    }
                } else {
                    if ix == 0x401921fb {
                        /* |x| ~= 4pi/2 */
                        return medium(x, ix);
                    }
                    if sign == 0 {
                        let z = x - 4.0 * PIO2_1;
                        let y0 = z - 4.0 * PIO2_1T;
                        let y1 = (z - y0) - 4.0 * PIO2_1T;
                        return (4, y0, y1);
                    } else {
                        let z = x + 4.0 * PIO2_1;
                        let y0 = z + 4.0 * PIO2_1T;
                        let y1 = (z - y0) + 4.0 * PIO2_1T;
                        return (-4, y0, y1);
                    }
                }
            }
            if ix < 0x413921fb {
                /* |x| ~< 2^20*(pi/2), medium size */
                return medium(x, ix);
            }
            /*
             * all other (large) arguments
             */
            if ix >= 0x7ff00000 {
                /* x is inf or NaN */
                let y0 = f64::NAN;
                let y1 = y0;
                return (0, y0, y1);
            }
            /* set z = scalbn(|x|,-ilogb(x)+23) */
            let mut ui = f64::to_bits(x);
            ui &= (!1) >> 12;
            ui |= (0x3ff + 23) << 52;
            let mut z = f64::from_bits(ui);
            let mut tx = [0.0; 3];
            for i in 0..2 {
                i!(tx,i, =, z as i32 as f64);
                z = (z - i!(tx, i)) * x1p24;
            }
            i!(tx,2, =, z);
            /* skip zero terms, first term is non-zero */
            let mut i = 2;
            while i != 0 && i!(tx, i) == 0.0 {
                i -= 1;
            }
            let mut ty = [0.0; 3];
            let n = rem_pio2_large(&tx[..=i], &mut ty, ((ix as i32) >> 20) - (0x3ff + 23), 1);
            if sign != 0 {
                return (-n, -i!(ty, 0), -i!(ty, 1));
            }
            (n, i!(ty, 0), i!(ty, 1))
        }
    }
    mod large {

        /* origin: FreeBSD /usr/src/lib/msun/src/k_rem_pio2.c */
        /*
         * ====================================================
         * Copyright (C) 1993 by Sun Microsystems, Inc. All rights reserved.
         *
         * Developed at SunSoft, a Sun Microsystems, Inc. business.
         * Permission to use, copy, modify, and distribute this
         * software is freely granted, provided that this notice
         * is preserved.
         * ====================================================
         */

        use libm::scalbn;

        // initial value for jk
        const INIT_JK: [usize; 4] = [3, 4, 4, 6];

        // Table of constants for 2/pi, 396 Hex digits (476 decimal) of 2/pi
        //
        //              integer array, contains the (24*i)-th to (24*i+23)-th
        //              bit of 2/pi after binary point. The corresponding
        //              floating value is
        //
        //                      ipio2[i] * 2^(-24(i+1)).
        //
        // NB: This table must have at least (e0-3)/24 + jk terms.
        //     For quad precision (e0 <= 16360, jk = 6), this is 686.
        const IPIO2: [i32; 66] = [
            0xA2F983, 0x6E4E44, 0x1529FC, 0x2757D1, 0xF534DD, 0xC0DB62, 0x95993C, 0x439041,
            0xFE5163, 0xABDEBB, 0xC561B7, 0x246E3A, 0x424DD2, 0xE00649, 0x2EEA09, 0xD1921C,
            0xFE1DEB, 0x1CB129, 0xA73EE8, 0x8235F5, 0x2EBB44, 0x84E99C, 0x7026B4, 0x5F7E41,
            0x3991D6, 0x398353, 0x39F49C, 0x845F8B, 0xBDF928, 0x3B1FF8, 0x97FFDE, 0x05980F,
            0xEF2F11, 0x8B5A0A, 0x6D1F6D, 0x367ECF, 0x27CB09, 0xB74F46, 0x3F669E, 0x5FEA2D,
            0x7527BA, 0xC7EBE5, 0xF17B3D, 0x0739F7, 0x8A5292, 0xEA6BFB, 0x5FB11F, 0x8D5D08,
            0x560330, 0x46FC7B, 0x6BABF0, 0xCFBC20, 0x9AF436, 0x1DA9E3, 0x91615E, 0xE61B08,
            0x659985, 0x5F14A0, 0x68408D, 0xFFD880, 0x4D7327, 0x310606, 0x1556CA, 0x73A8C9,
            0x60E27B, 0xC08C6B,
        ];

        const PIO2: [f64; 8] = [
            1.57079625129699707031e+00, /* 0x3FF921FB, 0x40000000 */
            7.54978941586159635335e-08, /* 0x3E74442D, 0x00000000 */
            5.39030252995776476554e-15, /* 0x3CF84698, 0x80000000 */
            3.28200341580791294123e-22, /* 0x3B78CC51, 0x60000000 */
            1.27065575308067607349e-29, /* 0x39F01B83, 0x80000000 */
            1.22933308981111328932e-36, /* 0x387A2520, 0x40000000 */
            2.73370053816464559624e-44, /* 0x36E38222, 0x80000000 */
            2.16741683877804819444e-51, /* 0x3569F31D, 0x00000000 */
        ];

        // fn rem_pio2_large(x : &[f64], y : &mut [f64], e0 : i32, prec : usize) -> i32
        //
        // Input parameters:
        //      x[]     The input value (must be positive) is broken into nx
        //              pieces of 24-bit integers in double precision format.
        //              x[i] will be the i-th 24 bit of x. The scaled exponent
        //              of x[0] is given in input parameter e0 (i.e., x[0]*2^e0
        //              match x's up to 24 bits.
        //
        //              Example of breaking a double positive z into x[0]+x[1]+x[2]:
        //                      e0 = ilogb(z)-23
        //                      z  = scalbn(z,-e0)
        //              for i = 0,1,2
        //                      x[i] = floor(z)
        //                      z    = (z-x[i])*2**24
        //
        //      y[]     output result in an array of double precision numbers.
        //              The dimension of y[] is:
        //                      24-bit  precision       1
        //                      53-bit  precision       2
        //                      64-bit  precision       2
        //                      113-bit precision       3
        //              The actual value is the sum of them. Thus for 113-bit
        //              precison, one may have to do something like:
        //
        //              long double t,w,r_head, r_tail;
        //              t = (long double)y[2] + (long double)y[1];
        //              w = (long double)y[0];
        //              r_head = t+w;
        //              r_tail = w - (r_head - t);
        //
        //      e0      The exponent of x[0]. Must be <= 16360 or you need to
        //              expand the ipio2 table.
        //
        //      prec    an integer indicating the precision:
        //                      0       24  bits (single)
        //                      1       53  bits (double)
        //                      2       64  bits (extended)
        //                      3       113 bits (quad)
        //
        // Here is the description of some local variables:
        //
        //      jk      jk+1 is the initial number of terms of ipio2[] needed
        //              in the computation. The minimum and recommended value
        //              for jk is 3,4,4,6 for single, double, extended, and quad.
        //              jk+1 must be 2 larger than you might expect so that our
        //              recomputation test works. (Up to 24 bits in the integer
        //              part (the 24 bits of it that we compute) and 23 bits in
        //              the fraction part may be lost to cancelation before we
        //              recompute.)
        //
        //      jz      local integer variable indicating the number of
        //              terms of ipio2[] used.
        //
        //      jx      nx - 1
        //
        //      jv      index for pointing to the suitable ipio2[] for the
        //              computation. In general, we want
        //                      ( 2^e0*x[0] * ipio2[jv-1]*2^(-24jv) )/8
        //              is an integer. Thus
        //                      e0-3-24*jv >= 0 or (e0-3)/24 >= jv
        //              Hence jv = max(0,(e0-3)/24).
        //
        //      jp      jp+1 is the number of terms in PIo2[] needed, jp = jk.
        //
        //      q[]     double array with integral value, representing the
        //              24-bits chunk of the product of x and 2/pi.
        //
        //      q0      the corresponding exponent of q[0]. Note that the
        //              exponent for q[i] would be q0-24*i.
        //
        //      PIo2[]  double precision array, obtained by cutting pi/2
        //              into 24 bits chunks.
        //
        //      f[]     ipio2[] in floating point
        //
        //      iq[]    integer array by breaking up q[] in 24-bits chunk.
        //
        //      fq[]    final product of x*(2/pi) in fq[0],..,fq[jk]
        //
        //      ih      integer. If >0 it indicates q[] is >= 0.5, hence
        //              it also indicates the *sign* of the result.

        /// Return the last three digits of N with y = x - N*pi/2
        /// so that |y| < pi/2.
        ///
        /// The method is to compute the integer (mod 8) and fraction parts of
        /// (2/pi)*x without doing the full multiplication. In general we
        /// skip the part of the product that are known to be a huge integer (
        /// more accurately, = 0 mod 8 ). Thus the number of operations are
        /// independent of the exponent of the input.
        pub(crate) fn rem_pio2_large(x: &[f64], y: &mut [f64], e0: i32, prec: usize) -> i32 {
            // FIXME(rust-lang/rust#144518): Inline assembly would cause `no_panic` to fail
            // on the callers of this function. As a workaround, avoid inlining `floor` here
            // when implemented with assembly.
            extern "C" fn floor(x: f64) -> f64 {
                x.floor()
            }

            let x1p24 = f64::from_bits(0x4170000000000000); // 0x1p24 === 2 ^ 24
            let x1p_24 = f64::from_bits(0x3e70000000000000); // 0x1p_24 === 2 ^ (-24)

            if cfg!(target_pointer_width = "64") {
                debug_assert!(e0 <= 16360);
            }

            let nx = x.len();

            let mut fw: f64;
            let mut n: i32;
            let mut ih: i32;
            let mut z: f64;
            let mut f: [f64; 20] = [0.; 20];
            let mut fq: [f64; 20] = [0.; 20];
            let mut q: [f64; 20] = [0.; 20];
            let mut iq: [i32; 20] = [0; 20];

            /* initialize jk*/
            let jk = i!(INIT_JK, prec);
            let jp = jk;

            /* determine jx,jv,q0, note that 3>q0 */
            let jx = nx - 1;
            let mut jv = (e0 - 3) / 24;
            if jv < 0 {
                jv = 0;
            }
            let mut q0 = e0 - 24 * (jv + 1);
            let jv = jv as usize;

            /* set up f[0] to f[jx+jk] where f[jx+jk] = ipio2[jv+jk] */
            let mut j = (jv as i32) - (jx as i32);
            let m = jx + jk;
            for i in 0..=m {
                i!(f, i, =, if j < 0 {
                    0.
                } else {
                    i!(IPIO2, j as usize) as f64
                });
                j += 1;
            }

            /* compute q[0],q[1],...q[jk] */
            for i in 0..=jk {
                fw = 0f64;
                for j in 0..=jx {
                    fw += i!(x, j) * i!(f, jx + i - j);
                }
                i!(q, i, =, fw);
            }

            let mut jz = jk;

            'recompute: loop {
                /* distill q[] into iq[] reversingly */
                let mut i = 0i32;
                z = i!(q, jz);
                for j in (1..=jz).rev() {
                    fw = (x1p_24 * z) as i32 as f64;
                    i!(iq, i as usize, =, (z - x1p24 * fw) as i32);
                    z = i!(q, j - 1) + fw;
                    i += 1;
                }

                /* compute n */
                z = scalbn(z, q0); /* actual value of z */
                z -= 8.0 * floor(z * 0.125); /* trim off integer >= 8 */
                n = z as i32;
                z -= n as f64;
                ih = 0;
                if q0 > 0 {
                    /* need iq[jz-1] to determine n */
                    i = i!(iq, jz - 1) >> (24 - q0);
                    n += i;
                    i!(iq, jz - 1, -=, i << (24 - q0));
                    ih = i!(iq, jz - 1) >> (23 - q0);
                } else if q0 == 0 {
                    ih = i!(iq, jz - 1) >> 23;
                } else if z >= 0.5 {
                    ih = 2;
                }

                if ih > 0 {
                    /* q > 0.5 */
                    n += 1;
                    let mut carry = 0i32;
                    for i in 0..jz {
                        /* compute 1-q */
                        let j = i!(iq, i);
                        if carry == 0 {
                            if j != 0 {
                                carry = 1;
                                i!(iq, i, =, 0x1000000 - j);
                            }
                        } else {
                            i!(iq, i, =, 0xffffff - j);
                        }
                    }
                    if q0 > 0 {
                        /* rare case: chance is 1 in 12 */
                        match q0 {
                            1 => {
                                i!(iq, jz - 1, &=, 0x7fffff);
                            }
                            2 => {
                                i!(iq, jz - 1, &=, 0x3fffff);
                            }
                            _ => {}
                        }
                    }
                    if ih == 2 {
                        z = 1. - z;
                        if carry != 0 {
                            z -= scalbn(1., q0);
                        }
                    }
                }

                /* check if recomputation is needed */
                if z == 0. {
                    let mut j = 0;
                    for i in (jk..=jz - 1).rev() {
                        j |= i!(iq, i);
                    }
                    if j == 0 {
                        /* need recomputation */
                        let mut k = 1;
                        while i!(iq, jk - k, ==, 0) {
                            k += 1; /* k = no. of terms needed */
                        }

                        for i in (jz + 1)..=(jz + k) {
                            /* add q[jz+1] to q[jz+k] */
                            i!(f, jx + i, =, i!(IPIO2, jv + i) as f64);
                            fw = 0f64;
                            for j in 0..=jx {
                                fw += i!(x, j) * i!(f, jx + i - j);
                            }
                            i!(q, i, =, fw);
                        }
                        jz += k;
                        continue 'recompute;
                    }
                }

                break;
            }

            /* chop off zero terms */
            if z == 0. {
                jz -= 1;
                q0 -= 24;
                while i!(iq, jz) == 0 {
                    jz -= 1;
                    q0 -= 24;
                }
            } else {
                /* break z into 24-bit if necessary */
                z = scalbn(z, -q0);
                if z >= x1p24 {
                    fw = (x1p_24 * z) as i32 as f64;
                    i!(iq, jz, =, (z - x1p24 * fw) as i32);
                    jz += 1;
                    q0 += 24;
                    i!(iq, jz, =, fw as i32);
                } else {
                    i!(iq, jz, =, z as i32);
                }
            }

            /* convert integer "bit" chunk to floating-point value */
            fw = scalbn(1., q0);
            for i in (0..=jz).rev() {
                i!(q, i, =, fw * (i!(iq, i) as f64));
                fw *= x1p_24;
            }

            /* compute PIo2[0,...,jp]*q[jz,...,0] */
            for i in (0..=jz).rev() {
                fw = 0f64;
                let mut k = 0;
                while (k <= jp) && (k <= jz - i) {
                    fw += i!(PIO2, k) * i!(q, i + k);
                    k += 1;
                }
                i!(fq, jz - i, =, fw);
            }

            /* compress fq[] into y[] */
            match prec {
                0 => {
                    fw = 0f64;
                    for i in (0..=jz).rev() {
                        fw += i!(fq, i);
                    }
                    i!(y, 0, =, if ih == 0 { fw } else { -fw });
                }
                1 | 2 => {
                    fw = 0f64;
                    for i in (0..=jz).rev() {
                        fw += i!(fq, i);
                    }
                    i!(y, 0, =, if ih == 0 { fw } else { -fw });
                    fw = i!(fq, 0) - fw;
                    for i in 1..=jz {
                        fw += i!(fq, i);
                    }
                    i!(y, 1, =, if ih == 0 { fw } else { -fw });
                }
                3 => {
                    /* painful */
                    for i in (1..=jz).rev() {
                        fw = i!(fq, i - 1) + i!(fq, i);
                        i!(fq, i, +=, i!(fq, i - 1) - fw);
                        i!(fq, i - 1, =, fw);
                    }
                    for i in (2..=jz).rev() {
                        fw = i!(fq, i - 1) + i!(fq, i);
                        i!(fq, i, +=, i!(fq, i - 1) - fw);
                        i!(fq, i - 1, =, fw);
                    }
                    fw = 0f64;
                    for i in (2..=jz).rev() {
                        fw += i!(fq, i);
                    }
                    if ih == 0 {
                        i!(y, 0, =, i!(fq, 0));
                        i!(y, 1, =, i!(fq, 1));
                        i!(y, 2, =, fw);
                    } else {
                        i!(y, 0, =, -i!(fq, 0));
                        i!(y, 1, =, -i!(fq, 1));
                        i!(y, 2, =, -fw);
                    }
                }
                #[cfg(debug_assertions)]
                _ => unreachable!(),
                #[cfg(not(debug_assertions))]
                _ => {}
            }
            n & 7
        }
    }
}
