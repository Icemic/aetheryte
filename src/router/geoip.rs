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
        let info: Country = match self.reader.lookup(IpAddr::V4(*ip)) {
            Ok(info) => info,
            Err(_) => {
                println!("error on lookup addr geo info");
                // fallback to US
                self.reader.lookup(IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8))).unwrap()
            }
        };
        info.country.unwrap().iso_code.unwrap_or_default()
    }
}
