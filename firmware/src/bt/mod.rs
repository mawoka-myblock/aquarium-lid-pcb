use defmt::{error, info, warn};
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, mutex::Mutex, signal::Signal};
use esp_radio::{
    ble::controller::BleConnector,
    wifi::{Config, Interfaces, WifiController, WifiError, sta::StationConfig},
};
use heapless::{String, Vec};
use trouble_host::prelude::*;

use embassy_futures::{join::join3, select::select4};

use crate::{
    bt::{
        handler::{WifiChannel, handler},
        long_write::{ConnectionContext, LongWriteAccumulator},
    },
    storage::nvs::Nvs,
};
pub mod handler;
mod long_write;
mod service;

#[derive(Debug, Clone, Copy)]
pub enum ImprovState {
    AuthorizationRequired = 0x01,
    Authorized = 0x02,
    Provisioning = 0x03,
    Provisioned = 0x04,
}

#[derive(Debug, Clone, Copy)]
pub enum ErrorState {
    NoError = 0x00,
    InvalidRpcPacket = 0x01,
    UnknownRpcCommand = 0x02,
    UnableToConnect = 0x03,
    NotAuthorized = 0x04,
    BadHostname = 0x05,
    UnknownError = 0xFF,
}

pub struct AppState {
    pub improv_state: ImprovState,
    pub error_state: ErrorState,
    pub rpc_result: RpcResult,
    pub channel: &'static WifiChannel,
}

pub type AppStateMutex = Mutex<CriticalSectionRawMutex, AppState>;
pub static ERROR_SIGNAL: Signal<CriticalSectionRawMutex, u8> = Signal::new();
pub static RESULT_SIGNAL: Signal<CriticalSectionRawMutex, u8> = Signal::new();
pub static STATE_SIGNAL: Signal<CriticalSectionRawMutex, u8> = Signal::new();

#[gatt_server]
pub struct Server {
    pub improv: Improv,
}

#[derive(Debug, Clone)]
pub struct RpcResult {
    pub data: Vec<u8, 512>,
    pub command: u8,
}

impl RpcResult {
    /// S: String Length
    /// L: Vec Length
    pub fn from_data<const S: usize, const L: usize>(command: u8, data: Vec<String<S>, L>) -> Self {
        let mut buf: Vec<u8, 512> = Vec::new();
        for s in data.iter() {
            let bytes = s.as_bytes();
            let len = bytes.len();
            // Make sure we don't overflow the buffer
            if buf.len() + 1 + len > buf.capacity() {
                break;
            }
            buf.push(len as u8).unwrap(); // store length
            buf.extend_from_slice(bytes).unwrap(); // store string bytes
        }
        RpcResult { data: buf, command }
    }
}

impl Default for RpcResult {
    fn default() -> Self {
        Self {
            data: Vec::new(),
            command: 0xff,
        }
    }
}

impl FromGatt for RpcResult {
    fn from_gatt(data: &[u8]) -> Result<Self, trouble_host::types::gatt_traits::FromGattError> {
        use heapless::Vec;
        use trouble_host::types::gatt_traits::FromGattError;

        // Need at least command, length, checksum
        if data.len() < 3 {
            return Err(FromGattError::InvalidLength);
        }

        // Verify checksum (last byte)
        let expected_checksum = data[data.len() - 1];
        let checksum: u8 = data[..data.len() - 1]
            .iter()
            .fold(0u8, |acc, &b| acc.wrapping_add(b));
        if checksum != expected_checksum {
            return Err(FromGattError::InvalidCharacter);
        }

        let command = data[0];
        let data_len = data[1] as usize;

        // Ensure advertised length fits buffer (excluding checksum)
        if data_len + 2 + 1 != data.len() {
            return Err(FromGattError::InvalidLength);
        }

        // Copy all payload bytes (excluding command, length, checksum) into flat Vec
        let mut payload: Vec<u8, 512> = Vec::new();
        let mut pos = 2; // start after command + length
        while pos < data.len() - 1 {
            let len = data[pos] as usize;
            pos += 1;
            if pos + len > data.len() - 1 {
                return Err(FromGattError::InvalidLength);
            }
            // Push length + bytes into payload
            payload
                .push(len as u8)
                .map_err(|_| FromGattError::InvalidCharacter)?;
            payload
                .extend_from_slice(&data[pos..pos + len])
                .map_err(|_| FromGattError::InvalidCharacter)?;
            pos += len;
        }

        Ok(RpcResult {
            command,
            data: payload,
        })
    }
}

impl AsGatt for RpcResult {
    const MAX_SIZE: usize = 1024;
    const MIN_SIZE: usize = 0;
    fn as_gatt(&self) -> &[u8] {
        static mut BUFFER: [u8; 1024] = [0; 1024];
        let buf = unsafe { &mut BUFFER[..] };
        let mut pos = 0;
        // Command byte
        buf[pos] = self.command;
        pos += 2; // reserve 2 bytes for command + length
        // Copy all flat-packed data into buffer
        for &b in self.data.iter() {
            if pos >= buf.len() {
                break; // avoid overflow
            }
            buf[pos] = b;
            pos += 1;
        }
        // Length byte (total payload + checksum)
        let data_len = (pos - 2 + 1) as u8;
        buf[1] = data_len;
        // Checksum
        let checksum: u8 = buf[..pos].iter().fold(0u8, |acc, &b| acc.wrapping_add(b));
        if pos < buf.len() {
            buf[pos] = checksum;
            pos += 1;
        }
        // Return slice to buffer
        &buf[..pos]
    }
}

#[gatt_service(uuid = "00467768-6228-2272-4663-277478268000")]
pub struct Improv {
    #[characteristic(uuid = "00467768-6228-2272-4663-277478268001", read, notify)]
    pub current_state: u8,
    #[characteristic(uuid = "00467768-6228-2272-4663-277478268002", read, notify)]
    pub error_state: u8,
    #[characteristic(uuid = "00467768-6228-2272-4663-277478268003", write)]
    pub rpc_command: u8, // handle long write
    #[characteristic(uuid = "00467768-6228-2272-4663-277478268004", read, notify)]
    pub rpc_result: RpcResult,
    #[characteristic(uuid = "00467768-6228-2272-4663-277478268005", read, notify)]
    pub capabilities: u8,
}

/// Max number of connections
const CONNECTIONS_MAX: usize = 1;

/// Max number of L2CAP channels.
const L2CAP_CHANNELS_MAX: usize = 2; // Signal + att
#[embassy_executor::task]
pub async fn improv_ble(
    controller: ExternalController<BleConnector<'static>, 20>,
    wifi_controller: &'static mut WifiController<'static>,
    wifi_interfaces: Interfaces<'static>,
    nvs: &'static Mutex<CriticalSectionRawMutex, Nvs>,
) {
    let wifi_channel: &'static WifiChannel = crate::mk_static!(WifiChannel, WifiChannel::new());
    // Using a fixed "random" address can be useful for testing. In real scenarios, one would
    // use e.g. the MAC 6 byte array as the address (how to get that varies by the platform).
    let address: Address = Address::random([0xff, 0x8f, 0x1a, 0x05, 0xe4, 0xff]);
    info!("Our address = {:?}", address);

    let mut resources: HostResources<DefaultPacketPool, CONNECTIONS_MAX, L2CAP_CHANNELS_MAX> =
        HostResources::new();
    let stack = trouble_host::new(controller, &mut resources).set_random_address(address);
    let Host {
        mut peripheral,
        runner,
        ..
    } = stack.build();

    info!("Starting advertising and GATT service");
    let server = Server::new_with_config(GapConfig::Peripheral(PeripheralConfig {
        name: "Aquarium",
        appearance: &appearance::power_device::GENERIC_POWER_DEVICE,
    }))
    .unwrap();
    let state: AppStateMutex = Mutex::new(AppState {
        error_state: ErrorState::NoError,
        improv_state: ImprovState::Authorized,
        rpc_result: RpcResult::default(),
        channel: wifi_channel,
    });
    let adv = async {
        info!("Advertise starting");
        loop {
            match advertise("Aquarium", &state, &mut peripheral, &server).await {
                Ok(conn) => {
                    let a = gatt_events_task(&server, &conn, &state);
                    let b = notify_error(&server, &conn, &state);
                    let c = notify_state(&server, &conn, &state);
                    let d = notify_result(&server, &conn, &state);
                    select4(a, b, c, d).await;
                }
                Err(e) => {
                    let e = defmt::Debug2Format(&e);
                    error!("[adv] error: {:?}", e);
                }
            }
        }
    };
    let _ = join3(
        ble_task(runner),
        handler(wifi_channel, &state, wifi_controller, wifi_interfaces, nvs),
        adv,
    )
    .await;
    // let _ = join(
    //     ble_task(runner),
    //     handler(wifi_channel, &state, wifi_controller, wifi_interfaces, nvs),
    // );
}

async fn ble_task<C: Controller, P: PacketPool>(mut runner: Runner<'_, C, P>) {
    info!("BLE Task running");
    loop {
        if let Err(e) = runner.run().await {
            let e = defmt::Debug2Format(&e);
            panic!("[ble_task] error: {:?}", e);
        }
    }
}

async fn gatt_events_task<P: PacketPool>(
    server: &Server<'_>,
    conn: &GattConnection<'_, '_, P>,
    state: &AppStateMutex,
) -> Result<(), Error> {
    let mut ctx = ConnectionContext {
        long_write: LongWriteAccumulator::new(),
    };
    info!("Before loop");
    let reason = loop {
        match conn.next().await {
            GattConnectionEvent::Disconnected { reason } => break reason,
            GattConnectionEvent::Gatt { event } => {
                let (long_write, in_progress) = match &event {
                    GattEvent::Other(e) => {
                        let acc = e.payload();
                        let inc = acc.incoming();
                        match inc {
                            trouble_host::att::AttClient::Request(req) => match req {
                                trouble_host::att::AttReq::PrepareWrite {
                                    handle,
                                    offset,
                                    value,
                                } => {
                                    let _ = ctx.long_write.prepare(handle, offset as usize, value);
                                    (None, true)
                                }
                                trouble_host::att::AttReq::ExecuteWrite { .. } => {
                                    (Some(ctx.long_write.execute()), false)
                                }
                                _ => (None, false),
                            },
                            _ => (None, false),
                        }
                    }
                    _ => (None, false),
                };
                if !in_progress {
                    server
                        .improv
                        .handle(&event, server, long_write, state)
                        .await;
                }

                if long_write.is_some() {
                    ctx.long_write.reset()
                }
                match event.accept() {
                    Ok(reply) => reply.send().await,
                    Err(e) => warn!("[gatt] error sending response: {:?}", e),
                };
            }
            _ => {} // ignore other Gatt Connection Events
        }
    };
    info!("[gatt] disconnected: {:?}", reason);
    Ok(())
}

/// Create an advertiser to use to connect to a BLE Central, and wait for it to connect.
async fn advertise<'values, 'server, C: Controller>(
    name: &'values str,
    state: &AppStateMutex,
    peripheral: &mut Peripheral<'values, C, DefaultPacketPool>,
    server: &'server Server<'values>,
) -> Result<GattConnection<'values, 'server, DefaultPacketPool>, BleHostError<C::Error>> {
    let mut advertiser_data = [0; 31];
    let len = AdStructure::encode_slice(
        &[
            AdStructure::Flags(LE_GENERAL_DISCOVERABLE | BR_EDR_NOT_SUPPORTED),
            AdStructure::ServiceData16 {
                uuid: [0x77, 0x46],
                data: &[
                    { state.lock().await.improv_state as u8 },
                    0xFF,
                    0x00,
                    0x00,
                    0x00,
                    0x00,
                ],
            },
            AdStructure::CompleteServiceUuids128(&[[
                0x00, 0x80, 0x26, 0x78, 0x74, 0x27, 0x63, 0x46, 0x72, 0x22, 0x28, 0x62, 0x68, 0x77,
                0x46, 0x00,
            ]]),
            // AdStructure::CompleteLocalName(name.as_bytes()),
        ],
        &mut advertiser_data[..],
    )
    .unwrap();
    let mut scan_data_buf = [0; 31];
    let scan_len = AdStructure::encode_slice(
        &[AdStructure::CompleteLocalName(name.as_bytes())],
        &mut scan_data_buf,
    )
    .unwrap();
    let advertiser = peripheral
        .advertise(
            &Default::default(),
            Advertisement::ConnectableScannableUndirected {
                adv_data: &advertiser_data[..len],
                scan_data: &scan_data_buf[..scan_len],
            },
        )
        .await
        .unwrap();
    info!("[adv] advertising");
    let conn = advertiser
        .accept()
        .await
        .unwrap()
        .with_attribute_server(server)
        .unwrap();
    info!("[adv] connection established");
    Ok(conn)
}

async fn notify_state<P: PacketPool>(
    server: &Server<'_>,
    conn: &GattConnection<'_, '_, P>,
    state: &AppStateMutex,
) {
    loop {
        let _ = STATE_SIGNAL.wait().await;
        info!("Received state");
        let istate = { state.lock().await.improv_state };
        server
            .improv
            .current_state
            .notify(conn, &(istate as u8))
            .await
            .unwrap();
    }
}

async fn notify_result<P: PacketPool>(
    server: &Server<'_>,
    conn: &GattConnection<'_, '_, P>,
    state: &AppStateMutex,
) {
    loop {
        let _ = RESULT_SIGNAL.wait().await;
        info!("Received result");
        let result = { &state.lock().await.rpc_result };
        server.improv.rpc_result.notify(conn, result).await.unwrap();
    }
}

async fn notify_error<P: PacketPool>(
    server: &Server<'_>,
    conn: &GattConnection<'_, '_, P>,
    state: &AppStateMutex,
) {
    loop {
        let _ = ERROR_SIGNAL.wait().await;
        info!("Received error");
        let estate = { state.lock().await.error_state };
        server
            .improv
            .error_state
            .notify(conn, &(estate as u8))
            .await
            .unwrap();
    }
}

pub async fn try_connect_wifi(
    controller_mutex: &Mutex<CriticalSectionRawMutex, &mut WifiController<'static>>,
    ssid: &str,
    password: &str,
) -> Result<(), WifiError> {
    let sta_cfg = Config::Station(
        StationConfig::default()
            .with_ssid(ssid)
            .with_password(password.into()),
    );
    let mut controller = controller_mutex.lock().await;
    controller.set_config(&sta_cfg)?;
    controller.set_power_saving(esp_radio::wifi::PowerSaveMode::Minimum)?;
    controller.connect_async().await?;
    Ok(())
}
