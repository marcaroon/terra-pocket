//! Windows BLE central for the Terra GATT service (docs/ble-protocol.md §1).
//!
//! Transport only: scanning, connecting, service discovery, TX notifications,
//! RX writes and the DEVICE STATUS read. It knows nothing about the protocol
//! payloads; `companion-core` drives the handshake and reconnects (M9).
#![cfg(windows)]

use std::fmt;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use windows::Devices::Bluetooth::Advertisement::{
    BluetoothLEAdvertisementReceivedEventArgs, BluetoothLEAdvertisementWatcher,
    BluetoothLEAdvertisementWatcherStatus, BluetoothLEScanningMode,
};
use windows::Devices::Bluetooth::GenericAttributeProfile::{
    GattCharacteristic, GattClientCharacteristicConfigurationDescriptorValue, GattCommunicationStatus,
    GattSession, GattSessionStatus, GattValueChangedEventArgs, GattWriteOption,
};
use windows::Devices::Bluetooth::{BluetoothCacheMode, BluetoothConnectionStatus, BluetoothLEDevice};
use windows::Foundation::TypedEventHandler;
use windows::Storage::Streams::{DataReader, DataWriter, IBuffer};
use windows::core::{GUID, IInspectable};

pub const DEVICE_NAME: &str = "Terra";
pub const SERVICE_UUID: GUID = GUID::from_u128(0x6F3A0001_9B21_4C71_9A8B_31A8B9F10001);
pub const RX_UUID: GUID = GUID::from_u128(0x6F3A0002_9B21_4C71_9A8B_31A8B9F10001);
pub const TX_UUID: GUID = GUID::from_u128(0x6F3A0003_9B21_4C71_9A8B_31A8B9F10001);
pub const STATUS_UUID: GUID = GUID::from_u128(0x6F3A0004_9B21_4C71_9A8B_31A8B9F10001);

/// Largest packet the firmware accepts (MTU 247 - 3).
pub const MAX_PACKET: usize = 244;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const DISCOVERY_ATTEMPTS: u32 = 3;

#[derive(Debug)]
pub enum BleError {
    Windows(windows::core::Error),
    /// A Windows call failed during the named step.
    Step(&'static str, windows::core::Error),
    /// A GATT operation completed with a non-success status.
    Gatt(&'static str, i32),
    NotFound(&'static str),
    /// The scan stopped on its own: Bluetooth is off or the adapter is gone.
    RadioUnavailable,
}

impl fmt::Display for BleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Windows(error) => write!(f, "Windows Bluetooth error: {}", error.message()),
            Self::Step(step, error) => {
                write!(f, "{step}: {} (HRESULT 0x{:08X})", error.message(), error.code().0 as u32)
            }
            Self::Gatt(what, status) => write!(f, "{what} failed (GATT status {status})"),
            Self::NotFound(what) => write!(f, "{what} not found"),
            Self::RadioUnavailable => f.write_str("Bluetooth is off or the adapter is unavailable"),
        }
    }
}

impl std::error::Error for BleError {}

impl From<windows::core::Error> for BleError {
    fn from(error: windows::core::Error) -> Self {
        Self::Windows(error)
    }
}

pub type Result<T> = std::result::Result<T, BleError>;

/// Attaches the failing step to a Windows error.
trait Step<T> {
    fn step(self, name: &'static str) -> Result<T>;
}

impl<T> Step<T> for windows::core::Result<T> {
    fn step(self, name: &'static str) -> Result<T> {
        self.map_err(|error| BleError::Step(name, error))
    }
}

fn check(status: GattCommunicationStatus, what: &'static str) -> Result<()> {
    if status == GattCommunicationStatus::Success { Ok(()) } else { Err(BleError::Gatt(what, status.0)) }
}

fn to_bytes(buffer: &IBuffer) -> Result<Vec<u8>> {
    let reader = DataReader::FromBuffer(buffer)?;
    let mut bytes = vec![0u8; reader.UnconsumedBufferLength()? as usize];
    reader.ReadBytes(&mut bytes)?;
    Ok(bytes)
}

fn to_buffer(bytes: &[u8]) -> Result<IBuffer> {
    let writer = DataWriter::new()?;
    writer.WriteBytes(bytes)?;
    Ok(writer.DetachBuffer()?)
}

/// A Terra peripheral seen while scanning.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Found {
    pub address: u64,
    pub name: String,
    pub rssi: i16,
}

impl Found {
    pub fn address_text(&self) -> String {
        let a = self.address.to_be_bytes();
        format!("{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}", a[2], a[3], a[4], a[5], a[6], a[7])
    }
}

/// Active scan for Terra advertisements (name in the advertisement, service UUID
/// in the scan response). Returns early once a device is found if `first_only`.
pub fn scan(timeout: Duration, first_only: bool) -> Result<Vec<Found>> {
    scan_until(timeout, |found| first_only && !found.is_empty())
}

/// Like `scan`, but returns as soon as `done` accepts the devices seen so far.
pub fn scan_until(timeout: Duration, mut done: impl FnMut(&[Found]) -> bool) -> Result<Vec<Found>> {
    let watcher = BluetoothLEAdvertisementWatcher::new()?;
    watcher.SetScanningMode(BluetoothLEScanningMode::Active)?;
    let found: Arc<Mutex<Vec<Found>>> = Arc::default();
    let sink = found.clone();
    let token = watcher.Received(&TypedEventHandler::<
        BluetoothLEAdvertisementWatcher,
        BluetoothLEAdvertisementReceivedEventArgs,
    >::new(move |_, args| {
        let Some(args) = args.as_ref() else { return Ok(()) };
        let advertisement = args.Advertisement()?;
        let name = advertisement.LocalName()?.to_string();
        let uuids = advertisement.ServiceUuids()?;
        let has_service = (0..uuids.Size()?).any(|i| uuids.GetAt(i).map(|u| u == SERVICE_UUID).unwrap_or(false));
        if name != DEVICE_NAME && !has_service {
            return Ok(());
        }
        let address = args.BluetoothAddress()?;
        let rssi = args.RawSignalStrengthInDBm()?;
        let mut list = sink.lock().unwrap_or_else(|p| p.into_inner());
        match list.iter_mut().find(|f| f.address == address) {
            Some(existing) => {
                existing.rssi = rssi;
                if existing.name.is_empty() {
                    existing.name = name;
                }
            }
            None => list.push(Found { address, name, rssi }),
        }
        Ok(())
    }))?;
    let started = watcher.Start();
    let deadline = Instant::now() + timeout;
    let mut aborted = started.is_err();
    while !aborted && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(100));
        if done(&found.lock().unwrap_or_else(|p| p.into_inner())) {
            break;
        }
        // Documented WinRT behavior: with the radio off the watcher moves to
        // Aborted instead of failing Start.
        aborted = watcher.Status().map(|s| s == BluetoothLEAdvertisementWatcherStatus::Aborted).unwrap_or(false);
    }
    let _ = watcher.Stop();
    watcher.RemoveReceived(token)?;
    started?;
    if aborted {
        return Err(BleError::RadioUnavailable);
    }
    let list = found.lock().unwrap_or_else(|p| p.into_inner()).clone();
    Ok(list)
}

/// Events delivered from WinRT threads while a link is open.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LinkEvent {
    /// One TX notification (a protocol packet).
    Packet(Vec<u8>),
    Disconnected,
}

/// An open GATT link to Terra with TX notifications enabled.
pub struct Link {
    connection: Connection,
    rx: GattCharacteristic,
    tx: GattCharacteristic,
    status: GattCharacteristic,
    tx_token: Option<i64>,
    connection_token: Option<i64>,
}

// Also closes partial connections when discovery or handler registration fails.
struct Connection {
    device: BluetoothLEDevice,
    session: GattSession,
}

impl Drop for Connection {
    fn drop(&mut self) {
        let _ = self.session.SetMaintainConnection(false);
        let _ = self.session.Close();
        let _ = self.device.Close();
    }
}

fn characteristic(
    service: &windows::Devices::Bluetooth::GenericAttributeProfile::GattDeviceService,
    uuid: GUID,
    what: &'static str,
) -> Result<GattCharacteristic> {
    let result = service.GetCharacteristicsForUuidAsync(uuid).and_then(|op| op.join()).step(what)?;
    check(result.Status()?, what)?;
    let list = result.Characteristics()?;
    if list.Size()? == 0 {
        return Err(BleError::NotFound(what));
    }
    Ok(list.GetAt(0)?)
}

impl Link {
    /// Connects, discovers the Terra service and subscribes to TX. The TX handler
    /// is registered before notifications are enabled, so the HELLO the firmware
    /// sends on subscribe cannot be missed (spec §5).
    pub fn connect(address: u64, on_event: impl Fn(LinkEvent) + Send + Sync + 'static) -> Result<Self> {
        let device = BluetoothLEDevice::FromBluetoothAddressAsync(address).and_then(|op| op.join()).step("Open device")?;
        let session = device
            .BluetoothDeviceId()
            .and_then(|id| GattSession::FromDeviceIdAsync(&id))
            .and_then(|op| op.join())
            .step("Open GATT session")?;
        let connection = Connection { device, session };
        let device = &connection.device;
        let session = &connection.session;
        session.SetMaintainConnection(true).step("Maintain connection")?;
        // Windows has no explicit connect: the first GATT request opens the link.
        // Observed on this laptop: waiting for an active session first never
        // completes, and uncached discovery on a link that is not up yet fails
        // with Unreachable or HRESULT 0x80070016. So discover with the default
        // cache mode (as bleak does), retry transient failures, then wait for the
        // session to become active.
        let mut attempt = 0;
        let services = loop {
            attempt += 1;
            let result = device.GetGattServicesForUuidAsync(SERVICE_UUID).and_then(|op| op.join());
            match result {
                Ok(result) if result.Status()? == GattCommunicationStatus::Success => break result,
                Ok(result) if attempt == DISCOVERY_ATTEMPTS => {
                    return Err(BleError::Gatt("Service discovery", result.Status()?.0));
                }
                Err(error) if attempt == DISCOVERY_ATTEMPTS => return Err(BleError::Step("Service discovery", error)),
                _ => std::thread::sleep(Duration::from_millis(500)),
            }
        };
        let deadline = Instant::now() + CONNECT_TIMEOUT;
        while session.SessionStatus()? != GattSessionStatus::Active {
            if Instant::now() >= deadline {
                return Err(BleError::NotFound("active GATT session"));
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        let list = services.Services()?;
        if list.Size()? == 0 {
            return Err(BleError::NotFound("Terra service"));
        }
        let service = list.GetAt(0)?;
        let rx = characteristic(&service, RX_UUID, "RX characteristic")?;
        let tx = characteristic(&service, TX_UUID, "TX characteristic")?;
        let status = characteristic(&service, STATUS_UUID, "STATUS characteristic")?;
        let mut link = Self { connection, rx, tx, status, tx_token: None, connection_token: None };

        let on_event = Arc::new(on_event);
        let packets = on_event.clone();
        link.tx_token = Some(link.tx.ValueChanged(&TypedEventHandler::<GattCharacteristic, GattValueChangedEventArgs>::new(
            move |_, args| {
                if let Some(args) = args.as_ref() {
                    if let Ok(bytes) = to_bytes(&args.CharacteristicValue()?) {
                        packets(LinkEvent::Packet(bytes));
                    }
                }
                Ok(())
            },
        ))?);
        let lost = on_event.clone();
        link.connection_token = Some(link.connection.device.ConnectionStatusChanged(&TypedEventHandler::<BluetoothLEDevice, IInspectable>::new(
            move |device, _| {
                if let Some(device) = device.as_ref() {
                    if device.ConnectionStatus()? == BluetoothConnectionStatus::Disconnected {
                        lost(LinkEvent::Disconnected);
                    }
                }
                Ok(())
            },
        ))?);
        let status = link
            .tx
            .WriteClientCharacteristicConfigurationDescriptorAsync(GattClientCharacteristicConfigurationDescriptorValue::Notify)
            .and_then(|op| op.join())
            .step("Enable TX notifications")?;
        check(status, "Enable TX notifications")?;
        Ok(link)
    }

    /// Maximum packet length for the current ATT MTU (MTU - 3), capped at the
    /// firmware limit. The MTU can grow after connecting, so read it per message.
    pub fn max_packet(&self) -> usize {
        let mtu = self.connection.session.MaxPduSize().unwrap_or(23) as usize;
        mtu.saturating_sub(3).clamp(20, MAX_PACKET)
    }

    /// Writes one protocol packet to RX with a write request (acknowledged by ATT).
    pub fn write(&self, packet: &[u8]) -> Result<()> {
        let status = self.rx.WriteValueWithOptionAsync(&to_buffer(packet)?, GattWriteOption::WriteWithResponse)?.join()?;
        check(status, "RX write")
    }

    /// Reads the raw DEVICE STATUS value (8 bytes in V1).
    pub fn read_status(&self) -> Result<Vec<u8>> {
        let result = self.status.ReadValueWithCacheModeAsync(BluetoothCacheMode::Uncached)?.join()?;
        check(result.Status()?, "STATUS read")?;
        to_bytes(&result.Value()?)
    }

    pub fn is_connected(&self) -> bool {
        self.connection.device.ConnectionStatus().map(|s| s == BluetoothConnectionStatus::Connected).unwrap_or(false)
    }
}

impl Drop for Link {
    fn drop(&mut self) {
        if let Some(token) = self.tx_token { let _ = self.tx.RemoveValueChanged(token); }
        if let Some(token) = self.connection_token { let _ = self.connection.device.RemoveConnectionStatusChanged(token); }
        if self.is_connected() {
            let _ = self
                .tx
                .WriteClientCharacteristicConfigurationDescriptorAsync(
                    GattClientCharacteristicConfigurationDescriptorValue::None,
                )
                .and_then(|op| op.join());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uuids_match_the_spec() {
        assert_eq!(format!("{SERVICE_UUID:?}"), "6F3A0001-9B21-4C71-9A8B-31A8B9F10001");
        assert_eq!(format!("{STATUS_UUID:?}"), "6F3A0004-9B21-4C71-9A8B-31A8B9F10001");
    }

    #[test]
    fn address_formatting() {
        let found = Found { address: 0x2884_8548_15BA, name: "Terra".into(), rssi: -40 };
        assert_eq!(found.address_text(), "28:84:85:48:15:BA");
    }
}
