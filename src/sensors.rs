//! Bill of materials for the **MovementLogger** pumpfoil session
//! recorder ([movement_logger_firmware]) plus a curated set of USB-C
//! pluggable modules that run **fully open-source firmware**.
//!
//! Two groups live here:
//!
//! 1. **Firmware BOM** — the exact parts the bare-metal firmware drives
//!    on the STEVAL-MKBOXPRO (SensorTile.box PRO Rev_C): the dev board
//!    itself, its on-board sensor ICs (trackable as standalone parts
//!    for repair / custom-board scenarios), and the externally-wired
//!    u-blox MAX-M10S GPS.
//! 2. **USB-C OSS modules** — host-pluggable GPS / WiFi / MCU modules
//!    that (a) expose a **USB-C** connector and (b) run **fully
//!    open-source firmware** (ESP-IDF, Arduino core, RP2040 SDK …).
//!    Open-source firmware is a hard requirement — closed-blob-only
//!    modules do not belong in this list no matter how convenient the
//!    hardware is.
//!
//! [movement_logger_firmware]: https://github.com/zdavatz/movement_logger_firmware
//!
//! The distributor sources (`sources::distributors`) consume this list:
//! API distributors (Mouser / DigiKey / Farnell) look parts up by the
//! manufacturer part numbers in [`Part::mpns`]; the scrape distributors
//! (ST / SparkFun) use [`Part::st_url`] / [`Part::sparkfun_pid`].

/// What the part *is* — also the PDF section / DB `category` label.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Role {
    /// Evaluation / dev board (STEVAL-MKBOXPRO, ESP32 devkits).
    Devkit,
    /// Bare MCU silicon.
    Mcu,
    /// GNSS / GPS receiver.
    Gps,
    /// WiFi (and usually BLE) radio module.
    Wifi,
    /// Inertial measurement unit (accel + gyro).
    Imu,
    /// Magnetometer.
    Magnetometer,
    /// Barometric pressure sensor.
    Barometer,
    /// Temperature sensor.
    Temperature,
    /// Battery fuel gauge.
    FuelGauge,
}

impl Role {
    pub fn label(self) -> &'static str {
        match self {
            Role::Devkit => "Dev Boards",
            Role::Mcu => "MCU",
            Role::Gps => "GPS / GNSS",
            Role::Wifi => "WiFi Modules",
            Role::Imu => "IMU (Accel + Gyro)",
            Role::Magnetometer => "Magnetometer",
            Role::Barometer => "Barometer",
            Role::Temperature => "Temperature",
            Role::FuelGauge => "Fuel Gauge",
        }
    }
    pub fn from_label(s: &str) -> Option<Self> {
        [
            Role::Devkit,
            Role::Mcu,
            Role::Gps,
            Role::Wifi,
            Role::Imu,
            Role::Magnetometer,
            Role::Barometer,
            Role::Temperature,
            Role::FuelGauge,
        ]
        .into_iter()
        .find(|r| r.label() == s)
    }
    /// Render order in the report.
    pub fn order(self) -> u8 {
        match self {
            Role::Devkit => 0,
            Role::Gps => 1,
            Role::Wifi => 2,
            Role::Mcu => 3,
            Role::Imu => 4,
            Role::Magnetometer => 5,
            Role::Barometer => 6,
            Role::Temperature => 7,
            Role::FuelGauge => 8,
        }
    }
}

/// How the module attaches to a host. Drives the "USB-C pluggable"
/// badge in the report.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Connector {
    /// Surfaced as a soldered IC on the dev board — not separately
    /// pluggable (tracked for repair / custom-board BOM only).
    Soldered,
    /// Bare UART pins (e.g. the MAX-M10S wired to UART4).
    Uart,
    /// SparkFun/Qwiic I²C connector.
    Qwiic,
    /// USB-C — host-pluggable, the connector the user cares about.
    UsbC,
}

impl Connector {
    pub fn label(self) -> &'static str {
        match self {
            Connector::Soldered => "soldered IC",
            Connector::Uart => "UART pins",
            Connector::Qwiic => "Qwiic / I²C",
            Connector::UsbC => "USB-C",
        }
    }
    pub fn is_pluggable(self) -> bool {
        matches!(self, Connector::UsbC | Connector::Qwiic | Connector::Uart)
    }
}

/// User-facing capability checkboxes shown per part in the report.
/// Deliberately a small fixed set (the things a buyer scans for) — not
/// an exhaustive datasheet feature list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Feature {
    UsbC,
    Wifi,
    Bluetooth,
    Gps,
    Motion,
    SdCard,
}

impl Feature {
    /// Fixed render order for the checkbox row.
    pub const ALL: [Feature; 6] = [
        Feature::UsbC,
        Feature::Wifi,
        Feature::Bluetooth,
        Feature::Gps,
        Feature::Motion,
        Feature::SdCard,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Feature::UsbC => "USB-C",
            Feature::Wifi => "WiFi",
            Feature::Bluetooth => "Bluetooth",
            Feature::Gps => "GPS",
            Feature::Motion => "Motion sensors",
            Feature::SdCard => "SD-card",
        }
    }
}

impl Part {
    /// Which of the six buyer-facing capabilities this part has.
    /// Single source of truth, keyed by the stable `key` so adding a
    /// part is one match arm (no per-literal field, test stays green).
    /// Verified against vendor docs: STEVAL-MKBOXPRO = SensorTile.box
    /// PRO (BLE + USB-C + LSM6DSV16X IMU + microSD; no WiFi/GPS);
    /// LilyGO T-Beam S3 Supreme = all six (LilyGo-LoRa-Series hw doc);
    /// bare sensor ICs / MCU / fuel-gauge carry none of the six.
    pub fn features(&self) -> &'static [Feature] {
        use Feature::*;
        match self.key {
            "steval-mkboxpro" => &[Bluetooth, UsbC, Motion, SdCard],
            "lsm6dsv16x" => &[Motion],
            "ublox-max-m10s" | "sparkfun-max-m10s" => &[Gps],
            "esp32-c3-devkitc"
            | "esp32-s3-devkitc"
            | "sparkfun-thing-plus-c"
            | "seeed-xiao-esp32c3"
            | "seeed-xiao-esp32s3"
            | "seeed-xiao-esp32s3-sense"
            | "seeed-xiao-esp32c6" => &[Wifi, Bluetooth, UsbC],
            "lilygo-tbeam-s3-supreme" => &[UsbC, Wifi, Bluetooth, Gps, Motion, SdCard],
            // stm32u585ai, lis2mdl, lps22df, stts22h, stc3115 → none
            _ => &[],
        }
    }
}

#[derive(Debug, Clone)]
pub struct Part {
    /// Stable grouping id (snake-ish, used as the DB/report group key).
    pub key: &'static str,
    pub name: &'static str,
    pub role: Role,
    pub manufacturer: &'static str,
    /// Manufacturer part numbers, most-canonical first. Distributor
    /// APIs (Mouser / DigiKey / Farnell) are queried per MPN.
    pub mpns: &'static [&'static str],
    pub connector: Connector,
    /// `true` ⇒ the module runs **fully open-source firmware** (the
    /// host code we flash is OSS, and there is no closed-only firmware
    /// blob gating its core function). Hard filter for the USB-C
    /// pluggable group.
    pub oss_firmware: bool,
    /// Direct st.com product page (ST parts) — scraped for og:title /
    /// og:image; ST eStore doesn't expose price without auth.
    pub st_url: Option<&'static str>,
    /// SparkFun numeric product id (the `/products/<id>` slug).
    pub sparkfun_pid: Option<&'static str>,
    /// Generic vendor product page (u-blox, Espressif, Raspberry Pi …)
    /// scraped for og metadata when there's no ST / SparkFun page.
    pub direct_url: Option<&'static str>,
    pub note: &'static str,
}

/// The full tracked list: firmware BOM + USB-C OSS modules.
pub fn bom() -> Vec<Part> {
    vec![
        // ───────── Firmware BOM: dev board + on-board ICs ─────────
        Part {
            key: "steval-mkboxpro",
            name: "STEVAL-MKBOXPRO (SensorTile.box PRO Rev_C)",
            role: Role::Devkit,
            manufacturer: "STMicroelectronics",
            mpns: &["STEVAL-MKBOXPRO"],
            connector: Connector::UsbC,
            oss_firmware: true, // MovementLogger firmware is GPL/OSS
            st_url: Some("https://www.st.com/en/evaluation-tools/steval-mkboxpro.html"),
            sparkfun_pid: None,
            direct_url: None,
            note: "Primary target board. USB-C for charge + STM32 DFU flashing.",
        },
        Part {
            key: "stm32u585ai",
            name: "STM32U585AII6Q (Cortex-M33 @ 160 MHz)",
            role: Role::Mcu,
            manufacturer: "STMicroelectronics",
            mpns: &["STM32U585AII6Q", "STM32U585AII6QTR"],
            connector: Connector::Soldered,
            oss_firmware: true,
            st_url: Some("https://www.st.com/en/microcontrollers-microprocessors/stm32u585ai.html"),
            sparkfun_pid: None,
            direct_url: None,
            note: "MCU on the STEVAL-MKBOXPRO. Tracked for custom-board BOM.",
        },
        Part {
            key: "lsm6dsv16x",
            name: "LSM6DSV16X (6-axis IMU)",
            role: Role::Imu,
            manufacturer: "STMicroelectronics",
            mpns: &["LSM6DSV16XTR", "LSM6DSV16X"],
            connector: Connector::Soldered,
            oss_firmware: true,
            st_url: Some("https://www.st.com/en/mems-and-sensors/lsm6dsv16x.html"),
            sparkfun_pid: None,
            direct_url: None,
            note: "On SPI2. Primary motion sensor for the session recorder.",
        },
        Part {
            key: "lis2mdl",
            name: "LIS2MDL (3-axis magnetometer)",
            role: Role::Magnetometer,
            manufacturer: "STMicroelectronics",
            mpns: &["LIS2MDLTR", "LIS2MDL"],
            connector: Connector::Soldered,
            oss_firmware: true,
            st_url: Some("https://www.st.com/en/mems-and-sensors/lis2mdl.html"),
            sparkfun_pid: None,
            direct_url: None,
            note: "On I²C2.",
        },
        Part {
            key: "lps22df",
            name: "LPS22DF (barometric pressure sensor)",
            role: Role::Barometer,
            manufacturer: "STMicroelectronics",
            mpns: &["LPS22DFTR", "LPS22DF"],
            connector: Connector::Soldered,
            oss_firmware: true,
            st_url: Some("https://www.st.com/en/mems-and-sensors/lps22df.html"),
            sparkfun_pid: None,
            direct_url: None,
            note: "On I²C2.",
        },
        Part {
            key: "stts22h",
            name: "STTS22H (digital temperature sensor)",
            role: Role::Temperature,
            manufacturer: "STMicroelectronics",
            mpns: &["STTS22HTR", "STTS22H"],
            connector: Connector::Soldered,
            oss_firmware: true,
            st_url: Some("https://www.st.com/en/mems-and-sensors/stts22h.html"),
            sparkfun_pid: None,
            direct_url: None,
            note: "On I²C2.",
        },
        Part {
            key: "stc3115",
            name: "STC3115 (Li-Po fuel gauge)",
            role: Role::FuelGauge,
            manufacturer: "STMicroelectronics",
            mpns: &["STC3115AIQT", "STC3115"],
            connector: Connector::Soldered,
            oss_firmware: true,
            st_url: Some("https://www.st.com/en/power-management/stc3115.html"),
            sparkfun_pid: None,
            direct_url: None,
            note: "On I²C4.",
        },
        // ───────── GPS: the firmware's external receiver ─────────
        Part {
            key: "ublox-max-m10s",
            name: "u-blox MAX-M10S GNSS module",
            role: Role::Gps,
            manufacturer: "u-blox",
            mpns: &["MAX-M10S-00B", "MAX-M10S"],
            connector: Connector::Uart,
            // The receiver runs a closed u-blox blob, but it is wired
            // to a host running our OSS firmware and only configured
            // (UBX-CFG) over UART — no firmware flashed to it.
            oss_firmware: true,
            st_url: None,
            sparkfun_pid: None,
            direct_url: Some("https://www.u-blox.com/en/product/max-m10s-module"),
            note: "Wired to UART4 @ 38400. The firmware's GPS source.",
        },
        Part {
            key: "sparkfun-max-m10s",
            name: "SparkFun GPS Breakout - MAX-M10S (Qwiic)",
            role: Role::Gps,
            manufacturer: "SparkFun",
            mpns: &["GPS-18037", "SPX-18037"],
            connector: Connector::Qwiic,
            oss_firmware: true,
            st_url: None,
            sparkfun_pid: Some("18037"),
            direct_url: None,
            note: "Recommended MAX-M10S carrier. Open-hardware breakout.",
        },
        // ───────── USB-C pluggable, OSS-firmware modules ─────────
        // Selection rule: USB-C connector AND fully open-source
        // firmware (ESP-IDF Apache-2.0 / Arduino / RP2040 SDK).
        Part {
            key: "esp32-c3-devkitc",
            name: "ESP32-C3-DevKitC-02 (WiFi + BLE, USB-C)",
            role: Role::Wifi,
            manufacturer: "Espressif",
            mpns: &["ESP32-C3-DevKitC-02", "ESP32-C3-DEVKITC-02"],
            connector: Connector::UsbC,
            oss_firmware: true, // ESP-IDF (Apache-2.0)
            st_url: None,
            sparkfun_pid: None,
            direct_url: Some(
                "https://www.espressif.com/en/products/devkits/esp32-c3-devkitc-02",
            ),
            note: "RISC-V WiFi/BLE devkit, USB-C, runs OSS ESP-IDF.",
        },
        Part {
            key: "esp32-s3-devkitc",
            name: "ESP32-S3-DevKitC-1 (WiFi + BLE, dual USB-C)",
            role: Role::Wifi,
            manufacturer: "Espressif",
            mpns: &["ESP32-S3-DevKitC-1", "ESP32-S3-DEVKITC-1-N8"],
            connector: Connector::UsbC,
            oss_firmware: true,
            st_url: None,
            sparkfun_pid: None,
            direct_url: Some(
                "https://www.espressif.com/en/products/devkits/esp32-s3-devkitc-1",
            ),
            note: "Xtensa WiFi/BLE devkit, native USB-C, OSS ESP-IDF.",
        },
        Part {
            key: "sparkfun-thing-plus-c",
            name: "SparkFun Thing Plus - ESP32 WROOM (USB-C)",
            role: Role::Wifi,
            manufacturer: "SparkFun",
            mpns: &["WRL-20168", "DEV-20168"],
            connector: Connector::UsbC,
            oss_firmware: true,
            st_url: None,
            sparkfun_pid: Some("20168"),
            direct_url: None,
            note: "USB-C ESP32 + Qwiic. OSS Arduino/ESP-IDF, open hardware.",
        },
        // Seeed Studio XIAO ESP32 family — thumb-sized, USB-C, OSS
        // firmware (ESP-IDF / Arduino). The natural pick for a
        // host-pluggable WiFi/BLE add-on; far smaller than the
        // Espressif DevKitC boards.
        Part {
            key: "seeed-xiao-esp32c3",
            name: "Seeed Studio XIAO ESP32-C3 (WiFi + BLE, USB-C)",
            role: Role::Wifi,
            manufacturer: "Seeed Studio",
            mpns: &["XIAO ESP32C3", "113991054"],
            connector: Connector::UsbC,
            oss_firmware: true, // ESP-IDF / Arduino
            st_url: None,
            sparkfun_pid: None,
            direct_url: Some("https://www.seeedstudio.com/Seeed-XIAO-ESP32C3-p-5431.html"),
            note: "RISC-V XIAO, USB-C, external antenna. OSS ESP-IDF.",
        },
        Part {
            key: "seeed-xiao-esp32s3",
            name: "Seeed Studio XIAO ESP32-S3 (WiFi + BLE, USB-C)",
            role: Role::Wifi,
            manufacturer: "Seeed Studio",
            mpns: &["XIAO ESP32S3", "113991114"],
            connector: Connector::UsbC,
            oss_firmware: true,
            st_url: None,
            sparkfun_pid: None,
            direct_url: Some("https://www.seeedstudio.com/XIAO-ESP32S3-p-5627.html"),
            note: "Xtensa XIAO, USB-C. OSS ESP-IDF / Arduino.",
        },
        Part {
            key: "seeed-xiao-esp32s3-sense",
            name: "Seeed Studio XIAO ESP32-S3 Sense (WiFi + camera, USB-C)",
            role: Role::Wifi,
            manufacturer: "Seeed Studio",
            mpns: &["XIAO ESP32S3 Sense", "113991115"],
            connector: Connector::UsbC,
            oss_firmware: true,
            st_url: None,
            sparkfun_pid: None,
            direct_url: Some("https://www.seeedstudio.com/XIAO-ESP32S3-Sense-p-5639.html"),
            note: "S3 + OV2640 camera + mic. USB-C, OSS ESP-IDF.",
        },
        Part {
            key: "seeed-xiao-esp32c6",
            name: "Seeed Studio XIAO ESP32-C6 (WiFi 6 + BLE + Zigbee, USB-C)",
            role: Role::Wifi,
            manufacturer: "Seeed Studio",
            mpns: &["XIAO ESP32C6", "113991143"],
            connector: Connector::UsbC,
            oss_firmware: true,
            st_url: None,
            sparkfun_pid: None,
            direct_url: Some(
                "https://www.seeedstudio.com/Seeed-Studio-XIAO-ESP32C6-p-5884.html",
            ),
            note: "WiFi 6 / BLE / 802.15.4 XIAO, USB-C. OSS ESP-IDF.",
        },
        // All-in-one session-recorder board: GNSS + IMU + mag + baro
        // + microSD + WiFi on one USB-C PCB, so nothing external to
        // wire (the "single board" answer). Spec verified against
        // LilyGO's official hardware doc (Xinyuan-LilyGO/LilyGo-LoRa-
        // Series, docs/en/t_beam_supreme/t_beam_supreme_hw.md):
        // ESP32-S3FN8, QMI8658 6-axis IMU, QMC6310 mag, BME280
        // baro/temp/humidity, microSD slot, and a GNSS receiver that
        // is variant-dependent — u-blox MAX-M10 *or* Quectel L76K
        // (two product variants). oss_firmware: ESP-IDF / Arduino-
        // ESP32 (Apache-2.0); the GNSS runs its vendor firmware
        // exactly like the bare `ublox-max-m10s` part above (already
        // oss_firmware:true) — consistent treatment. Not stocked by
        // Mouser/DigiKey/Farnell; LilyGO sells direct (Shopify), so
        // `direct_url` is the lookup path (vendor source →
        // og:image/title/price).
        Part {
            key: "lilygo-tbeam-s3-supreme",
            name: "LilyGO T-Beam S3 Supreme (GNSS+IMU+mag+baro+SD, USB-C)",
            role: Role::Gps,
            manufacturer: "LilyGO",
            mpns: &["T-Beam S3 Supreme"],
            connector: Connector::UsbC,
            oss_firmware: true,
            st_url: None,
            sparkfun_pid: None,
            direct_url: Some("https://www.lilygo.cc/products/t-beam-supreme-meshtastic"),
            note: "All-in-one, nothing to plug in: ESP32-S3 WiFi/BLE + \
                   QMI8658 6-axis IMU + QMC6310 mag + BME280 \
                   baro/temp/humidity + microSD slot + GNSS (u-blox \
                   MAX-M10 or Quectel L76K, variant-dependent) on one \
                   USB-C board. OSS ESP-IDF / Arduino-ESP32.",
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bom_is_well_formed() {
        let parts = bom();
        assert!(parts.len() >= 12);
        // Keys unique.
        let mut keys: Vec<_> = parts.iter().map(|p| p.key).collect();
        keys.sort_unstable();
        let n = keys.len();
        keys.dedup();
        assert_eq!(keys.len(), n, "duplicate part key");
        // Every part is reachable by at least one distributor path.
        for p in &parts {
            assert!(
                !p.mpns.is_empty()
                    || p.st_url.is_some()
                    || p.sparkfun_pid.is_some()
                    || p.direct_url.is_some(),
                "{} has no distributor lookup path",
                p.key
            );
        }
        // Role label round-trips.
        for p in &parts {
            assert_eq!(Role::from_label(p.role.label()), Some(p.role));
        }
        // USB-C pluggable modules must run OSS firmware (hard rule —
        // applies to ANY USB-C role: WiFi, GPS, MCU, …).
        for p in &parts {
            if p.connector == Connector::UsbC {
                assert!(p.oss_firmware, "{} USB-C module must be OSS", p.key);
            }
            // A part declaring USB-C as a feature must actually be a
            // USB-C connector part (keeps the checkbox honest).
            if p.features().contains(&Feature::UsbC) {
                assert_eq!(
                    p.connector,
                    Connector::UsbC,
                    "{} claims USB-C feature but connector isn't UsbC",
                    p.key
                );
            }
        }
        // The all-in-one board carries every checkbox; bare sensor ICs
        // carry none — guards against a future edit silently regressing
        // the feature table.
        let by_key = |k: &str| parts.iter().find(|p| p.key == k).unwrap();
        assert_eq!(
            by_key("lilygo-tbeam-s3-supreme").features(),
            Feature::ALL.as_slice()
        );
        assert!(by_key("lps22df").features().is_empty());
    }
}
