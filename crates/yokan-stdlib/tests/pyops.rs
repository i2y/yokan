//! The Py-semantics functions against ground truth CPython printed
//! (tests/pyops_expected.txt — hex-packed doubles, exact strings).

use yokan_stdlib::*;

fn hex_f64(h: &str) -> f64 {
    f64::from_bits(u64::from_str_radix(h, 16).unwrap())
}

#[test]
fn matches_cpython_ground_truth() {
    let src = include_str!("pyops_expected.txt");
    for line in src.lines() {
        let p: Vec<&str> = line.split(' ').collect();
        match p[0] {
            "R" => assert_eq!(py_float_repr(hex_f64(p[1])), p[2], "repr of {}", p[1]),
            "D" => {
                let (a, b): (i64, i64) = (p[1].parse().unwrap(), p[2].parse().unwrap());
                let want = hex_f64(p[3]);
                let got = py_truediv_int(a, b);
                assert!(got == want || (got.is_nan() && want.is_nan()), "{a}/{b}: got {got:e} want {want:e}");
            }
            "F" => {
                let (a, b): (i64, i64) = (p[1].parse().unwrap(), p[2].parse().unwrap());
                assert_eq!(py_floordiv_int(a, b), p[3].parse::<i64>().unwrap(), "{a}//{b}");
            }
            "M" => {
                let (a, b): (i64, i64) = (p[1].parse().unwrap(), p[2].parse().unwrap());
                assert_eq!(py_mod_int(a, b), p[3].parse::<i64>().unwrap(), "{a}%{b}");
            }
            "FM" => {
                let (a, b) = (hex_f64(p[1]), hex_f64(p[2]));
                assert_eq!(py_mod_float(a, b).to_bits(), hex_f64(p[3]).to_bits(), "{a}%{b}");
                assert_eq!(py_floordiv_float(a, b).to_bits(), hex_f64(p[4]).to_bits(), "{a}//{b}");
            }
            "P" => {
                let (a, b) = (hex_f64(p[1]), hex_f64(p[2]));
                assert_eq!(py_pow_float(a, b).to_bits(), hex_f64(p[3]).to_bits(), "{a}**{b}");
            }
            "IP" => {
                let (a, b): (i64, i64) = (p[1].parse().unwrap(), p[2].parse().unwrap());
                assert_eq!(py_pow_int(a, b), p[3].parse::<i64>().unwrap(), "{a}**{b}");
            }
            other => panic!("unknown row {other}"),
        }
    }
}
