use defmt::info;
use heapless::{String, Vec};
use trouble_host::{PacketPool, gatt::GattEvent};

use crate::{
    COMMANDS,
    bt::{AppStateMutex, ERROR_SIGNAL, Improv, RESULT_SIGNAL, RpcResult, STATE_SIGNAL, Server},
};

impl Improv {
    pub async fn handle<P: PacketPool>(
        &self,
        event: &GattEvent<'_, '_, P>,
        server: &Server<'_>,
        long_write: Option<(&[u8], u16)>,
        state: &AppStateMutex,
    ) {
        match event {
            GattEvent::Read(e) => {
                let handle = e.handle();
                if handle == self.capabilities.handle {
                    self.read_capabilities(server).await;
                } else if handle == self.rpc_result.handle {
                    self.read_result(server, state).await;
                } else if handle == self.current_state.handle {
                    self.read_state(server, state).await;
                } else if handle == self.error_state.handle {
                    self.read_error(server, state).await;
                }
            }
            GattEvent::Write(e) => {
                let handle = e.handle();
                if handle == self.rpc_command.handle {
                    self.run_command(e.data(), state).await;
                }
            }
            _ => (),
        }
        if let Some(lw) = long_write
            && lw.1 == self.rpc_command.handle
        {
            self.run_command(lw.0, state).await;
        }
    }

    async fn read_capabilities(&self, server: &Server<'_>) {
        info!("Reading capabilities");
        self.capabilities.set(server, &0x0F).unwrap()
    }

    async fn read_result(&self, server: &Server<'_>, state: &AppStateMutex) {
        let d = { &state.lock().await.rpc_result };
        self.rpc_result.set(server, d).unwrap();
    }

    async fn read_state(&self, server: &Server<'_>, state: &AppStateMutex) {
        let d = { state.lock().await.improv_state };
        self.current_state.set(server, &(d as u8)).unwrap();
    }

    async fn read_error(&self, server: &Server<'_>, state: &AppStateMutex) {
        let d = { state.lock().await.error_state };
        self.error_state.set(server, &(d as u8)).unwrap();
    }

    async fn run_command(&self, data: &[u8], state: &AppStateMutex) {
        info!("Received command: {:?}", data[..2]);
        match data {
            // Send wifi settings
            [0x01, ..] => {
                state.lock().await.improv_state = crate::bt::ImprovState::Provisioning;
                STATE_SIGNAL.signal(0x00);
                let wifi_data = parse_wifi(data);
                if wifi_data.is_none() {
                    state.lock().await.error_state = crate::bt::ErrorState::UnableToConnect;
                    ERROR_SIGNAL.signal(0x00);
                    return;
                }
                // todo error handling
                let wifi_unwrapped = wifi_data.unwrap();
                let mut ssid_vec = Vec::<u8, 32>::new();
                ssid_vec.extend_from_slice(wifi_unwrapped.0).unwrap();
                let ssid: String<32> = String::from_utf8(ssid_vec).unwrap();
                let mut password_vec = Vec::<u8, 64>::new();
                password_vec.extend_from_slice(wifi_unwrapped.1).unwrap();
                let password: String<64> = String::from_utf8(password_vec).unwrap();
                // todo: Probably set state to provisioning
                {
                    state
                        .lock()
                        .await
                        .channel
                        .send(crate::bt::handler::RpcCommand::SetWifiCreds { ssid, password })
                        .await;
                }
            }
            // Identify
            [0x02, ..] => {
                COMMANDS
                    .publisher()
                    .unwrap()
                    .publish(crate::Command::SetLeds(crate::LedCommand::BlinkIdentify))
                    .await
            }
            // Device info
            [0x03, ..] => {
                let mut data: Vec<String<20>, 4> = Vec::new();
                data.push(String::try_from("Aquarium").unwrap()).unwrap(); // Firmware name
                data.push(String::try_from("0.0.1").unwrap()).unwrap(); // Firmware version
                data.push(String::try_from("ESP32-C3").unwrap()).unwrap(); // Hardware
                data.push(String::try_from("Aquarium").unwrap()).unwrap(); // Device name
                {
                    let mut s = state.lock().await;
                    s.rpc_result = RpcResult::from_data(0x03, data);
                }
                RESULT_SIGNAL.signal(0x00);
            }
            // Scan wifi networks
            [0x04, ..] => {
                state
                    .lock()
                    .await
                    .channel
                    .send(crate::bt::handler::RpcCommand::ScanNetworks)
                    .await
            }
            // Get/Set Hostname
            [0x05, ..] => todo!(),
            _ => (),
        }
    }
}

fn parse_wifi(input: &[u8]) -> Option<(&[u8], &[u8])> {
    let input = input.get(2..)?; // skip command + length

    let (ssid_len, rest) = input.split_first()?;
    let (ssid, rest) = rest.split_at(*ssid_len as usize);

    let (pwd_len, rest) = rest.split_first()?;
    let (password, _) = rest.split_at(*pwd_len as usize);

    Some((ssid, password))
}
