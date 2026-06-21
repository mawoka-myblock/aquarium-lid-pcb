use core::{
    net::{Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6},
    num::NonZero,
    str::FromStr,
};

use crc::{CRC_32_ISCSI, Crc};
use defmt::{Format, unwrap};
use embassy_net::dns::DnsSocket;
use heapless::{String, Vec};
use postcard::{from_bytes_crc32, to_slice_crc32};
use rust_mqtt::{
    client::options::ConnectOptions,
    types::{MqttBinary, MqttString},
};
use serde::{Deserialize, Serialize};

use crate::{CONFIG_SIGNAL, Config, NvsMutex};

impl Config {
    pub async fn from_nvs(nvs_mutex: NvsMutex) -> Self {
        let data_from_nvs = {
            let nvs = nvs_mutex.lock().await;
            nvs.get_key(b"CFG").await.ok()
        };

        if let Some(d) = data_from_nvs {
            let crc = Crc::<u32>::new(&CRC_32_ISCSI);
            match from_bytes_crc32::<Self>(&d, crc.digest()) {
                Ok(d) => d,
                Err(_) => {
                    defmt::error!("Couldn't read nvs Config data, using default");
                    Self::default()
                }
            }
        } else {
            Self::default()
        }
    }

    pub async fn to_nvs(&self, nvs_mutex: NvsMutex) {
        let crc = Crc::<u32>::new(&CRC_32_ISCSI);
        let mut buf = [0u8; 1024];
        let res = to_slice_crc32(self, &mut buf, crc.digest()).unwrap();
        {
            let nvs = nvs_mutex.lock().await;
            if let Err(e) = nvs.invalidate_key(b"CFG").await {
                defmt::error!(
                    "Failed to invalidate old config key: {:#?}",
                    defmt::Debug2Format(&e)
                );
            }
            if let Err(e) = nvs.append_key(b"CFG", res).await {
                defmt::error!(
                    "Failed to write config to NVS: {:#?}",
                    defmt::Debug2Format(&e)
                );
            }
        }
    }

    pub fn get_from_signal() -> Self {
        let mut recv = CONFIG_SIGNAL.anon_receiver();
        unwrap!(recv.try_get())
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            max_safe_temp: 26.0,
            min_safe_temp: 23.0,
            alarm_hysteresis: 0.3,
            fan_on_threshold: 25.3,
            fan_off_threshold: 25.0,
            led_brightness: 255,
        }
    }
}
#[derive(Debug, Clone, Format)]
pub struct WifiCreds {
    pub ssid: heapless::String<32>,
    pub password: heapless::String<64>,
}

impl WifiCreds {
    pub async fn from_nvs(nvs_mutex: NvsMutex) -> Option<WifiCreds> {
        let nvs = nvs_mutex.lock().await;

        let (ssid, passwd) = {
            let ssid = nvs.get_key(b"WF_SSID").await.ok()?;
            let passwd = nvs.get_key(b"WF_PW").await.ok();
            (ssid, passwd)
        };
        let ssid_vec: Vec<u8, 32> = Vec::from_slice(&ssid[..32]).ok()?;
        let mut passwd_vec: Vec<u8, 64> = Vec::new();
        if let Some(pw_slice) = passwd {
            passwd_vec.extend_from_slice(&pw_slice[..64]).ok()?;
        }
        Some(WifiCreds {
            password: heapless::String::from_utf8(passwd_vec).ok()?,
            ssid: heapless::String::from_utf8(ssid_vec).ok()?,
        })
    }
    pub async fn save_to_nvs(self, nvs_mutex: NvsMutex) {
        let nvs = nvs_mutex.lock().await;
        if let Err(e) = nvs.invalidate_key(b"WF_SSID").await {
            defmt::error!(
                "Failed to invalidate WF_SSID: {:#?}",
                defmt::Debug2Format(&e)
            );
        }
        if let Err(e) = nvs.invalidate_key(b"WF_PW").await {
            defmt::error!("Failed to invalidate WF_PW: {:#?}", defmt::Debug2Format(&e));
        }

        if let Err(e) = nvs.append_key(b"WF_SSID", self.ssid.as_bytes()).await {
            defmt::error!("Failed to save SSID: {:#?}", defmt::Debug2Format(&e));
        }
        if let Err(e) = nvs.append_key(b"WF_PW", self.password.as_bytes()).await {
            defmt::error!("Failed to save password: {:#?}", defmt::Debug2Format(&e));
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct MqttData {
    pub host: String<64>,
    pub port: Option<u16>,
    pub client_id: String<32>,
    pub username: String<32>,
    pub password: String<64>,
}

impl MqttData {
    pub fn to_mqtt_client_config(&self) -> ConnectOptions<'_> {
        ConnectOptions::new()
            .clean_start()
            .session_expiry_interval(rust_mqtt::config::SessionExpiryInterval::EndOnDisconnect)
            .keep_alive(rust_mqtt::config::KeepAlive::Seconds(
                NonZero::new(10).unwrap(),
            ))
            .user_name(MqttString::from_str(self.username.as_str()).unwrap())
            .password(MqttBinary::from_slice(self.password.as_bytes()).unwrap())
    }

    pub async fn from_nvs(nvs_mutex: NvsMutex) -> Option<Self> {
        let data_from_nvs = {
            let nvs = nvs_mutex.lock().await;
            nvs.get_key(b"MQTT").await.ok()
        };

        if let Some(d) = data_from_nvs {
            let crc = Crc::<u32>::new(&CRC_32_ISCSI);
            match from_bytes_crc32::<Self>(&d, crc.digest()) {
                Ok(d) => Some(d),
                Err(_) => {
                    defmt::error!("Couldn't read nvs MqttData data, using default");
                    None
                }
            }
        } else {
            None
        }
    }

    pub async fn get_socket_addr(&self, dns: DnsSocket<'_>) -> Option<SocketAddr> {
        if let Ok(ipv4) = Ipv4Addr::from_str(self.host.as_str()) {
            return Some(SocketAddr::V4(SocketAddrV4::new(
                ipv4,
                self.port.unwrap_or(1883),
            )));
        }
        if let Ok(ipv6) = Ipv6Addr::from_str(self.host.as_str()) {
            return Some(SocketAddr::V6(SocketAddrV6::new(
                ipv6,
                self.port.unwrap_or(1883),
                0,
                0,
            )));
        }
        let dns_res = dns
            .query(&self.host, smoltcp::wire::DnsQueryType::A)
            .await
            .ok()?;
        if let Some(first_ip) = dns_res.first()
            && let embassy_net::IpAddress::Ipv4(ad) = first_ip
        {
            return Some(SocketAddr::V4(SocketAddrV4::new(
                *ad,
                self.port.unwrap_or(1883),
            )));
        }
        let dns_res = dns
            .query(&self.host, smoltcp::wire::DnsQueryType::Aaaa)
            .await
            .ok()?;
        if let Some(first_ip) = dns_res.first()
            && let embassy_net::IpAddress::Ipv6(ad) = first_ip
        {
            return Some(SocketAddr::V6(SocketAddrV6::new(
                *ad,
                self.port.unwrap_or(1883),
                0,
                0,
            )));
        }
        None
    }

    pub async fn to_nvs(&self, nvs_mutex: NvsMutex) {
        let crc = Crc::<u32>::new(&CRC_32_ISCSI);
        let mut buf = [0u8; 1024];
        let res = to_slice_crc32(self, &mut buf, crc.digest()).unwrap();
        {
            let nvs = nvs_mutex.lock().await;
            let _ = nvs.invalidate_key(b"MQTT").await;
            nvs.append_key(b"MQTT", res).await.unwrap();
        }
    }
}
