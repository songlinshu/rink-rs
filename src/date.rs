use unit_defs::{DatePattern, Token};
use chrono::Weekday;
use chrono::format::parsed::Parsed;
use std::iter::Peekable;

pub fn parse_date<I>(
    out: &mut Parsed,
    date: &mut Peekable<I>,
    pat: &[DatePattern]
) -> Result<(), String>
    where I: Iterator<Item=Token>+Clone {

    let tok = date.peek().cloned();

    macro_rules! numeric_match {
        ($name:expr, $digits:expr, $field:ident) => {
            match tok {
                Some(Token::Number(ref s, None, None)) if $digits == 0 || s.len() == $digits => {
                    let value = i32::from_str_radix(&**s, 10).unwrap();
                    out.$field = Some(value as _);
                    Ok(())
                },
                x => Err(format!("Expected {}-digit {}, got {:?}", $digits, $name, x))
            }
        }
    }

    let res = match pat.first() {
        None if tok.is_some() => Err(format!("Expected EOF, got {:?}", tok.unwrap())),
        None => return Ok(()),
        Some(&DatePattern::Literal(ref l)) => match tok {
            Some(Token::Ident(ref s)) if s == l => Ok(()),
            x => Err(format!("Expected `{}`, got {:?}", l, x)),
        },
        Some(&DatePattern::Match(ref what)) => match &**what {
            "fullyear"  => numeric_match!("fullyear",  4, year),
            "shortyear" => numeric_match!("shortyear", 2, year_mod_100),
            "century"   => numeric_match!("century",   2, year_div_100),
            "monthnum"  => numeric_match!("monthnum",  2, month),
            "day"       => numeric_match!("day",       2, day),
            "hour12"    => numeric_match!("hour12",    2, hour_mod_12),
            "min"       => numeric_match!("min",       2, minute),
            "ordinal"   => numeric_match!("ordinal",   3, ordinal),
            "isoyear"   => numeric_match!("isoyear",   4, isoyear),
            "isoweek"   => numeric_match!("isoweek",   2, isoweek),
            "unix"      => numeric_match!("unix",      0, timestamp),
            "hour24" => match tok {
                Some(Token::Number(ref s, None, None)) if s.len() == 2 => {
                    let value = u32::from_str_radix(&**s, 10).unwrap();
                    out.hour_div_12 = Some(value / 12);
                    out.hour_mod_12 = Some(value % 12);
                    Ok(())
                },
                x => Err(format!("Expected 2-digit hour24, got {:?}", x))
            },
            "sec" => match tok {
                Some(Token::Number(ref s, None, None)) if s.len() == 2 => {
                    let value = u32::from_str_radix(&**s, 10).unwrap();
                    out.second = Some(value);
                    Ok(())
                },
                Some(Token::Number(ref s, Some(ref f), None)) if s.len() == 2 => {
                    let secs = u32::from_str_radix(&**s, 10).unwrap();
                    let nsecs = u32::from_str_radix(&**f, 10).unwrap() * 10u32.pow(9 - f.len() as u32);
                    out.second = Some(secs);
                    out.nanosecond = Some(nsecs);
                    Ok(())
                },
                x => Err(format!("Expected 2-digit sec, got {:?}", x))
            },
            "offset" => {
                macro_rules! take {
                    ($($pat: pat)|+) => {
                        match date.peek().cloned() {
                            $(Some($pat))|+ => date.next().unwrap(),
                            x => return Err(format!("Expected {}, got {:?}", stringify!($($pat)|+), x))
                        }
                    };
                    ($pat:pat, $var:ident) => {
                        match date.peek().cloned() {
                            Some($pat) => {date.next(); $var},
                            x => return Err(format!("Expected {}, got {:?}", stringify!($pat), x))
                        }
                    }
                }
                let s = match take!(Token::Plus | Token::Minus) {
                    Token::Plus => 1, Token::Minus => -1, _ => panic!()
                };
                let h = take!(Token::Number(s, None, None), s);
                let h = i32::from_str_radix(&*h, 10).unwrap();
                take!(Token::Colon);
                let m = take!(Token::Number(s, None, None), s);
                let m = i32::from_str_radix(&*m, 10).unwrap();
                out.offset = Some(s * (h*3600 + m*60));
                Ok(())
            },
            "monthname" => match tok {
                Some(Token::Ident(ref s)) => {
                    let res = match &*s.to_lowercase() {
                        "jan" | "january" => 1,
                        "feb" | "february" => 2,
                        "mar" | "march" => 3,
                        "apr" | "april" => 4,
                        "may" => 5,
                        "jun" | "june" => 6,
                        "jul" | "july" => 7,
                        "aug" | "august" => 8,
                        "sep" | "september" => 9,
                        "oct" | "october" => 10,
                        "nov" | "november" => 11,
                        "dec" | "december" => 12,
                        x => return Err(format!("Unknown month name: {}", x))
                    };
                    out.month = Some(res);
                    Ok(())
                },
                x => Err(format!("Expected month name, got {:?}", x))
            },
            "weekday" => match tok {
                Some(Token::Ident(ref s)) => {
                    let res = match &*s.to_lowercase() {
                        "mon" | "monday" => Weekday::Mon,
                        "tue" | "tuesday" => Weekday::Tue,
                        "wed" | "wednesday" => Weekday::Wed,
                        "thu" | "thursday" => Weekday::Thu,
                        "fri" | "friday" => Weekday::Fri,
                        "sat" | "saturday" => Weekday::Sat,
                        "sun" | "sunday" => Weekday::Sun,
                        x => return Err(format!("Unknown weekday: {}", x))
                    };
                    out.weekday = Some(res);
                    Ok(())
                },
                x => Err(format!("Expected weekday, got {:?}", x))
            },
            x => Err(format!("Unknown match pattern `{}`", x))
        },
        Some(&DatePattern::Optional(ref pats)) => {
            let mut iter = date.clone();
            match parse_date(out, &mut iter, &pats[..]) {
                Ok(()) => *date = iter,
                Err(_) => ()
            };
            Ok(())
        }
        Some(&DatePattern::Dash) => match tok {
            Some(Token::Minus) => Ok(()),
            x => Err(format!("Expected `-`, got {:?}", x))
        },
        Some(&DatePattern::Colon) => match tok {
            Some(Token::Colon) => Ok(()),
            x => Err(format!("Expected `:`, got {:?}", x))
        },
        Some(&DatePattern::Error(ref err)) => Err(err.clone())
    };
    date.next();
    match res {
        Ok(()) => parse_date(out, date, &pat[1..]),
        Err(e) => Err(e)
    }
}
