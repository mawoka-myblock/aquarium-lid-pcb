use core::str::FromStr;

use defmt::info;
use embassy_futures::join::join;
use embassy_net::{DhcpConfig, StackResources};
use embassy_sync::{
    blocking_mutex::raw::{CriticalSectionRawMutex, NoopRawMutex},
    channel::Channel,
    mutex::Mutex,
};
use esp_hal::rng::Rng;
use esp_radio::wifi::{
    AuthenticationMethod, Config, Interfaces, WifiController, scan::ScanConfig, sta::StationConfig,
};
use heapless::{String, Vec};

use crate::{
    bt::{AppStateMutex, ERROR_SIGNAL, RESULT_SIGNAL, RpcResult, STATE_SIGNAL, try_connect_wifi},
    storage::{config::WifiCreds, nvs::Nvs},
};

pub enum RpcCommand {
    ScanNetworks,
    GetHostname,
    SetHostname(heapless::String<255>),
    SetWifiCreds {
        ssid: heapless::String<32>,
        password: heapless::String<64>,
    },
}

pub type WifiChannel = Channel<NoopRawMutex, RpcCommand, 1>;

pub async fn handler(
    channel: &'static WifiChannel,
    state: &AppStateMutex,
    controller: &'static mut WifiController<'static>,
    ifaces: Interfaces<'static>,
    nvs: &'static Mutex<CriticalSectionRawMutex, Nvs>,
) {
    info!("Handler running");
    let iface = ifaces.station;
    let rng = Rng::new();
    let seed = (rng.random() as u64) << 32 | rng.random() as u64;
    let mut dhcp_cfg = DhcpConfig::default();
    let hostname = heapless::String::from_str("aquarium").unwrap();
    dhcp_cfg.hostname = Some(hostname);
    let config = embassy_net::Config::dhcpv4(dhcp_cfg);
    let (_stack, mut runner) = embassy_net::new(
        iface,
        config,
        crate::mk_static!(StackResources<3>, StackResources::<3>::new()),
        seed,
    );
    let sta_cfg = Config::Station(StationConfig::default());
    controller.set_config(&sta_cfg).unwrap();
    let controller_mutex: Mutex<CriticalSectionRawMutex, &mut WifiController<'static>> =
        Mutex::new(controller);
    join(runner.run(), async {
        loop {
            match channel.receive().await {
                RpcCommand::ScanNetworks => {
                    let scan_res = controller_mutex
                        .lock()
                        .await
                        .scan_async(&ScanConfig::default().with_max(16))
                        .await
                        .unwrap();
                    let mut response_data: Vec<String<32>, 20> = Vec::new();
                    for res in scan_res {
                        info!("SSID: {}", res.ssid.as_str());
                        response_data
                            .push(String::from_str(res.ssid.as_str()).unwrap())
                            .unwrap();
                        info!("Signal Strength: {}", res.signal_strength);
                        response_data
                            .push(
                                String::from_str(itoa::Buffer::new().format(res.signal_strength))
                                    .unwrap(),
                            )
                            .unwrap();
                        response_data
                            .push(option_auth_to_string(res.auth_method))
                            .unwrap();
                    }
                    state.lock().await.rpc_result =
                        RpcResult::from_data::<32, 20>(0x04, response_data);
                    RESULT_SIGNAL.signal(0x00);
                }
                RpcCommand::GetHostname => {
                    let mut data = Vec::new();
                    data.push(String::from_str("aquarium").unwrap()).unwrap();
                    let rpc_result = RpcResult::from_data::<32, 1>(0x05, data);
                    {
                        let mut s = state.lock().await;
                        s.rpc_result = rpc_result;
                    }
                    RESULT_SIGNAL.signal(0x00);
                }
                RpcCommand::SetHostname(_d) => {
                    RESULT_SIGNAL.signal(0x00);
                }
                RpcCommand::SetWifiCreds { ssid, password } => {
                    let res = try_connect_wifi(&controller_mutex, &ssid, &password).await;
                    {
                        let mut s = state.lock().await;
                        match res {
                            Ok(_) => {
                                WifiCreds { password, ssid }.save_to_nvs(nvs).await;
                                s.rpc_result = RpcResult {
                                    data: Vec::new(),
                                    command: 0x01,
                                };
                                RESULT_SIGNAL.signal(0x00);
                                s.improv_state = crate::bt::ImprovState::Provisioned;
                            }
                            Err(_) => {
                                s.error_state = crate::bt::ErrorState::UnableToConnect;
                                ERROR_SIGNAL.signal(0x00);
                                s.improv_state = crate::bt::ImprovState::Authorized;
                            }
                        }
                    }
                    STATE_SIGNAL.signal(0x00);
                }
            }
        }
    })
    .await;
}

fn option_auth_to_string(am: Option<AuthenticationMethod>) -> String<32> {
    if am.is_none() {
        return String::from_str("NONE").unwrap();
    }
    let a = am.unwrap();
    String::from_str(match a {
        AuthenticationMethod::None => "NO",
        AuthenticationMethod::WapiPersonal => "WPA",
        AuthenticationMethod::Wep => "WEP",
        AuthenticationMethod::Wpa => "WPA",
        AuthenticationMethod::Wpa2Enterprise => "WPA2",
        AuthenticationMethod::Wpa2Personal => "WPA2",
        AuthenticationMethod::Wpa2Wpa3Personal => "WPA2",
        AuthenticationMethod::WpaWpa2Personal => "WPA",
        AuthenticationMethod::Wpa3Personal => "WPA3",
        _ => "NO",
    })
    .unwrap()
}
