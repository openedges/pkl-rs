//! Pinned ValueFormatter string literals, preserving all UTF-16 code units.
use super::PklString;

pub(super) fn string(value: &PklString, indent: &PklString, custom: bool) -> PklString {
    let units: Vec<_> = value.encode_utf16().collect();
    let multiline = units.contains(&10);
    let (mut quotes, mut context, mut current, mut maxima) =
        (0usize, 0usize, [0usize; 4], [0usize; 4]);
    for &unit in &units {
        match unit {
            92 => {
                context = 3;
                current[3] = 1;
                maxima[3] = maxima[3].max(1);
            }
            34 => {
                quotes += 1;
                context = if quotes < 3 { 1 } else { 2 };
                current[context] = 1;
                maxima[context] = maxima[context].max(1);
            }
            35 => {
                quotes = 0;
                if context != 0 {
                    current[context] += 1;
                    maxima[context] = maxima[context].max(current[context]);
                }
            }
            _ => {
                quotes = 0;
                context = 0;
            }
        }
    }
    let count = if custom {
        maxima[3].max(maxima[if multiline { 2 } else { 1 }])
    } else {
        0
    };
    let mut out = Vec::new();
    let pound = vec![35; count];
    let escape: Vec<_> = std::iter::once(92).chain(pound.iter().copied()).collect();
    out.extend(&pound);
    if multiline {
        out.extend([34, 34, 34, 10]);
        out.extend(indent.encode_utf16());
        let mut quotes = 0;
        for unit in units {
            match unit {
                10 => {
                    out.push(10);
                    out.extend(indent.encode_utf16());
                    quotes = 0;
                }
                13 | 9 => {
                    out.extend(&escape);
                    out.push(if unit == 13 { 114 } else { 116 });
                    quotes = 0;
                }
                92 => {
                    out.push(92);
                    if !custom {
                        out.push(92);
                    }
                    quotes = 0;
                }
                34 => {
                    if quotes == 2 && !custom {
                        out.push(92);
                        quotes = 0;
                    } else {
                        quotes += 1;
                    }
                    out.push(34);
                }
                _ => {
                    out.push(unit);
                    quotes = 0;
                }
            }
        }
        out.push(10);
        out.extend(indent.encode_utf16());
        out.extend([34, 34, 34]);
    } else {
        out.push(34);
        let mut start = 0;
        if custom && units == [34] {
            out.extend(&escape);
            out.push(34);
            start = 1;
        } else if custom && units.starts_with(&[34, 34]) {
            out.push(34);
            out.extend(&escape);
            out.push(34);
            start = 2;
        }
        for &unit in &units[start..] {
            match unit {
                10 | 13 | 9 => {
                    out.extend(&escape);
                    out.push(match unit {
                        10 => 110,
                        13 => 114,
                        _ => 116,
                    });
                }
                92 | 34 if !custom => out.extend([92, unit]),
                _ => out.push(unit),
            }
        }
        out.push(34);
    }
    out.extend(pound);
    PklString::from_utf16(out)
}
