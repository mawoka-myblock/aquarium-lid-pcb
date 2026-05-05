use core::net::IpAddr;

use crc::{CRC_32_ISCSI, Crc};
use defmt::{Format, unwrap};
use embedded_mqttc::{ClientConfig, ClientCredentials, Host};
use heapless::{String, Vec};
use postcard::{from_bytes_crc32, to_slice_crc32};
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
            let _ = nvs.invalidate_key(b"CFG").await;
            nvs.append_key(b"CFG", res).await.unwrap();
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
            alarm_above: 26.0,
            alarm_below: 23.0,
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
        let ssid_vec: Vec<u8, 32> = Vec::from_slice(&ssid[..32]).unwrap();
        let mut passwd_vec: Vec<u8, 64> = Vec::new();
        if let Some(pw_slice) = passwd {
            passwd_vec.extend_from_slice(&pw_slice[..64]).unwrap();
        }
        Some(WifiCreds {
            password: heapless::String::from_utf8(passwd_vec).unwrap(),
            ssid: heapless::String::from_utf8(ssid_vec).unwrap(),
        })
    }
    pub async fn save_to_nvs(self, nvs_mutex: NvsMutex) {
        let nvs = nvs_mutex.lock().await;
        nvs.invalidate_key(b"WF_SSID").await.ok();
        nvs.invalidate_key(b"WF_PW").await.ok();

        nvs.append_key(b"WF_SSID", self.ssid.as_bytes())
            .await
            .unwrap();
        nvs.append_key(b"WF_PW", self.password.as_bytes())
            .await
            .unwrap();
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
    pub fn to_mqtt_client_config(&self) -> ClientConfig<'_> {
        let host = match IpAddr::parse_ascii(self.host.as_bytes()) {
            Ok(d) => Host::Ip(d),
            Err(_) => Host::Hostname(&self.host),
        };
        ClientConfig {
            host,
            port: self.port,
            client_id: &self.client_id,
            credentials: Some(ClientCredentials {
                password: &self.password,
                username: &self.username,
            }),
            auto_subscribes: serde_json_core::heapless::Vec::new(),
        }
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
