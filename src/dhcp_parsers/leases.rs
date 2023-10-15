use std::net::Ipv4Addr;

use chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime, Utc};
use eyre::{eyre, ContextCompat, Result};
use nom::{
    branch::alt,
    character::complete::{self, space0 },
    combinator::{self, all_consuming},
    multi::many1,
    sequence::{preceded, terminated},
    Finish, IResult, bytes,
};

use crate::model::{Lease, LeaseTime, MacAddr};

use super::{val_string, keyword_hardware_ethernet, val_address, anyspace0, anyspace1};

#[derive(Debug, PartialEq)]
enum LeaseFileItem {
    AuthoringByteOrder(ByteOrder),
    Lease(Ipv4Addr, Vec<LeaseField>),
    ServerDuid(String),
}

#[derive(Debug, PartialEq)]
enum ByteOrder {
    LittleEndian,
    BigEndian,
}

type NaiveLeaseTime = Option<NaiveDateTime>;

#[derive(Debug, PartialEq)]
enum LeaseField {
    Starts(NaiveLeaseTime),
    Ends(NaiveLeaseTime),
    Tstp(NaiveLeaseTime),
    Cltt(NaiveLeaseTime),
    HardwareEthernet(MacAddr),
    ClientHostname(String),
    Uid(String),
    VendorClassIdentifier(String),
    Ignore(String),
}

pub fn parse(input: &str) -> Result<Vec<Lease>> {
    let (_, lease_file_items) = all_consuming(lease_file_items)(input)
        .finish()
        .map_err(|e| eyre!("parse error: {}", e))?;
    let mut leases = Vec::new();

    for item in lease_file_items {
        let mut starts: LeaseTime = None;
        let mut ends: LeaseTime = None;
        let mut tstp: LeaseTime = None;
        let mut cltt: LeaseTime = None;
        let mut hardware_ethernet: Option<MacAddr> = None;
        let mut client_hostname: Option<String> = None;
        if let LeaseFileItem::Lease(address, fields) = item {
            for field in fields {
                match field {
                    LeaseField::Starts(t) => {
                        starts = t.map(|t| DateTime::from_naive_utc_and_offset(t, Utc))
                    }
                    LeaseField::Ends(t) => {
                        ends = t.map(|t| DateTime::from_naive_utc_and_offset(t, Utc))
                    }
                    LeaseField::Tstp(t) => {
                        tstp = t.map(|t| DateTime::from_naive_utc_and_offset(t, Utc))
                    }
                    LeaseField::Cltt(t) => {
                        cltt = t.map(|t| DateTime::from_naive_utc_and_offset(t, Utc))
                    }
                    LeaseField::HardwareEthernet(addr) => hardware_ethernet = Some(addr),
                    LeaseField::ClientHostname(hostname) => client_hostname = Some(hostname),
                    _ => {}
                }
            }

            let lease = Lease {
                address,
                starts,
                ends,
                tstp,
                cltt,
                hardware_ethernet: hardware_ethernet
                    .wrap_err(eyre!("lease missing hardware ethernet"))?,
                client_hostname,
            };
            leases.push(lease)
        }
    }

    Ok(leases)
}

fn lease_file_items(input: &str) -> IResult<&str, Vec<LeaseFileItem>> {
    let (input, items) = many1(preceded(
        anyspace0,
        alt((authoring_byte_order, server_duid, lease)),
    ))(input)?;
    Ok((input, items))
}

fn server_duid(input: &str) -> IResult<&str, LeaseFileItem> {
    let (input, _) = bytes::complete::tag("server-duid")(input)?;
    let (input, _) = anyspace1(input)?;
    let (input, s) = val_string(input)?;
    let (input, _) = anyspace0(input)?;
    let (input, _) = complete::char(';')(input)?;
    let (input, _) = anyspace0(input)?;
    Ok((input, LeaseFileItem::ServerDuid(s)))
}

fn lease(input: &str) -> IResult<&str, LeaseFileItem> {
    let (input, _) = anyspace0(input)?;
    let (input, _) = bytes::complete::tag("lease")(input)?;
    let (input, _) = anyspace1(input)?;
    let (input, address) = val_address(input)?;
    let (input, _) = anyspace0(input)?;
    let (input, _) = complete::char('{')(input)?;
    let (input, _) = anyspace0(input)?;
    let (input, fields) = many1(lease_field)(input)?;
    let (input, _) = anyspace0(input)?;
    let (input, _) = complete::char('}')(input)?;
    let (input, _) = anyspace0(input)?;
    Ok((input, LeaseFileItem::Lease(address, fields)))
}

fn authoring_byte_order(input: &str) -> IResult<&str, LeaseFileItem> {
    let (input, _) = bytes::complete::tag("authoring-byte-order")(input)?;
    let (input, _) = anyspace1(input)?;
    let (input, byte_order) = alt((
        bytes::complete::tag("little-endian"),
        bytes::complete::tag("big-endian"),
    ))(input)?;
    let (input, _) = anyspace0(input)?;
    let (input, _) = complete::char(';')(input)?;
    let (input, _) = anyspace0(input)?;
    let byte_order = match byte_order {
        "little-endian" => ByteOrder::LittleEndian,
        "big-endian" => ByteOrder::BigEndian,
        _ => unreachable!(),
    };
    Ok((input, LeaseFileItem::AuthoringByteOrder(byte_order)))
}

fn lease_field(input: &str) -> IResult<&str, LeaseField> {
    let (input, _) = anyspace0(input)?;
    let (input, field) = alt((
        field_starts,
        field_ends,
        field_tstp,
        field_cltt,
        field_hardware_ethernet,
        field_client_hostname,
        field_uid,
        field_vendor_class_identifier,
        binding_state,
    ))(input)?;
    let (input, _) = anyspace0(input)?;
    let (input, _) = complete::char(';')(input)?;
    let (input, _) = anyspace0(input)?;
    Ok((input, field))
}

fn val_date(input: &str) -> IResult<&str, NaiveDate> {
    let (input, year) = complete::digit1(input)?;
    let (input, _) = complete::char('/')(input)?;
    let (input, month) = complete::digit1(input)?;
    let (input, _) = complete::char('/')(input)?;
    let (input, day) = complete::digit1(input)?;

    let year = year.parse::<i32>().unwrap_or_default();
    let month = month.parse::<u32>().unwrap_or_default();
    let day = day.parse::<u32>().unwrap_or_default();

    let date = NaiveDate::from_ymd_opt(year, month, day).unwrap_or_default();

    Ok((input, date))
}


fn val_time(input: &str) -> IResult<&str, NaiveTime> {
    let (input, hour) = complete::digit1(input)?;
    let (input, _) = complete::char(':')(input)?;
    let (input, minute) = complete::digit1(input)?;
    let (input, _) = complete::char(':')(input)?;
    let (input, second) = complete::digit1(input)?;

    let hour = hour.parse::<u32>().unwrap_or_default();
    let minute = minute.parse::<u32>().unwrap_or_default();
    let second = second.parse::<u32>().unwrap_or_default();

    let time = NaiveTime::from_hms_opt(hour, minute, second).unwrap_or_default();

    Ok((input, time))
}

fn val_never(input: &str) -> IResult<&str, Option<NaiveDateTime>> {
    let (input, _) = bytes::complete::tag("never")(input)?;
    Ok((input, None))
}

fn val_datetime_or_never(input: &str) -> IResult<&str, Option<NaiveDateTime>> {
    alt((val_datetime, val_never))(input)
}

fn val_datetime(input: &str) -> IResult<&str, Option<NaiveDateTime>> {
    let (input, _) = complete::one_of("01234567")(input)?;
    let (input, _) = complete::space0(input)?;
    let (input, date) = val_date(input)?;
    let (input, _) = complete::space0(input)?;
    let (input, time) = val_time(input)?;

    let datetime = NaiveDateTime::new(date, time);

    Ok((input, Some(datetime)))
}



//        wday date       time
// starts 0    2022/11/20 21:27:34;
fn field_starts(input: &str) -> IResult<&str, LeaseField> {
    let (input, _) = bytes::complete::tag("starts")(input)?;
    let (input, _) = space0(input)?;
    let (input, datetime) = val_datetime_or_never(input)?;

    Ok((input, LeaseField::Starts(datetime)))
}

fn field_ends(input: &str) -> IResult<&str, LeaseField> {
    let (input, _) = bytes::complete::tag("ends")(input)?;
    let (input, _) = space0(input)?;
    let (input, datetime) = val_datetime_or_never(input)?;

    Ok((input, LeaseField::Ends(datetime)))
}

fn field_tstp(input: &str) -> IResult<&str, LeaseField> {
    let (input, _) = bytes::complete::tag("tstp")(input)?;
    let (input, _) = space0(input)?;
    let (input, datetime) = val_datetime_or_never(input)?;

    Ok((input, LeaseField::Tstp(datetime)))
}

fn field_cltt(input: &str) -> IResult<&str, LeaseField> {
    let (input, _) = bytes::complete::tag("cltt")(input)?;
    let (input, _) = space0(input)?;
    let (input, datetime) = val_datetime_or_never(input)?;

    Ok((input, LeaseField::Cltt(datetime)))
}

fn field_hardware_ethernet(input: &str) -> IResult<&str, LeaseField> {
    let (input, mac) = keyword_hardware_ethernet(input)?;

    Ok((input, LeaseField::HardwareEthernet(mac)))
}

fn field_uid(input: &str) -> IResult<&str, LeaseField> {
    let (input, _) = bytes::complete::tag("uid")(input)?;
    let (input, _) = space0(input)?;
    let (input, s) = val_string(input)?;

    Ok((input, LeaseField::Uid(s)))
}

fn field_client_hostname(input: &str) -> IResult<&str, LeaseField> {
    let (input, _) = bytes::complete::tag("client-hostname")(input)?;
    let (input, _) = space0(input)?;
    let (input, s) = val_string(input)?;

    Ok((input, LeaseField::ClientHostname(s)))
}

fn field_vendor_class_identifier(input: &str) -> IResult<&str, LeaseField> {
    let (input, _) = bytes::complete::tag("set")(input)?;
    let (input, _) = space0(input)?;
    let (input, _) = bytes::complete::tag("vendor-class-identifier")(input)?;
    let (input, _) = space0(input)?;
    let (input, _) = complete::char('=')(input)?;
    let (input, _) = anyspace0(input)?;
    let (input, s) = val_string(input)?;

    Ok((input, LeaseField::VendorClassIdentifier(s)))
}

// take until ;
fn binding_state(input: &str) -> IResult<&str, LeaseField> {
    let (input, prefix) = combinator::opt(terminated(
        alt((bytes::complete::tag("next"), bytes::complete::tag("rewind"))),
        complete::space1,
    ))(input)?;
    let (input, _) = bytes::complete::tag("binding")(input)?;
    let (input, _) = space0(input)?;
    let (input, _) = bytes::complete::tag("state")(input)?;
    let (input, _) = space0(input)?;
    let (input, s) = alt((
        bytes::complete::tag("active"),
        bytes::complete::tag("free"),
        bytes::complete::tag("abandoned"),
        bytes::complete::tag("backup"),
        bytes::complete::tag("expired"),
        bytes::complete::tag("released"),
        bytes::complete::tag("reset"),
        bytes::complete::tag("reset"),
    ))(input)?;
    Ok((
        input,
        LeaseField::Ignore(format!(
            "{} binding state {}",
            prefix.unwrap_or_default(),
            s
        )),
    ))
}