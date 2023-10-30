use eyre::Result;
use nom::{
    branch::alt,
    bytes,
    character::{
        complete::{self, digit1},
        is_alphanumeric,
    },
    combinator::{all_consuming, map_res, opt},
    multi,
    sequence::{preceded, terminated, tuple},
    Finish, IResult,
};
use std::net::Ipv4Addr;

use crate::model::{Host, MacAddr};

use super::{anyspace0, anyspace1, keyword_hardware_ethernet, val_address, val_string};

pub fn parse(input: &str) -> Result<Vec<Host>> {
    let (_, items) = dhcpd_conf(input)
        .finish()
        .map_err(|e| eyre::eyre!("Failed to parse dhcpd.conf: {}", e))?;
    let mut hosts = vec![];

    for item in items {
        let Some(fixed_address) = item.fixed_address() else {
            continue;
        };
        let Some(hardware_ethernet) = item.hardware_ethernet() else {
            continue;
        };
        let hostname = item.hostname();
        hosts.push(Host {
            fixed_address,
            hardware_ethernet,
            hostname,
        });
    }

    Ok(hosts)
}

fn dhcpd_conf(input: &str) -> IResult<&str, Vec<HostFileItem>> {
    let (input, items) = all_consuming(multi::many0(file_item))(input)?;
    Ok((input, items))
}

#[derive(Debug, PartialEq)]
enum HostFileItem {
    Host {
        label: String,
        fields: Vec<HostField>,
    },
    Subnet,
    Option(String, DhcpOptionValue),
    Directive(String, Option<String>),
}

impl HostFileItem {
    #[allow(dead_code)]
    fn label(&self) -> Option<&str> {
        match self {
            Self::Host { label, .. } => Some(label),
            _ => None,
        }
    }

    fn fixed_address(&self) -> Option<Ipv4Addr> {
        match self {
            Self::Host { fields, .. } => {
                for field in fields {
                    if let HostField::FixedAddress(ip) = field {
                        return Some(*ip);
                    }
                }
                None
            }
            _ => None,
        }
    }

    fn hardware_ethernet(&self) -> Option<MacAddr> {
        match self {
            Self::Host { fields, .. } => {
                for field in fields {
                    if let HostField::HardwareEthernet(mac) = field {
                        return Some(mac.clone());
                    }
                }
                None
            }
            _ => None,
        }
    }

    fn hostname(&self) -> Option<String> {
        match self {
            Self::Host { fields, .. } => {
                for field in fields {
                    if let HostField::Option(name, value) = field {
                        if name == "host-name" {
                            return Some(value.to_string());
                        }
                    }
                }
                None
            }
            _ => None,
        }
    }
}

fn file_item(input: &str) -> IResult<&str, HostFileItem> {
    let (input, _) = anyspace0(input)?;
    let (input, item) = alt((host_block, subnet_block, option, directive))(input)?;
    let (input, _) = anyspace0(input)?;
    Ok((input, item))
}

fn host_block(input: &str) -> IResult<&str, HostFileItem> {
    let (input, _) = bytes::complete::tag("host")(input)?;
    let (input, _) = anyspace1(input)?;
    let (input, name) = val_identifier(input)?;
    let (input, _) = anyspace0(input)?;
    let (input, _) = complete::char('{')(input)?;
    let (input, _) = anyspace0(input)?;
    let (input, fields) = multi::many1(preceded(anyspace0, host_field))(input)?;
    let (input, _) = anyspace1(input)?;
    let (input, _) = complete::char('}')(input)?;
    Ok((
        input,
        HostFileItem::Host {
            label: name,
            fields,
        },
    ))
}

#[derive(Debug, PartialEq)]
enum HostField {
    HardwareEthernet(MacAddr),
    FixedAddress(Ipv4Addr),
    Option(String, String),
    Ignored(String),
}

fn host_field(input: &str) -> IResult<&str, HostField> {
    let (input, field) = alt((
        host_field_hardware_ethernet,
        host_field_fixed_address,
        host_field_option,
        host_field_set_hostname_override,
        host_field_default_lease_time,
        host_field_max_lease_time,
    ))(input)?;
    let (input, _) = anyspace0(input)?;
    let (input, _) = complete::char(';')(input)?;
    Ok((input, field))
}

fn host_field_hardware_ethernet(input: &str) -> IResult<&str, HostField> {
    let (input, mac) = keyword_hardware_ethernet(input)?;
    Ok((input, HostField::HardwareEthernet(mac)))
}

fn host_field_fixed_address(input: &str) -> IResult<&str, HostField> {
    let (input, _) = bytes::complete::tag("fixed-address")(input)?;
    let (input, _) = anyspace1(input)?;
    let (input, ip) = val_address(input)?;
    Ok((input, HostField::FixedAddress(ip)))
}

fn host_field_option(input: &str) -> IResult<&str, HostField> {
    let (input, _) = bytes::complete::tag("option")(input)?;
    let (input, _) = anyspace1(input)?;
    let (input, name) = val_identifier(input)?;
    let (input, _) = anyspace1(input)?;
    let (input, value) = val_string(input)?;
    Ok((input, HostField::Option(name, value)))
}

fn host_field_set_hostname_override(input: &str) -> IResult<&str, HostField> {
    let (input, _) = bytes::complete::tag("set")(input)?;
    let (input, _) = anyspace1(input)?;
    let (input, _) = bytes::complete::tag("hostname-override")(input)?;
    let (input, _) = anyspace1(input)?;
    let (input, _) = complete::char('=')(input)?;
    let (input, _) = anyspace1(input)?;
    let (input, _) = bytes::complete::tag("config-option")(input)?;
    let (input, _) = anyspace1(input)?;
    let (input, _) = bytes::complete::tag("host-name")(input)?;
    Ok((
        input,
        HostField::Ignored("set hostname-override = config-option host-name".to_string()),
    ))
}

fn host_field_default_lease_time(input: &str) -> IResult<&str, HostField> {
    let (input, _) = bytes::complete::tag("default-lease-time")(input)?;
    let (input, _) = anyspace1(input)?;
    let (input, s) = complete::digit1(input)?;
    let (input, _) = anyspace0(input)?;
    Ok((
        input,
        HostField::Ignored(format!("default-lease-time {s}")),
    ))
}

fn host_field_max_lease_time(input: &str) -> IResult<&str, HostField> {
    let (input, _) = bytes::complete::tag("max-lease-time")(input)?;
    let (input, _) = anyspace1(input)?;
    let (input, s) = complete::digit1(input)?;
    let (input, _) = anyspace0(input)?;
    Ok((input, HostField::Ignored(format!("max-lease-time {s}"))))
}

fn subnet_block(input: &str) -> IResult<&str, HostFileItem> {
    let (input, _) = bytes::complete::tag("subnet")(input)?;
    let (input, _) = anyspace1(input)?;
    let (input, _) = val_address(input)?;
    let (input, _) = anyspace1(input)?;
    let (input, _) = bytes::complete::tag("netmask")(input)?;
    let (input, _) = anyspace1(input)?;
    let (input, _) = val_address(input)?;
    let (input, _) = anyspace0(input)?;
    let (input, _) = complete::char('{')(input)?;
    let (input, _) = multi::many0(subnet_item)(input)?;
    let (input, _) = anyspace0(input)?;
    let (input, _) = complete::char('}')(input)?;
    Ok((input, HostFileItem::Subnet))
}

fn subnet_item(input: &str) -> IResult<&str, ()> {
    let (input, ()) = preceded(
        anyspace0,
        alt((
            pool_block,
            terminated(alt((subnet_option, subnet_field)), complete::char(';')),
        )),
    )(input)?;
    Ok((input, ()))
}

fn subnet_option(input: &str) -> IResult<&str, ()> {
    let (input, _) = bytes::complete::tag("option")(input)?;
    let (input, _) = anyspace1(input)?;
    let (input, _) = val_identifier(input)?;
    let (input, _) = anyspace1(input)?;
    let (input, _) = alt((val_ipaddr_string, val_string))(input)?;
    let (input, _) = anyspace0(input)?;
    Ok((input, ()))
}

fn subnet_field(input: &str) -> IResult<&str, ()> {
    let (input, _) = val_identifier(input)?;
    let (input, _) = anyspace1(input)?;
    let (input, _) = alt((val_ipaddr_string, val_string, val_int_string))(input)?;
    let (input, _) = anyspace0(input)?;
    Ok((input, ()))
}

fn pool_block(input: &str) -> IResult<&str, ()> {
    let (input, _) = bytes::complete::tag("pool")(input)?;
    let (input, _) = anyspace0(input)?;
    let (input, _) = complete::char('{')(input)?;
    let (input, _) = anyspace0(input)?;
    let (input, _) = multi::many0(pool_field)(input)?;
    let (input, _) = complete::char('}')(input)?;
    Ok((input, ()))
}

fn pool_field(input: &str) -> IResult<&str, ()> {
    let (input, _) = anyspace0(input)?;
    let (input, ()) = alt((pool_field_option, pool_field_range))(input)?;
    let (input, _) = anyspace0(input)?;
    let (input, _) = complete::char(';')(input)?;
    let (input, _) = anyspace0(input)?;
    Ok((input, ()))
}

fn pool_field_option(input: &str) -> IResult<&str, ()> {
    let (input, _) = bytes::complete::tag("option")(input)?;
    let (input, _) = anyspace1(input)?;
    let (input, _) = val_identifier(input)?;
    let (input, _) = anyspace1(input)?;
    let (input, _) = val_address(input)?;
    Ok((input, ()))
}

fn pool_field_range(input: &str) -> IResult<&str, ()> {
    let (input, _) = bytes::complete::tag("range")(input)?;
    let (input, _) = anyspace1(input)?;
    let (input, _) = val_address(input)?;
    let (input, _) = anyspace1(input)?;
    let (input, _) = val_address(input)?;
    Ok((input, ()))
}

#[derive(Debug, PartialEq)]
enum DhcpOptionValue {
    String(String),
    CodeType(u8, DhcpOptionType),
}

fn option(input: &str) -> IResult<&str, HostFileItem> {
    let (input, _) = bytes::complete::tag("option")(input)?;
    let (input, _) = anyspace1(input)?;
    let (input, name) = val_identifier(input)?;
    let (input, _) = anyspace1(input)?;
    let (input, value) = alt((option_string, option_code_type))(input)?;
    let (input, _) = anyspace0(input)?;
    let (input, _) = complete::char(';')(input)?;
    Ok((input, HostFileItem::Option(name, value)))
}

fn option_string(input: &str) -> IResult<&str, DhcpOptionValue> {
    let (input, s) = val_string(input)?;
    Ok((input, DhcpOptionValue::String(s)))
}

fn option_code_type(input: &str) -> IResult<&str, DhcpOptionValue> {
    let (input, _) = bytes::complete::tag("code")(input)?;
    let (input, _) = anyspace1(input)?;
    let (input, code) = map_res(digit1, |s: &str| s.parse::<u8>())(input)?;
    let (input, _) = anyspace1(input)?;
    let (input, _) = complete::char('=')(input)?;
    let (input, _) = anyspace1(input)?;
    let (input, t) = option_type(input)?;

    Ok((input, DhcpOptionValue::CodeType(code, t)))
}

#[derive(Debug, PartialEq)]
enum DhcpOptionType {
    Text,
    UnsignedInteger(u8),
}

fn option_type(input: &str) -> IResult<&str, DhcpOptionType> {
    alt((option_type_text, option_type_unsigned_integer))(input)
}

fn option_type_text(input: &str) -> IResult<&str, DhcpOptionType> {
    let (input, _) = bytes::complete::tag("text")(input)?;
    Ok((input, DhcpOptionType::Text))
}

fn option_type_unsigned_integer(input: &str) -> IResult<&str, DhcpOptionType> {
    let (input, _) = tuple((
        bytes::complete::tag("unsigned"),
        anyspace1,
        bytes::complete::tag("integer"),
        anyspace1,
    ))(input)?;
    let (input, i) = map_res(digit1, |s: &str| s.parse::<u8>())(input)?;

    Ok((input, DhcpOptionType::UnsignedInteger(i)))
}

fn directive(input: &str) -> IResult<&str, HostFileItem> {
    let (input, _) = anyspace0(input)?;
    let (input, name) = val_directive_name(input)?;
    let (input, value) = opt(preceded(anyspace1, val_identifier))(input)?;
    let (input, _) = anyspace0(input)?;
    let (input, _) = complete::char(';')(input)?;

    Ok((input, HostFileItem::Directive(name, value)))
}

fn val_directive_name(input: &str) -> IResult<&str, String> {
    let (input, s) = alt((
        bytes::complete::tag("default-lease-time"),
        bytes::complete::tag("max-lease-time"),
        bytes::complete::tag("log-facility"),
        bytes::complete::tag("one-lease-per-client"),
        bytes::complete::tag("deny"),
        bytes::complete::tag("ping-check"),
        bytes::complete::tag("update-conflict-detection"),
        bytes::complete::tag("authoritative"),
    ))(input)?;

    Ok((input, s.to_string()))
}

fn val_identifier(input: &str) -> IResult<&str, String> {
    let (input, s) =
        bytes::streaming::take_while1(|c| is_alphanumeric(c as u8) || c == '_' || c == '-')(input)?;
    Ok((input, s.to_string()))
}

fn val_ipaddr_string(input: &str) -> IResult<&str, String> {
    let (input, s) = val_address(input)?;
    Ok((input, s.to_string()))
}

fn val_int_string(input: &str) -> IResult<&str, String> {
    let (input, s) = digit1(input)?;
    Ok((input, s.to_string()))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use crate::dhcp_parsers::anyspace0;

    use super::*;

    static TEST_HOST: &str = r#"host s_lan_0 {
  hardware ethernet 10:20:30:40:50:60;
  fixed-address 192.168.1.1;
  option host-name "router";
  set hostname-override = config-option host-name;
}"#;

    static TEST_SUBNET: &str = r#"subnet 192.168.1.0 netmask 255.255.255.0 {
  pool {
    option domain-name-servers 192.168.1.1;
    range 192.168.1.1 192.168.1.254;
  }

  option routers 192.168.1.1;
  option domain-search "home.arpa";
  option domain-name-servers 192.168.1.1;
  default-lease-time 86400;
  max-lease-time 7776000;
  option ntp-servers 192.168.1.1;
}"#;

    #[test]
    fn test_host_block() {
        let (
            input,
            HostFileItem::Host {
                label: name,
                fields,
            },
        ) = host_block(TEST_HOST).unwrap()
        else {
            panic!("Failed to parse host block");
        };
        assert_eq!(input, "");
        assert_eq!(name, "s_lan_0");
        assert_eq!(fields.len(), 4);
        assert_eq!(
            fields[0],
            HostField::HardwareEthernet(MacAddr::from([0x10, 0x20, 0x30, 0x40, 0x50, 0x60]))
        );
        assert_eq!(
            fields[1],
            HostField::FixedAddress(Ipv4Addr::new(192, 168, 1, 1))
        );
        assert_eq!(
            fields[2],
            HostField::Option("host-name".to_string(), "router".to_string())
        );
        assert_eq!(
            fields[3],
            HostField::Ignored("set hostname-override = config-option host-name".to_string())
        );
    }

    #[test]
    fn test_subnet_block_empty() {
        let (input, _) = subnet_block("subnet 192.168.1.0 netmask 255.255.255.0 { }").unwrap();
        assert_eq!(input, "");
    }

    #[test]
    fn test_subnet_block() {
        let (input, _) = subnet_block(TEST_SUBNET).unwrap();
        assert_eq!(input, "");
    }

    #[test]
    fn test_subnet_item() {
        let (input, ()) = subnet_item(
            r#"pool {
        option domain-name-servers 192.168.1.1;
        range 192.168.1.10 192.168.1.254;
        }"#,
        )
        .unwrap();
        assert_eq!(input, "");
    }

    #[test]
    fn test_pool_block() {
        let (input, ()) = pool_block(
            r#"pool {
        option domain-name-servers 192.168.1.1;
        range 192.168.1.10 192.168.1.254;
        }"#,
        )
        .unwrap();
        assert_eq!(input, "");
    }

    #[test]
    fn test_subnet_option() {
        let input = "option routers 192.168.1.1";
        let (input, ()) = subnet_option(input).unwrap();
        assert_eq!(input, "");
    }

    #[test]
    fn test_val_identifier() {
        let example = "foo-bar-baz # stuff\n";
        let (input, s) = val_identifier(example).unwrap();
        assert_eq!(s, "foo-bar-baz");
        assert_eq!(input, " # stuff\n");

        let (input, s) = terminated(val_identifier, anyspace0)(example).unwrap();
        assert_eq!(input, "");
        assert_eq!(s, "foo-bar-baz");

        let example = "foo-bar-baz # stuff";
        let (input, s) = terminated(val_identifier, anyspace0)(example).unwrap();
        assert_eq!(s, "foo-bar-baz");
        assert_eq!(input, "");
    }

    #[test]
    fn test_option_with_comment() {
        let input = "option arch code 93 = unsigned integer 16; # RFC4578";
        let (input, _) = option(input).unwrap();
        let (input, _) = anyspace0(input).unwrap();
        assert_eq!(input, "");
    }

    #[test]
    fn test_options() {
        let input = r#"option domain-name "home.arpa";
option ldap-server code 95 = text;
option arch code 93 = unsigned integer 16; # RFC4578
option pac-webui code 252 = text;"#;
        let (input, options) = multi::many1(terminated(option, anyspace0))(input).unwrap();
        assert_eq!(input, "");
        assert_eq!(options.len(), 4);
    }

    #[test]
    fn test_directive() {
        let input = r#"default-lease-time 7200;
max-lease-time 86400;
log-facility local7;
one-lease-per-client true;
deny duplicates;
ping-check true;
update-conflict-detection false;
authoritative;"#;

        let (input, directives) = multi::many1(terminated(directive, anyspace0))(input).unwrap();
        assert_eq!(input, "");
        assert_eq!(directives.len(), 8);
        assert_eq!(
            directives,
            vec![
                HostFileItem::Directive("default-lease-time".to_string(), Some("7200".to_string())),
                HostFileItem::Directive("max-lease-time".to_string(), Some("86400".to_string())),
                HostFileItem::Directive("log-facility".to_string(), Some("local7".to_string())),
                HostFileItem::Directive(
                    "one-lease-per-client".to_string(),
                    Some("true".to_string())
                ),
                HostFileItem::Directive("deny".to_string(), Some("duplicates".to_string())),
                HostFileItem::Directive("ping-check".to_string(), Some("true".to_string())),
                HostFileItem::Directive(
                    "update-conflict-detection".to_string(),
                    Some("false".to_string())
                ),
                HostFileItem::Directive("authoritative".to_string(), None),
            ]
        );
    }

    #[test]
    fn test_default_lease_and_max_lease_times() {
        let input = r#"host s_lan_16 {
  hardware ethernet f0:b3:ec:25:8c:2d;
  fixed-address 10.0.0.50;
  option host-name "Big-Apple";
  set hostname-override = config-option host-name;
  default-lease-time 86400;
  max-lease-time 7776000;
}"#;

        let (input, _) = host_block(input).unwrap();
        assert_eq!(input, "");
    }
}
