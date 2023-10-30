use std::net::Ipv4Addr;

use nom::{
    branch::alt,
    bytes,
    character::{
        complete::{self, newline, space1},
        streaming,
    },
    combinator::{eof, map_res},
    multi::{self, many1},
    IResult,
};

use crate::model::MacAddr;

pub mod hosts;
pub mod leases;

fn val_string(input: &str) -> IResult<&str, String> {
    let (input, _) = complete::char('"')(input)?;
    let (input, s) = many1(alt((str_octal_escape, str_char_escape, str_literal)))(input)?;
    let (input, _) = complete::char('"')(input)?;
    Ok((input, s.concat()))
}

fn str_octal_escape(input: &str) -> IResult<&str, String> {
    let (input, _) = complete::char('\\')(input)?;
    let (input, s) = multi::count(complete::one_of("01234567"), 3)(input)?;
    let s = s.iter().collect::<String>();
    let s = u8::from_str_radix(&s, 8).unwrap_or_default();
    let s = std::char::from_u32(u32::from(s)).unwrap_or_default();

    Ok((input, s.to_string()))
}

fn str_char_escape(input: &str) -> IResult<&str, String> {
    let (input, _) = complete::char('\\')(input)?;
    let (input, c) = complete::one_of("abtnvfre\\\"")(input)?;

    let c = match c {
        'a' => Some('\x07'),
        'b' => Some('\x08'),
        't' => Some('\t'),
        'n' => Some('\n'),
        'v' => Some('\x0b'),
        'f' => Some('\x0c'),
        'r' => Some('\r'),
        'e' => Some('\x1b'),
        '\\' => Some('\\'),
        '"' => Some('"'),
        _ => None,
    };

    Ok((input, c.map_or_else(String::new, |c| c.to_string())))
}

fn str_literal(input: &str) -> IResult<&str, String> {
    // not a \ or "
    let (input, s) = bytes::complete::take_while1(|c| c != '\\' && c != '"')(input)?;

    Ok((input, s.to_string()))
}

fn keyword_hardware_ethernet(input: &str) -> IResult<&str, MacAddr> {
    let (input, _) = bytes::complete::tag("hardware")(input)?;
    let (input, _) = space1(input)?;
    let (input, _) = bytes::complete::tag("ethernet")(input)?;
    let (input, _) = space1(input)?;
    let (input, mac) = val_macaddr(input)?;
    Ok((input, mac))
}

static HEX: &str = "0123456789abcdef";

fn val_hexbyte(input: &str) -> IResult<&str, u8> {
    let (input, byte) = multi::count(streaming::one_of(HEX), 2)(input)?;
    let byte = byte.iter().collect::<String>();
    // handle with convert_error
    u8::from_str_radix(&byte, 16).map_or_else(
        |_| {
            Err(nom::Err::Error(nom::error::Error::new(
                input,
                nom::error::ErrorKind::Digit,
            )))
        },
        |byte| Ok((input, byte)),
    )
}

fn val_macaddr(input: &str) -> IResult<&str, MacAddr> {
    let (input, x1) = val_hexbyte(input)?;
    let (input, _) = complete::char(':')(input)?;
    let (input, x2) = val_hexbyte(input)?;
    let (input, _) = complete::char(':')(input)?;
    let (input, x3) = val_hexbyte(input)?;
    let (input, _) = complete::char(':')(input)?;
    let (input, x4) = val_hexbyte(input)?;
    let (input, _) = complete::char(':')(input)?;
    let (input, x5) = val_hexbyte(input)?;
    let (input, _) = complete::char(':')(input)?;
    let (input, x6) = val_hexbyte(input)?;

    let mac = MacAddr::from([x1, x2, x3, x4, x5, x6]);

    Ok((input, mac))
}

fn val_address(input: &str) -> IResult<&str, Ipv4Addr> {
    let (input, a) = complete::digit1(input)?;
    let (input, _) = complete::char('.')(input)?;
    let (input, b) = complete::digit1(input)?;
    let (input, _) = complete::char('.')(input)?;
    let (input, c) = complete::digit1(input)?;
    let (input, _) = complete::char('.')(input)?;
    let (input, d) = complete::digit1(input)?;

    let a = a.parse::<u8>().unwrap_or_default();
    let b = b.parse::<u8>().unwrap_or_default();
    let c = c.parse::<u8>().unwrap_or_default();
    let d = d.parse::<u8>().unwrap_or_default();

    let ip = Ipv4Addr::new(a, b, c, d);

    Ok((input, ip))
}

fn comment(input: &str) -> IResult<&str, &str> {
    let (input, _) = complete::char('#')(input)?;
    let (input, _) = bytes::complete::take_while(|c| c != '\n')(input)?;
    let (input, _) = alt((
        map_res(newline, |_| Ok::<&str, nom::error::Error<&str>>("")),
        map_res(eof, |_| Ok::<&str, nom::error::Error<&str>>("")),
    ))(input)?;

    Ok((input, ""))
}

fn anyspace(input: &str) -> IResult<&str, &str> {
    let (input, _) = complete::one_of(" \t\r\n")(input)?;
    Ok((input, ""))
}

// zero or more spaces or comments

fn anyspace0(input: &str) -> IResult<&str, &str> {
    let (input, _) = multi::many0(alt((anyspace, comment)))(input)?;

    Ok((input, ""))
}

// at least one space or comment
fn anyspace1(input: &str) -> IResult<&str, &str> {
    let (input, _) = multi::many1(alt((anyspace, comment)))(input)?;

    Ok((input, ""))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use nom::Finish;

    use super::*;

    #[test]
    fn test_comment() {
        let example = "# stuff\n";
        let (input, _) = comment(example).unwrap();
        assert_eq!(input, "");
    }

    #[test]
    fn test_anyspace() {
        let example = "# stuff\n";
        let (input, _) = anyspace1(example).unwrap();
        assert_eq!(input, "");

        let example = " # stuff\n";
        let (input, _) = anyspace1(example).unwrap();
        assert_eq!(input, "");
    }

    #[test]
    fn test_anyspace0() {
        let example = "";
        let (input, _) = anyspace0(example).unwrap();
        assert_eq!(input, "");

        let example = " # foobar";
        let (input, _) = anyspace0(example).unwrap();
        assert_eq!(input, "");

        let example = " # foobar\n# test";
        let (input, _) = anyspace0(example).unwrap();
        assert_eq!(input, "");

        let example = " # foobar\n";
        let (input, _) = anyspace0(example).unwrap();
        assert_eq!(input, "");

        let example = " # foobar\n  ";
        let (input, _) = anyspace0(example).unwrap();
        assert_eq!(input, "");

        let example = " # foobar\n#baz\n  ";
        let (input, _) = anyspace0(example).unwrap();
        assert_eq!(input, "");
    }

    #[test]
    fn test_anyspace1() {
        let example = "";
        let _ = anyspace1(example).finish().expect_err("Expected error");

        let example = "# foobar\n";
        let (input, _) = anyspace1(example).unwrap();
        assert_eq!(input, "");

        let example = " # foobar\n  ";
        let (input, _) = anyspace1(example).unwrap();
        assert_eq!(input, "");

        let example = " # foobar\n#baz\n  ";
        let (input, _) = anyspace1(example).unwrap();
        assert_eq!(input, "");
    }
}
