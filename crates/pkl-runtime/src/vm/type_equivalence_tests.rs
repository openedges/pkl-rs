//! Pinned Java TypeNode predicate controls, separate from language API results.
use super::TypeCheck;
use crate::{source::Span, value::PklString};

fn recipe(name: &str) -> TypeCheck {
    let (kind, values) = name.split_once('_').unwrap();
    let (literals, default): (&[&[u16]], _) = match values {
        "ab" => (&[&[97], &[98]], None),
        "ba" => (&[&[98], &[97]], None),
        "dup" => (&[&[97], &[98], &[97]], None),
        "d0" => (&[&[97], &[98]], Some(0)),
        "d1" => (&[&[97], &[98]], Some(1)),
        "ac" => (&[&[97], &[99]], None),
        "hl" => (&[&[0xd83d], &[0xdc00]], None),
        "lh" => (&[&[0xdc00], &[0xd83d]], None),
        _ => panic!("unknown original Java recipe"),
    };
    TypeCheck::Union {
        string_literals: kind == "s",
        types: literals
            .iter()
            .map(|units| TypeCheck::StringLiteral(PklString::from_utf16(units.to_vec())))
            .collect(),
        spans: vec![Span::new(0, 9); literals.len()],
        labels: vec![Span::new(0, 9); literals.len()],
        default,
    }
}

#[test]
fn literal_and_general_union_equivalence_matches_actual_java_predicate() {
    // Recipes and pairs are the unchanged Java helper inputs. Expectations are
    // its complete stdout from two fresh JVMs, not Rust-derived booleans.
    let pairs = [
        ("optimized-reordered", "s_ab", "s_ba"),
        ("optimized-duplicates", "s_ab", "s_dup"),
        ("optimized-duplicates-reverse", "s_dup", "s_ab"),
        ("optimized-no-vs-default", "s_ab", "s_d0"),
        ("optimized-different-defaults", "s_d0", "s_d1"),
        ("optimized-vs-general", "s_ab", "g_ab"),
        ("general-vs-optimized", "g_ab", "s_ab"),
        ("general-reordered", "g_ab", "g_ba"),
        ("general-duplicates", "g_ab", "g_dup"),
        ("general-no-vs-default", "g_ab", "g_d0"),
        ("general-different-defaults", "g_d0", "g_d1"),
        ("optimized-different-set", "s_ab", "s_ac"),
        ("general-different-child", "g_ab", "g_ac"),
        ("optimized-unpaired-reordered", "s_hl", "s_lh"),
        ("unpaired-optimized-vs-general", "s_hl", "g_hl"),
        ("unpaired-general-vs-optimized", "g_hl", "s_hl"),
    ];
    let mut actual = String::new();
    for (id, left, right) in pairs {
        actual.push_str(&format!(
            "{id}\t{}\n",
            recipe(left).equivalent(&recipe(right))
        ));
    }
    for expected in [
        include_bytes!(
            "../../../../tests/fixtures/m2-literal-union-predicate/repeats/0/stdout.bin"
        )
        .as_slice(),
        include_bytes!(
            "../../../../tests/fixtures/m2-literal-union-predicate/repeats/1/stdout.bin"
        )
        .as_slice(),
    ] {
        assert_eq!(actual.as_bytes(), expected);
    }
}
