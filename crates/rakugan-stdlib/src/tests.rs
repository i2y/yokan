// What perl prints, printed by perl. The table these are held to grows
// in the phase that brings the rest of the twins; these are the rows
// the demos that exist today reach.
use super::*;

#[test]
fn perl_prints_a_number_with_fifteen_digits() {
    assert_eq!(num_text(0.1 + 0.2), "0.3");
    assert_eq!(num_text(1.0 / 3.0), "0.333333333333333");
    assert_eq!(num_text(3.0), "3");
    assert_eq!(num_text(1e21), "1e+21");
    assert_eq!(num_text(-0.0), "0");
    assert_eq!(num_text(0.25), "0.25");
    assert_eq!(num_text(1234567.0), "1234567");
    assert_eq!(num_text(0.00001), "1e-05");
    assert_eq!(num_text(0.0001), "0.0001");
}

#[test]
fn perl_prints_a_bool_as_one_or_as_nothing() {
    assert_eq!(bool_text(true), "1");
    assert_eq!(bool_text(false), "");
}

#[test]
fn the_remainder_takes_the_sign_of_the_right_side() {
    assert_eq!(mod_int(-7, 3), 2);
    assert_eq!(mod_int(7, -3), -2);
    assert_eq!(mod_int(7, 3), 1);
    assert_eq!(mod_int(-7, -3), -1);
}

#[test]
fn dividing_two_whole_numbers_answers_a_fraction() {
    assert_eq!(num_text(div_int(7, 2)), "3.5");
    assert_eq!(num_text(div_int(4, 2)), "2");
}

#[test]
fn a_string_read_as_a_number_stops_where_the_number_does() {
    assert_eq!(num_of("10"), 10.0);
    assert_eq!(num_of("3abc"), 3.0);
    assert_eq!(num_of("abc"), 0.0);
    assert_eq!(num_of("  12  "), 12.0);
    assert_eq!(num_of("-2.5"), -2.5);
    assert_eq!(num_of("1e3"), 1000.0);
    assert_eq!(num_of("1e"), 1.0);
    assert_eq!(num_of(""), 0.0);
    assert_eq!(num_of("0."), 0.0);
    assert_eq!(num_of(".5"), 0.5);
}

#[test]
fn int_throws_the_fraction_away_towards_zero() {
    assert_eq!(int_of(3.7), 3);
    assert_eq!(int_of(-3.7), -3);
}

#[test]
fn sprintf_writes_what_perl_writes() {
    assert_eq!(fmt_num("%.1f", 5.0), "5.0");
    assert_eq!(fmt_num("%.2f", 0.255), "0.26");
    assert_eq!(fmt_num("total %.2f", 12.345), "total 12.35");
    assert_eq!(fmt_num("%.0f", 2.5), "2");
    assert_eq!(fmt_num("%6.2f", 1.5), "  1.50");
    assert_eq!(fmt_num("%-6.2f|", 1.5), "1.50  |");
    assert_eq!(fmt_int("%d items", 4), "4 items");
    assert_eq!(fmt_int("%04d", 7), "0007");
    assert_eq!(fmt_int("%x", 255), "ff");
    assert_eq!(fmt_str("[%s]", "hi"), "[hi]");
    assert_eq!(fmt_str("%10s|", "hi"), "        hi|");
    assert_eq!(fmt_int("100%% of %d", 3), "100% of 3");
}
