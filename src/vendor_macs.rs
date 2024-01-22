static VENDOR_MACS_URL: &str = "https://devtools360.com/en/macaddress/vendorMacs.xml?download=true";
const VENDORS_MACS_CACHE_SECS: u64 = 60 * 60 * 24 * 7;

use std::str::FromStr;

use crate::macaddr::{MacAddr, MacPrefix};

use quick_xml::{
    events::{attributes::AttrError, Event},
    name::QName,
    reader::Reader,
};
use radix_trie::Trie;

#[derive(Debug, Clone)]
pub struct VendorMapping(Trie<MacPrefix, String>);

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to get cache dir")]
    NoCacheDir,

    #[error("http request error: {0}")]
    Fetch(#[from] reqwest::Error),

    #[error("system time error: {0}")]
    SystemTime(#[from] std::time::SystemTimeError),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("xml attribute error: {0}")]
    XmlAttr(#[from] AttrError),

    #[error("xml error: {0}")]
    XmlParse(#[from] quick_xml::Error),

    #[error("invalid mac prefix: {0}")]
    InvalidMacPrefix(#[from] crate::macaddr::InvalidMacPrefix),
}

impl VendorMapping {
    pub async fn fetch(cache: bool) -> Result<Self, Error> {
        let cache_file = dirs::cache_dir()
            .ok_or(Error::NoCacheDir)?
            .join("dhcpd-api")
            .join("vendorMacs.xml");

        if cache && cache_file.exists() {
            let metadata = tokio::fs::metadata(&cache_file).await?;
            if metadata.modified()?.elapsed()?.as_secs() < VENDORS_MACS_CACHE_SECS {
                let xml = tokio::fs::read_to_string(cache_file).await?;
                return Self::parse(&xml);
            }
        }

        let xml = reqwest::get(VENDOR_MACS_URL).await?.text().await?;
        let dir = cache_file.parent().ok_or(Error::NoCacheDir)?.to_owned();
        tokio::fs::create_dir_all(dir).await?;
        tokio::fs::write(cache_file, &xml).await?;

        Self::parse(&xml)
    }

    pub fn parse(xml: &str) -> Result<Self, Error> {
        let mut reader = Reader::from_str(xml);
        reader.trim_text(true);
        let mut vendor_mapping = Trie::new();

        loop {
            match reader.read_event()? {
                Event::Empty(e) if e.name().as_ref() == b"VendorMapping" => {
                    let mut mac_prefix: Option<MacPrefix> = None;
                    let mut vendor_name: Option<String> = None;

                    for attr in e.attributes() {
                        let attr = attr?;
                        let value = attr.unescape_value()?;
                        match attr.key {
                            QName(b"mac_prefix") => {
                                let p = MacPrefix::from_str(&value)?;
                                mac_prefix.replace(p);
                            }

                            QName(b"vendor_name") => {
                                vendor_name.replace(value.into_owned());
                            }

                            _ => (),
                        }
                    }

                    if let (Some(mac_prefix), Some(vendor_name)) = (mac_prefix, vendor_name) {
                        let vendor_name = vendor_name.trim().to_owned();
                        vendor_mapping.insert(mac_prefix, vendor_name);
                    }
                }
                Event::Eof => break,
                _ => (),
            }
        }

        Ok(Self(vendor_mapping))
    }

    pub fn get_vendor_name(&self, mac: &MacAddr) -> Option<&str> {
        self.0
            .get_ancestor_value(&mac.into())
            .map(std::string::String::as_str)
    }
}
