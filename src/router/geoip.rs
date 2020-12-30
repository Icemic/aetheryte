use maxminddb::{geoip2::Country, Reader};
use std::net::{IpAddr, Ipv4Addr};
use tokio::fs;

pub struct GeoIP {
    pub reader: Reader<Vec<u8>>,
}

impl GeoIP {
    pub async fn new() -> GeoIP {
        match fs::read("data/GeoLite2-Country.mmdb").await {
            Ok(file) => {
                let reader = Reader::from_source(file).unwrap();
                GeoIP { reader }
            }
            Err(_) => {
                panic!("failed to open geolite2 mmdb file");
            }
        }
    }
    pub fn lookup_country_code(&self, ip: &Ipv4Addr) -> &str {
        if let Ok(info) = self.reader.lookup::<Country>(IpAddr::V4(*ip)) {
            if let Some(country) = info.country {
                return country.iso_code.unwrap_or_default();
            }
        }
        // println!("warning: error on lookup addr geo info");
        "ERROR"
    }
}
