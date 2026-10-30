//! fdlibm inverse-trigonometric and log10 operation order, with explicit words.
// Copyright (C) 1993 by Sun Microsystems, Inc. All rights reserved.
// Developed at SunSoft, a Sun Microsystems, Inc. business.
// Permission to use, copy, modify, and distribute this software is freely
// granted, provided that this notice is preserved.
// Original e_asin.c, e_acos.c, s_atan.c, e_atan2.c, e_log.c and e_log10.c are retained
// with their exact hashes in the Stage126 source preparation provenance.
#![allow(clippy::eq_op)] // Preserve fdlibm's IEEE NaN-producing arithmetic.

const PI: f64 = f64::from_bits(0x4009_21fb_5444_2d18);
const PIO2_HI: f64 = f64::from_bits(0x3ff9_21fb_5444_2d18);
const PIO2_LO: f64 = f64::from_bits(0x3c91_a626_3314_5c07);
const PIO4_HI: f64 = f64::from_bits(0x3fe9_21fb_5444_2d18);
const PI_LO: f64 = f64::from_bits(0x3ca1_a626_3314_5c07);

fn ratio(value: f64) -> f64 {
    let p = value
        * (f64::from_bits(0x3fc5_5555_5555_5555)
            + value
                * (f64::from_bits(0xbfd4_d612_03eb_6f7d)
                    + value
                        * (f64::from_bits(0x3fc9_c155_0e88_4455)
                            + value
                                * (f64::from_bits(0xbfa4_8228_b568_8f3b)
                                    + value
                                        * (f64::from_bits(0x3f49_efe0_7501_b288)
                                            + value * f64::from_bits(0x3f02_3de1_0dfd_f709))))));
    let q = 1.0
        + value
            * (f64::from_bits(0xc003_3a27_1c8a_2d4b)
                + value
                    * (f64::from_bits(0x4000_2ae5_9c59_8ac8)
                        + value
                            * (f64::from_bits(0xbfe6_066c_1b8d_0159)
                                + value * f64::from_bits(0x3fb3_b8c5_b12e_9282))));
    p / q
}

pub(super) fn asin(value: f64) -> f64 {
    let high = (value.to_bits() >> 32) as u32;
    let absolute = high & 0x7fff_ffff;
    if absolute >= 0x3ff0_0000 {
        if ((absolute - 0x3ff0_0000) | value.to_bits() as u32) == 0 {
            return value * PIO2_HI + value * PIO2_LO;
        }
        return (value - value) / (value - value);
    }
    if absolute < 0x3fe0_0000 {
        if absolute < 0x3e40_0000 {
            return value;
        }
        return value + value * ratio(value * value);
    }
    let square = (1.0 - value.abs()) * 0.5;
    let root = libm::sqrt(square);
    let result = if absolute >= 0x3fef_3333 {
        PIO2_HI - (2.0 * (root + root * ratio(square)) - PIO2_LO)
    } else {
        let high_root = f64::from_bits(root.to_bits() & 0xffff_ffff_0000_0000);
        let correction = (square - high_root * high_root) / (root + high_root);
        let p = 2.0 * root * ratio(square) - (PIO2_LO - 2.0 * correction);
        let q = PIO4_HI - 2.0 * high_root;
        PIO4_HI - (p - q)
    };
    if high >> 31 == 0 { result } else { -result }
}

pub(super) fn acos(value: f64) -> f64 {
    let high = (value.to_bits() >> 32) as u32;
    let absolute = high & 0x7fff_ffff;
    if absolute >= 0x3ff0_0000 {
        if ((absolute - 0x3ff0_0000) | value.to_bits() as u32) == 0 {
            return if high >> 31 == 0 {
                0.0
            } else {
                PI + 2.0 * PIO2_LO
            };
        }
        return (value - value) / (value - value);
    }
    if absolute < 0x3fe0_0000 {
        if absolute <= 0x3c60_0000 {
            return PIO2_HI + PIO2_LO;
        }
        return PIO2_HI - (value - (PIO2_LO - value * ratio(value * value)));
    }
    if high >> 31 != 0 {
        let square = (1.0 + value) * 0.5;
        let root = libm::sqrt(square);
        let correction = ratio(square) * root - PIO2_LO;
        return PI - 2.0 * (root + correction);
    }
    let square = (1.0 - value) * 0.5;
    let root = libm::sqrt(square);
    let high_root = f64::from_bits(root.to_bits() & 0xffff_ffff_0000_0000);
    let correction = (square - high_root * high_root) / (root + high_root);
    let result = ratio(square) * root + correction;
    2.0 * (high_root + result)
}

pub(super) fn atan(mut value: f64) -> f64 {
    const HIGH: [f64; 4] = [
        f64::from_bits(0x3fdd_ac67_0561_bb4f),
        f64::from_bits(0x3fe9_21fb_5444_2d18),
        f64::from_bits(0x3fef_730b_d281_f69b),
        PIO2_HI,
    ];
    const LOW: [f64; 4] = [
        f64::from_bits(0x3c7a_2b7f_222f_65e2),
        f64::from_bits(0x3c81_a626_3314_5c07),
        f64::from_bits(0x3c70_0788_7af0_cbbd),
        PIO2_LO,
    ];
    const A: [f64; 11] = [
        f64::from_bits(0x3fd5_5555_5555_550d),
        f64::from_bits(0xbfc9_9999_9998_ebc4),
        f64::from_bits(0x3fc2_4924_9200_83ff),
        f64::from_bits(0xbfbc_71c6_fe23_1671),
        f64::from_bits(0x3fb7_45cd_c54c_206e),
        f64::from_bits(0xbfb3_b0f2_af74_9a6d),
        f64::from_bits(0x3fb1_0d66_a0d0_3d51),
        f64::from_bits(0xbfad_de2d_52de_fd9a),
        f64::from_bits(0x3fa9_7b4b_2476_0deb),
        f64::from_bits(0xbfa2_b444_2c6a_6c2f),
        f64::from_bits(0x3f90_ad3a_e322_da11),
    ];
    let negative = value.is_sign_negative();
    let high = (value.to_bits() >> 32) as u32 & 0x7fff_ffff;
    if high >= 0x4410_0000 {
        if value.is_nan() {
            return value + value;
        }
        return if negative {
            -HIGH[3] - LOW[3]
        } else {
            HIGH[3] + LOW[3]
        };
    }
    let index = if high < 0x3fdc_0000 {
        if high < 0x3e20_0000 {
            return value;
        }
        None
    } else {
        value = value.abs();
        Some(if high < 0x3ff3_0000 {
            if high < 0x3fe6_0000 {
                value = (2.0 * value - 1.0) / (2.0 + value);
                0
            } else {
                value = (value - 1.0) / (value + 1.0);
                1
            }
        } else if high < 0x4003_8000 {
            value = (value - 1.5) / (1.0 + 1.5 * value);
            2
        } else {
            value = -1.0 / value;
            3
        })
    };
    let square = value * value;
    let fourth = square * square;
    let odd = square
        * (A[0]
            + fourth
                * (A[2] + fourth * (A[4] + fourth * (A[6] + fourth * (A[8] + fourth * A[10])))));
    let even =
        fourth * (A[1] + fourth * (A[3] + fourth * (A[5] + fourth * (A[7] + fourth * A[9]))));
    if let Some(index) = index {
        let result = HIGH[index] - ((value * (odd + even) - LOW[index]) - value);
        if negative { -result } else { result }
    } else {
        value - value * (odd + even)
    }
}

pub(super) fn atan2(y: f64, x: f64) -> f64 {
    const TINY: f64 = 1.0e-300;
    let hx = (x.to_bits() >> 32) as u32;
    let hy = (y.to_bits() >> 32) as u32;
    let ix = hx & 0x7fff_ffff;
    let iy = hy & 0x7fff_ffff;
    if x.is_nan() || y.is_nan() {
        return x + y;
    }
    if x.to_bits() == 1.0f64.to_bits() {
        return atan(y);
    }
    let quadrant = ((hy >> 31) & 1) | ((hx >> 30) & 2);
    if y == 0.0 {
        return match quadrant {
            0 | 1 => y,
            2 => PI + TINY,
            _ => -PI - TINY,
        };
    }
    if x == 0.0 {
        return if y.is_sign_negative() {
            -PIO2_HI - TINY
        } else {
            PIO2_HI + TINY
        };
    }
    if x.is_infinite() {
        return if y.is_infinite() {
            match quadrant {
                0 => PIO4_HI + TINY,
                1 => -PIO4_HI - TINY,
                2 => 3.0 * PIO4_HI + TINY,
                _ => -3.0 * PIO4_HI - TINY,
            }
        } else {
            match quadrant {
                0 => 0.0,
                1 => -0.0,
                2 => PI + TINY,
                _ => -PI - TINY,
            }
        };
    }
    if y.is_infinite() {
        return if y.is_sign_negative() {
            -PIO2_HI - TINY
        } else {
            PIO2_HI + TINY
        };
    }
    let difference = (iy as i32 - ix as i32) >> 20;
    let angle = if difference > 60 {
        PIO2_HI + 0.5 * PI_LO
    } else if x.is_sign_negative() && difference < -60 {
        0.0
    } else {
        atan((y / x).abs())
    };
    match quadrant {
        0 => angle,
        1 => -angle,
        2 => PI - (angle - PI_LO),
        _ => (angle - PI_LO) - PI,
    }
}

pub(super) fn log10(mut value: f64) -> f64 {
    const TWO54: f64 = f64::from_bits(0x4350_0000_0000_0000);
    const IVLN10: f64 = f64::from_bits(0x3fdb_cb7b_1526_e50e);
    const LOG10_2HI: f64 = f64::from_bits(0x3fd3_4413_509f_6000);
    const LOG10_2LO: f64 = f64::from_bits(0x3d59_fef3_11f1_2b36);
    let mut high = (value.to_bits() >> 32) as i32;
    let mut exponent = 0;
    if high < 0x0010_0000 {
        if value == 0.0 {
            return f64::NEG_INFINITY;
        }
        if high < 0 {
            return (value - value) / 0.0;
        }
        exponent -= 54;
        value *= TWO54;
        high = (value.to_bits() >> 32) as i32;
    }
    if high >= 0x7ff0_0000 {
        return value + value;
    }
    exponent += (high >> 20) - 1023;
    let adjust = ((exponent as u32) >> 31) as i32;
    high = (high & 0x000f_ffff) | ((0x3ff - adjust) << 20);
    let power = (exponent + adjust) as f64;
    value = f64::from_bits(((high as u32 as u64) << 32) | (value.to_bits() & 0xffff_ffff));
    let result = power * LOG10_2LO + IVLN10 * log(value);
    result + power * LOG10_2HI
}

// Preserve e_log.c's separate near-one and cancellation branches. The existing
// shared M1 numeric kernel is deliberately unchanged by this module binding.
pub(super) fn log(mut value: f64) -> f64 {
    const LN2_HI: f64 = f64::from_bits(0x3fe6_2e42_fee0_0000);
    const LN2_LO: f64 = f64::from_bits(0x3dea_39ef_3579_3c76);
    const TWO54: f64 = f64::from_bits(0x4350_0000_0000_0000);
    const LG1: f64 = f64::from_bits(0x3fe5_5555_5555_5593);
    const LG2: f64 = f64::from_bits(0x3fd9_9999_9997_fa04);
    const LG3: f64 = f64::from_bits(0x3fd2_4924_9422_9359);
    const LG4: f64 = f64::from_bits(0x3fcc_71c5_1d8e_78af);
    const LG5: f64 = f64::from_bits(0x3fc7_4664_96cb_03de);
    const LG6: f64 = f64::from_bits(0x3fc3_9a09_d078_c69f);
    const LG7: f64 = f64::from_bits(0x3fc2_f112_df3e_5244);
    let mut high = (value.to_bits() >> 32) as i32;
    let mut exponent = 0;
    if high < 0x0010_0000 {
        if value == 0.0 {
            return f64::NEG_INFINITY;
        }
        if high < 0 {
            return (value - value) / 0.0;
        }
        exponent -= 54;
        value *= TWO54;
        high = (value.to_bits() >> 32) as i32;
    }
    if high >= 0x7ff0_0000 {
        return value + value;
    }
    exponent += (high >> 20) - 1023;
    let fraction = high & 0x000f_ffff;
    let adjust = (fraction + 0x0009_5f64) & 0x0010_0000;
    let normalized = fraction | (adjust ^ 0x3ff0_0000);
    value = f64::from_bits(((normalized as u32 as u64) << 32) | (value.to_bits() & 0xffff_ffff));
    exponent += adjust >> 20;
    let offset = value - 1.0;
    let power = exponent as f64;
    if (0x000f_ffff & (2 + fraction)) < 3 {
        if offset == 0.0 {
            return if exponent == 0 {
                0.0
            } else {
                power * LN2_HI + power * LN2_LO
            };
        }
        let remainder = offset * offset * (0.5 - f64::from_bits(0x3fd5_5555_5555_5555) * offset);
        return if exponent == 0 {
            offset - remainder
        } else {
            power * LN2_HI - ((remainder - power * LN2_LO) - offset)
        };
    }
    let ratio = offset / (2.0 + offset);
    let square = ratio * ratio;
    let fourth = square * square;
    let even = fourth * (LG2 + fourth * (LG4 + fourth * LG6));
    let odd = square * (LG1 + fourth * (LG3 + fourth * (LG5 + fourth * LG7)));
    let remainder = odd + even;
    let branch = (fraction - 0x0006_147a) | (0x0006_b851 - fraction);
    if branch > 0 {
        let half_square = 0.5 * offset * offset;
        if exponent == 0 {
            offset - (half_square - ratio * (half_square + remainder))
        } else {
            power * LN2_HI
                - ((half_square - (ratio * (half_square + remainder) + power * LN2_LO)) - offset)
        }
    } else if exponent == 0 {
        offset - ratio * (offset - remainder)
    } else {
        power * LN2_HI - ((ratio * (offset - remainder) - power * LN2_LO) - offset)
    }
}
