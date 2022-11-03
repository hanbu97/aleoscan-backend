use std::collections::BTreeMap;

use ipgeolocate::{Locator, Service};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct IpInfo {
    pub ip: String,
    pub lat: f32,
    pub long: f32,
    pub city: String,
    pub region: String,
    pub contry: String,
}

pub struct GlobalIpMap {
    pub map: RwLock<BTreeMap<String, IpInfo>>,
}

impl GlobalIpMap {
    pub fn new() -> Self {
        Self {
            map: RwLock::new(BTreeMap::new()),
        }
    }

    // add new ip: ip_info pair to map
    pub fn insert(&self, ip: String, ip_info: IpInfo) {
        self.map.write().insert(ip, ip_info);
    }

    pub fn get_ips_info(&self, ips: Vec<String>) -> Vec<IpInfo> {
        let mut infos = vec![];

        let map = { self.map.read().clone() };

        for ip in ips {
            if let Some(ip_info) = map.get(&ip) {
                infos.push(ip_info.to_owned());
            } else {
            }
        }

        vec![]
    }
}




async fn get_ip_info(ip: &str) {
    let service = Service::IpApi;
    match Locator::get(&ip, service).await {
        Ok(ip) => println!("{} - {} ({})", ip.ip, ip.city, ip.country),
        Err(error) => println!("Error: {}", error),
    };
}




// store latest ip infos
pub struct CurrentIpInfo {}

// Prints the city where 1.1.1.1 is.
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let service = Service::IpApi;
    let ip = "1.1.1.1";

    match Locator::get(ip, service).await {
        Ok(ip) => println!("{} - {} ({})", ip.ip, ip.city, ip.country),
        Err(error) => println!("Error: {}", error),
    };

    Ok(())
}
