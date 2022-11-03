use std::{
    collections::BTreeMap,
    sync::{
        atomic::{AtomicI16, AtomicUsize},
        Arc,
    },
};

use chrono::{DateTime, Utc};
use futures::StreamExt;
use ipgeolocate::{Locator, Service};
use lazy_static::lazy_static;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing::warn;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct IpInfo {
    pub ip: String,
    pub lat: f32,
    pub long: f32,
    pub city: String,
    pub region: String,
    pub contry: String,
    pub timezone: String,
}

impl From<Locator> for IpInfo {
    fn from(l: Locator) -> Self {
        Self {
            ip: l.ip,
            lat: l.latitude.parse::<f32>().unwrap_or(0.),
            long: l.longitude.parse::<f32>().unwrap_or(0.),
            city: l.city,
            region: l.region,
            contry: l.country,
            timezone: l.timezone,
        }
    }
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

    pub async fn get_ips_info(&self, ips: Vec<String>) -> Vec<IpInfo> {
        let mut infos = vec![];

        let map = { self.map.read().clone() };

        let mut missing_ips = vec![];
        for ip in ips {
            if let Some(ip_info) = map.get(&ip) {
                infos.push(ip_info.to_owned());
            } else {
                missing_ips.push(ip)
            }
        }

        if missing_ips.is_empty() {
            return infos;
        }

        let new_infos = get_ip_info(missing_ips.clone()).await;
        for (ip, ip_info) in missing_ips.into_iter().zip(&new_infos) {
            self.insert(ip, ip_info.to_owned())
        }

        infos.extend(new_infos);
        infos
    }
}

lazy_static! {
    pub static ref GLOBAL_IP_MAP: GlobalIpMap = GlobalIpMap::new();
    pub static ref IP_APIS_POOL: IpApisSuggestor = IpApisSuggestor::new();
    pub static ref GLOBAL_PEERS_INFO: PeerList = PeerList::new();
}

// change api usage for each loop
pub struct IpApisSuggestor {
    pub pool: Vec<Service>,
    pub count: AtomicUsize,
}

impl IpApisSuggestor {
    pub fn new() -> Self {
        Self {
            pool: vec![
                Service::IpApi,
                Service::FreeGeoIp,
                Service::IpApiCo,
                Service::IpWhois,
            ],
            count: AtomicUsize::new(0),
        }
    }

    pub fn get(&self) -> Service {
        let l = self.pool.len();
        let idx = self.count.load(std::sync::atomic::Ordering::Relaxed) % l;

        if idx == l - 1 {
            self.count.store(0, std::sync::atomic::Ordering::Relaxed);
        } else {
            self.count
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }

        self.pool[idx]
    }
}

const API_CONCURRENCY: usize = 5;

async fn get_ip_info_inner(ips: Vec<String>) -> (Vec<IpInfo>, Vec<String>) {
    let service = IP_APIS_POOL.get();

    let mut tasks = vec![];
    for ip in &ips {
        tasks.push(Locator::get(ip, service))
    }

    let stream = futures::stream::iter(tasks).buffer_unordered(API_CONCURRENCY);
    let results = stream.collect::<Vec<_>>().await;

    let mut retry_ips = vec![];
    let mut ip_infos: Vec<IpInfo> = vec![];

    for (r, ip) in results.into_iter().zip(ips) {
        match r {
            Ok(l) => ip_infos.push(l.into()),
            Err(e) => {
                warn!("failed to req api {}, using {:?}", e, &service);
                retry_ips.push(ip);
            }
        }
    }

    (ip_infos, retry_ips)
}

async fn get_ip_info(ips: Vec<String>) -> Vec<IpInfo> {
    let mut ip_infos = vec![];
    let mut next = ips.clone();

    loop {
        let (out_infos, out_next) = get_ip_info_inner(next).await;
        ip_infos.extend(out_infos);

        if out_next.is_empty() {
            break;
        }
        next = out_next;
    }

    ip_infos
}

const API_BASE_URL: &str = "https://vm.aleo.org/api";
const PEERS_ALL: &str = "/testnet3/peers/all";

lazy_static! {
    pub static ref PEERS_ALL_URL: String = format!("{API_BASE_URL}{PEERS_ALL}");
}

pub type IpResult = Vec<String>;

async fn get_ips_from_rpc() -> anyhow::Result<IpResult> {
    Ok(reqwest::get(&*PEERS_ALL_URL).await?.json().await?)
}

pub struct PeerList {
    pub peers: Arc<RwLock<IpResult>>,
    pub peers_info: Arc<RwLock<Vec<IpInfo>>>,
    pub last_updated: Arc<RwLock<DateTime<Utc>>>,
}

const UPDATE_INTERVAL: u64 = 30;

async fn init_updater_inner(
    peers: Arc<RwLock<IpResult>>,
    last_updated: Arc<RwLock<DateTime<Utc>>>,
    peers_info: Arc<RwLock<Vec<IpInfo>>>,
) -> anyhow::Result<()> {
    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(UPDATE_INTERVAL)).await;

        let new_peers = get_ips_from_rpc().await?;
        let flag = { *peers.read() == new_peers };

        if !flag {
            let new_peers_info = GLOBAL_IP_MAP.get_ips_info(new_peers.clone()).await;
            {
                *peers_info.write() = new_peers_info;
                *peers.write() = new_peers
            }
            {
                *last_updated.write() = Utc::now()
            }
        }
    }
}

impl PeerList {
    pub fn new() -> Self {
        Self {
            peers: Arc::new(RwLock::new(vec![])),
            last_updated: Arc::new(RwLock::new(Utc::now())),
            peers_info: Arc::new(RwLock::new(vec![])),
        }
    }

    pub async fn init_updater(&self) {
        let peers = self.peers.clone();
        let last_updated = self.last_updated.clone();
        let peers_info = self.peers_info.clone();

        tokio::spawn(async move {
            loop {
                if let Err(e) =
                    init_updater_inner(peers.clone(), last_updated.clone(), peers_info.clone())
                        .await
                {
                    tracing::error!("{}", e);
                    continue;
                }
            }
        });
    }
}

#[tokio::test]
async fn test_get_ips_from_rpc() -> anyhow::Result<()> {
    get_ips_from_rpc().await?;

    Ok(())
}

// store latest ip infos
pub struct CurrentIpInfo {}

// Prints the city where 1.1.1.1 is.
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let service = Service::IpApi;
    let ip = "1.1.1.1";

    match Locator::get(ip, service).await {
        Ok(ip) => println!("{} - {} ({})", ip.ip, ip.city, ip.country),
        Err(error) => println!("Error: {}", error),
    };

    Ok(())
}
