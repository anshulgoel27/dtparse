use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use chrono::NaiveDate;
use std::collections::HashMap;
use std::str;

use parse;
use ParseError;
use Parser;

#[test]
fn test_fuzz() {
    assert_eq!(
        parse("\x2D\x38\x31\x39\x34\x38\x34"),
        Err(ParseError::ImpossibleTimestamp("Invalid month"))
    );

    // Garbage in the third delimited field
    assert_eq!(
        parse("2..\x00\x000d\x00+\x010d\x01\x00\x00\x00+"),
        Err(ParseError::UnrecognizedFormat)
    );

    let default = NaiveDate::from_ymd_opt(2016, 6, 29)
        .unwrap()
        .and_hms_opt(0, 0, 0)
        .unwrap();
    let p = Parser::default();
    let res = p.parse(
        "\x0D\x31",
        None,
        None,
        false,
        false,
        Some(&default),
        false,
        &HashMap::new(),
    );
    assert_eq!(res, Err(ParseError::NoDate));

    assert_eq!(
        parse("\x2D\x2D\x32\x31\x38\x6D"),
        Err(ParseError::ImpossibleTimestamp("Invalid minute"))
    );
}

#[test]
fn large_int() {
    let parse_result = parse("1412409095009.jpg");
    assert!(parse_result.is_err());
}

#[test]
fn another_large_int() {
    let parse_result = parse("1412409095009");
    assert!(parse_result.is_err());
}

#[test]
fn an_even_larger_int() {
    let parse_result = parse("1566997680962280");
    assert!(parse_result.is_err());
}

#[test]
fn empty_string() {
    assert_eq!(parse(""), Err(ParseError::NoDate))
}

#[test]
fn github_33() {
    assert_eq!(
        parse("66:'"),
        Err(ParseError::InvalidNumeric("'".to_owned()))
    )
}

#[test]
fn github_32() {
    assert_eq!(
        parse("99999999999999999999999"),
        Err(ParseError::InvalidNumeric(
            "99999999999999999999999".to_owned()
        ))
    )
}

#[test]
fn large_minute_component_does_not_silently_zero() {
    assert_eq!(
        parse("2024-01-01 12:999999999999999999999999"),
        Err(ParseError::InvalidNumeric(
            "999999999999999999999999".to_owned()
        ))
    );
    assert_eq!(
        parse("2024-01-01 12h 999999999999999999999999m"),
        Err(ParseError::InvalidNumeric(
            "999999999999999999999999".to_owned()
        ))
    );
    assert_eq!(
        parse("2024-01-01 12:999999999999999999999999:30"),
        Err(ParseError::InvalidNumeric(
            "999999999999999999999999".to_owned()
        ))
    );
}

#[test]
fn textual_month_separator_edges_do_not_panic() {
    assert_eq!(parse("Jan-"), Err(ParseError::UnrecognizedFormat));
    assert_eq!(parse("Jan-01-"), Err(ParseError::UnrecognizedFormat));
    assert_eq!(parse("Jan/01/"), Err(ParseError::UnrecognizedFormat));
}

#[test]
fn numeric_trailing_separator_is_rejected() {
    assert_eq!(parse("2000-01-"), Err(ParseError::UnrecognizedFormat));
    assert_eq!(parse("2024/01/"), Err(ParseError::UnrecognizedFormat));
}

#[test]
fn trailing_timezone_sign_is_rejected() {
    assert_eq!(
        parse("2000-01-01 12:00:00+"),
        Err(ParseError::TimezoneUnsupported)
    );
    assert_eq!(
        parse("2000-01-01 12:00:00-"),
        Err(ParseError::TimezoneUnsupported)
    );
}

#[test]
fn recombine_skipped_sorts_before_merging_adjacent_tokens() {
    let parser = Parser::default();
    let tokens = vec![
        "a".to_owned(),
        "b".to_owned(),
        "c".to_owned(),
        "d".to_owned(),
        "e".to_owned(),
        "f".to_owned(),
    ];

    assert_eq!(
        parser.recombine_skipped(vec![3, 1, 2, 5], tokens),
        vec!["bcd".to_owned(), "f".to_owned()]
    );
}

#[test]
fn i32_wraparound_does_not_silently_truncate() {
    // Values that fit in i64 but overflow i32, e.g. 2^32 + 30 = 4294967326
    // Previously these would silently truncate via `as i32` (4294967326 as i32 == 30)

    // HH:MM path — minute wraps to 30
    assert!(parse("2024-01-01 12:4294967326").is_err());

    // HH:MM path — hour wraps to 12
    assert!(parse("2024-01-01 4294967308:30").is_err());

    // ampm path — hour wraps to 12
    assert!(parse("2024-01-01 4294967308 pm").is_err());

    // HH.MMh fractional hour path — 12.4294967326h is a valid decimal
    // (0.4294967326 * 60 ≈ 25.77 minutes), not a wraparound case
    // so this is intentionally not tested here

    // h suffix path — overflow hour silently becomes None
    assert!(parse("2024-01-01 2147483648h").is_err());
    assert!(parse("2024-01-01 2147483648h 30m").is_err());

    // Just over i32::MAX
    assert!(parse("2024-01-01 2147483648:00").is_err());
    assert!(parse("2024-01-01 00:2147483648").is_err());
}

#[test]
fn leap_year_century_rules() {
    // Divisible by 400 — leap year, Feb 29 valid
    assert!(parse("2000-02-29").is_ok());
    // Regular leap year
    assert!(parse("2024-02-29").is_ok());
    // Divisible by 100 but not 400 — not a leap year, explicit day rejects
    assert_eq!(
        parse("1900-02-29"),
        Err(ParseError::ImpossibleTimestamp("Invalid day"))
    );
    // Regular non-leap year — explicit day rejects
    assert_eq!(
        parse("2023-02-29"),
        Err(ParseError::ImpossibleTimestamp("Invalid day"))
    );
    // No explicit day — default day (e.g. 31) should clamp to month max
    // Default is typically Jan 1, but using a custom default with day=31
    // to verify clamp behavior
    let default = NaiveDate::from_ymd_opt(2020, 1, 31)
        .unwrap()
        .and_hms_opt(0, 0, 0)
        .unwrap();
    let p = Parser::default();
    // "2020-02" has no explicit day, inherits day=31 from default, clamps to 29 (leap year)
    let (dt, _, _) = p
        .parse(
            "2020-02",
            None,
            None,
            false,
            false,
            Some(&default),
            false,
            &HashMap::new(),
        )
        .unwrap();
    assert_eq!(format!("{:?}", dt), "2020-02-29T00:00:00");
    // "2023-02" inherits day=31, clamps to 28 (non-leap year)
    let (dt, _, _) = p
        .parse(
            "2023-02",
            None,
            None,
            false,
            false,
            Some(&default),
            false,
            &HashMap::new(),
        )
        .unwrap();
    assert_eq!(format!("{:?}", dt), "2023-02-28T00:00:00");
}

#[test]
fn github_34() {
    let parse_vec = STANDARD.decode("KTMuLjYpGDYvLjZTNiouNjYuHzZpLjY/NkwuNh42Ry42PzYnKTMuNk02NjY2NjA2NjY2NjY2NjYTNjY2Ni82NjY2NlAuNlAuNlNI").unwrap();
    let parse_str = str::from_utf8(&parse_vec).unwrap();
    let parse_result = parse(parse_str);
    assert!(parse_result.is_err());
}

#[test]
fn github_35() {
    let parse_vec = STANDARD.decode("KTY6LjYqNio6KjYn").unwrap();
    let parse_str = str::from_utf8(&parse_vec).unwrap();
    let parse_result = parse(parse_str);
    assert!(parse_result.is_err());
}

#[test]
fn github_36() {
    let parse_vec = STANDARD.decode("KTYuLg==").unwrap();
    let parse_str = str::from_utf8(&parse_vec).unwrap();
    let parse_result = parse(parse_str);
    assert!(parse_result.is_err());
}

#[test]
fn github_46() {
    assert_eq!(
        parse("2000-01-01 12:00:00+00:"),
        Err(ParseError::TimezoneUnsupported)
    );
    assert_eq!(
        parse("2000-01-01 12:00:00+09123"),
        Err(ParseError::TimezoneUnsupported)
    );
    assert_eq!(
        parse("2000-01-01 13:00:00+00:003"),
        Err(ParseError::TimezoneUnsupported)
    );
    assert_eq!(
        parse("2000-01-01 13:00:00+009:03"),
        Err(ParseError::TimezoneUnsupported)
    );
    assert_eq!(
        parse("2000-01-01 13:00:00+xx:03"),
        Err(ParseError::InvalidNumeric(
            "invalid digit found in string".to_owned()
        ))
    );
    assert_eq!(
        parse("2000-01-01 13:00:00+00:yz"),
        Err(ParseError::InvalidNumeric(
            "invalid digit found in string".to_owned()
        ))
    );
    let mut parse_result = parse("2000-01-01 13:00:00+00:03");
    match parse_result {
        Ok((dt, offset)) => {
            assert_eq!(format!("{:?}", dt), "2000-01-01T13:00:00".to_string());
            assert_eq!(format!("{:?}", offset), "Some(+00:03)".to_string());
        }
        Err(_) => {
            panic!();
        }
    };

    parse_result = parse("2000-01-01 12:00:00+0811");
    match parse_result {
        Ok((dt, offset)) => {
            assert_eq!(format!("{:?}", dt), "2000-01-01T12:00:00".to_string());
            assert_eq!(format!("{:?}", offset), "Some(+08:11)".to_string());
        }
        Err(_) => {
            panic!();
        }
    }

    parse_result = parse("2000");
    match parse_result {
        Ok((dt, offset)) => {
            assert_eq!(format!("{:?}", dt), "2000-01-01T00:00:00".to_string());
            assert!(offset.is_none());
        }
        Err(_) => {
            panic!();
        }
    }
}

#[test]
fn github_45() {
    assert!(parse("/2018-fifa-").is_err());
    assert!(parse("/2009/07/").is_err());
    assert!(parse("2021-09-").is_err());
}
