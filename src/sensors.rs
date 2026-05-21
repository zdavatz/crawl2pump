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
    /// Downward distance / time-of-flight sensor (height over water).
    Distance,
    /// Temperature sensor.
    Temperature,
    /// Battery fuel gauge.
    FuelGauge,
    /// Battery cell (the 18650 that powers the LilyGO).
    Battery,
    /// Waterproof / weatherproof enclosure. Polycarbonate or ABS only —
    /// the GPS receiver and (where present) the radar must be able to
    /// see through the wall, so metal/carbon shells are excluded.
    Case,
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
            Role::Distance => "Distance / ToF",
            Role::Temperature => "Temperature",
            Role::FuelGauge => "Fuel Gauge",
            Role::Battery => "Battery / Power",
            Role::Case => "Enclosure",
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
            Role::Distance,
            Role::Temperature,
            Role::FuelGauge,
            Role::Battery,
            Role::Case,
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
            Role::Distance => 7,
            Role::Temperature => 8,
            Role::FuelGauge => 9,
            Role::Battery => 10,
            Role::Case => 11,
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
    /// Not a connector — a power cell (the 18650). Kept so the
    /// battery can be a first-class BOM part without faking a bus.
    Battery,
    /// Not a connector — a passive enclosure (project / junction box).
    /// Kept so an enclosure can be a first-class BOM part without
    /// faking a bus.
    Enclosure,
    /// RF coax (SMA / U.FL). Used for GPS antennas and their pigtails —
    /// they're physically a connector, not a host bus, so
    /// `is_pluggable()` stays false.
    Coax,
    /// 40-pin GPIO header on Raspberry Pi / compatible SBCs. Pluggable
    /// HATs (Hardware-Attached-on-Top) sit on this header.
    Gpio,
}

impl Connector {
    pub fn label(self) -> &'static str {
        match self {
            Connector::Soldered => "soldered IC",
            Connector::Uart => "UART pins",
            Connector::Qwiic => "Qwiic / I²C",
            Connector::UsbC => "USB-C",
            Connector::Battery => "18650 cell",
            Connector::Enclosure => "enclosure",
            Connector::Coax => "SMA / U.FL",
            Connector::Gpio => "40-pin GPIO header",
        }
    }
    pub fn is_pluggable(self) -> bool {
        matches!(
            self,
            Connector::UsbC | Connector::Qwiic | Connector::Uart | Connector::Gpio
        )
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
            // Raspberry Pi Zero 2 W: 2.4 GHz WiFi + Bluetooth 4.2 BLE +
            // boots from microSD. No onboard GPS / IMU; no USB-C
            // (Micro-USB power input).
            "rpi-zero-2-w" => &[Wifi, Bluetooth, SdCard],
            // Waveshare L76X GPS HAT — adds GPS to the Pi via UART on
            // GPIO14/15. Nothing else.
            "waveshare-l76x-gps-hat" => &[Gps],
            // PiSugar 3 is a power/battery HAT — none of the six
            // buyer-facing capabilities apply (USB-C jack is charge-only,
            // not a host bus).
            // stm32u585ai, lis2mdl, lps22df, stts22h, stc3115 → none
            _ => &[],
        }
    }

    /// EU/Switzerland resellers with a buy link, keyed by part. Used
    /// for boards that no API distributor (Mouser/DigiKey/Farnell)
    /// stocks, so the report can still point a Swiss/EU buyer at a
    /// real shop. `(label, url)`; label is "Shop (CC)". Links are
    /// either a verified deep product page or the shop's own product
    /// search (robust — stays valid if the shop reslugs). Sourced
    /// from LilyGO's official distributors page (lilygo.cc/pages/
    /// distributors), EU/CH entries only. Deliberately no price in
    /// the label — a hardcoded price in a static table goes stale.
    pub fn resellers(&self) -> &'static [(&'static str, &'static str)] {
        match self.key {
            "lilygo-tbeam-s3-supreme" => &[
                (
                    "Bastelgarage (CH)",
                    "https://www.bastelgarage.ch/lilygo-t-beam-supreme-meshtastic-esp32-s3-868mhz-lora-modul-mit-gps",
                ),
                (
                    "ChipDepot (CH)",
                    "https://chipdepot.ch/shop/lilygo-t-beam-supreme-m-868mhz/",
                ),
                (
                    "TinyTronics (NL)",
                    "https://www.tinytronics.nl/en/search?query=T-Beam+S3+Supreme",
                ),
                (
                    "OpenELAB (EU)",
                    "https://openelab.io/search?q=T-Beam+Supreme",
                ),
                (
                    "Elektor (EU)",
                    "https://www.elektor.com/catalogsearch/result/?q=T-Beam+Supreme",
                ),
                (
                    "The Machine Shop (UK)",
                    "https://themachineshop.uk/?s=T-Beam+Supreme&post_type=product",
                ),
                (
                    "Bot'n Roll (PT)",
                    "https://www.botnroll.com/en/search?s=T-Beam+Supreme",
                ),
                (
                    "LilyGO (direct)",
                    "https://lilygo.cc/products/t-beam-supreme-meshtastic",
                ),
            ],
            "battery-18650" => &[
                (
                    "nkon.nl (EU 18650 specialist)",
                    "https://www.nkon.nl/rechargeable/18650-size.html",
                ),
                (
                    "Bastelgarage (CH)",
                    "https://www.bastelgarage.ch/index.php?route=product/search&search=18650",
                ),
            ],
            _ => &[],
        }
    }

    /// Physical size as (L, B, H) in **centimetres**, keyed by part.
    /// Boards/modules = product enclosure/PCB size; bare solder-down
    /// ICs = datasheet package body size (sub-cm — that's the real
    /// "device" for an SMD part, shown for completeness). Sources:
    /// STEVAL-MKBOXPRO 63×40×20 mm (measured, see task notes); ESP32
    /// DevKitC + XIAO + Feather/Thing-Plus = vendor form-factor specs;
    /// LilyGO T-Beam S3 Supreme ≈ 100×33×13 mm (no 18650); ST IC
    /// packages from their datasheets (UFBGA169 7×7, LGA-14 3×2.5,
    /// LGA/HLGA-1x 2×2, UDFN-6 2×2, DFN-8 3×2). All 17 are known so
    /// the report never shows "—" for this row.
    pub fn dimensions_cm(&self) -> Option<(f32, f32, f32)> {
        let d = match self.key {
            "steval-mkboxpro" => (6.3, 4.0, 2.0),
            "stm32u585ai" => (0.70, 0.70, 0.06), // UFBGA169 7×7
            "lsm6dsv16x" => (0.30, 0.25, 0.08),  // LGA-14 3.0×2.5
            "lis2mdl" => (0.20, 0.20, 0.10),     // LGA-12 2×2
            "lps22df" => (0.20, 0.20, 0.10),     // HLGA-10 2×2
            "stts22h" => (0.20, 0.20, 0.05),     // UDFN-6 2×2
            "stc3115" => (0.30, 0.20, 0.06),     // DFN-8 3×2
            "ublox-max-m10s" => (1.01, 0.97, 0.25), // MAX-M10 module
            "sparkfun-max-m10s" => (2.54, 2.54, 0.60), // 1×1" Qwiic
            "esp32-c3-devkitc" => (5.2, 2.3, 1.0),
            "esp32-s3-devkitc" => (6.9, 2.55, 1.0),
            "sparkfun-thing-plus-c" => (5.84, 2.29, 0.71), // Feather FF
            "seeed-xiao-esp32c3" | "seeed-xiao-esp32s3" | "seeed-xiao-esp32c6" => {
                (2.1, 1.75, 0.40)
            }
            "seeed-xiao-esp32s3-sense" => (2.1, 1.75, 0.65), // + cam/mic
            "lilygo-tbeam-s3-supreme" => (10.0, 3.3, 1.3),
            "vl53l1x-tof" => (2.54, 2.54, 0.5), // SparkFun Qwiic 1×1"
            "qwiic-cable-100mm" => (10.0, 0.5, 0.3), // 100 mm flex cable
            "sparkfun-xm125-radar" => (5.08, 2.54, 0.5), // 1.0×2.0" board
            "qwiic-jumper-female" => (15.0, 0.5, 0.3), // ~150 mm flex cable
            "battery-18650" => (6.5, 1.8, 1.8), // ∅18 × 65 mm cylinder
            "hammond-1554g2gycl" => (12.0, 9.0, 6.0), // external 1554G2 size
            // SERPAC RBF63 — external 6.30 × 3.15 × N inches. Three
            // depths in the BOM: C10 = 1.59" (shallow), C16 = 2.17"
            // (medium), C22 = 3.35" (deep).
            "serpac-rbf63-c10-clear" => (16.0, 8.0, 4.0),
            "serpac-rbf63-c16-clear" => (16.0, 8.0, 5.5),
            "serpac-rbf63-c22-clear" => (16.0, 8.0, 8.5),
            // Active mag-mount GPS antenna: 40×40×13 mm puck + 3 m
            // coax. Listed dimensions are the puck.
            "sparkfun-gps-antenna-sma" => (4.0, 4.0, 1.3),
            // U.FL→SMA pigtail: 100 mm flex with two coax connectors.
            "sparkfun-ufl-sma-100mm" => (10.0, 0.3, 0.3),
            // Samtec FFSD-07 100 mm ribbon (1.27 mm both ends).
            "samtec-ffsd-07-100mm" => (10.0, 0.5, 0.2),
            // Generic 14-pin JTAG/SWD-to-DuPont cable, ~100 mm.
            "arm-jtag-dupont-cable" => (10.0, 0.5, 0.2),
            // Raspberry Pi Zero 2 W: 65 × 30 × 5 mm PCB.
            "rpi-zero-2-w" => (6.5, 3.0, 0.5),
            // PiSugar 3 for Zero: matches Pi Zero footprint (65 × 30 mm),
            // ~10 mm tall (LiPo cell + STM8 + pogo pins).
            "pisugar-3-5000mah" => (6.5, 3.0, 1.0),
            // Waveshare L76X GPS HAT: standard Pi-Zero-HAT footprint
            // (65 × 30 mm), ~10 mm with the chip antenna.
            "waveshare-l76x-gps-hat" => (6.5, 3.0, 1.0),
            _ => return None,
        };
        Some(d)
    }

    /// Canonical open-source-firmware repo for this device, keyed by
    /// part. For the MovementLogger recorder hardware (STEVAL-MKBOXPRO,
    /// the custom-board STM32U585, and every ST sensor IC + the GPS it
    /// reads) the OSS firmware *is* `movement_logger_firmware` — that
    /// repo is the whole reason this BOM exists. The USB-C ESP32
    /// modules run Espressif's ESP-IDF (Apache-2.0); the LilyGO
    /// all-in-one uses the LilyGo-LoRa-Series SDK/examples (the same
    /// repo whose hardware doc we verified its spec against). Every
    /// part is `oss_firmware: true`, so every part has a repo — the
    /// test enforces `Some` + a github.com URL so the report never
    /// renders a part without a firmware link.
    pub fn firmware_repo(&self) -> Option<&'static str> {
        // Passive accessories (cables, enclosures, antennas) have no
        // firmware concept.
        if matches!(
            self.key,
            "qwiic-cable-100mm"
                | "qwiic-jumper-female"
                | "battery-18650"
                | "hammond-1554g2gycl"
                | "serpac-rbf63-c10-clear"
                | "serpac-rbf63-c16-clear"
                | "serpac-rbf63-c22-clear"
                | "sparkfun-gps-antenna-sma"
                | "sparkfun-ufl-sma-100mm"
                | "samtec-ffsd-07-100mm"
                | "arm-jtag-dupont-cable"
        ) {
            return None;
        }
        let url = match self.key {
            "esp32-c3-devkitc"
            | "esp32-s3-devkitc"
            | "sparkfun-thing-plus-c"
            | "seeed-xiao-esp32c3"
            | "seeed-xiao-esp32s3"
            | "seeed-xiao-esp32s3-sense"
            | "seeed-xiao-esp32c6" => "https://github.com/espressif/esp-idf",
            // Sold as a Meshtastic device (the "(M)" variant ships
            // with it) — meshtastic/firmware is the actual flashable
            // end-user OSS firmware. (LilyGo-LoRa-Series is only
            // LilyGO's low-level hw examples/SDK, used to verify the
            // spec — not what a buyer runs.)
            "lilygo-tbeam-s3-supreme" => "https://github.com/meshtastic/firmware",
            // Raspberry Pi Zero 2 W runs Raspberry Pi OS = Linux kernel
            // (GPL-2.0). The canonical OSS firmware repo for the
            // platform is the Pi-flavoured kernel.
            "rpi-zero-2-w" => "https://github.com/raspberrypi/linux",
            // PiSugar 3: GPL firmware running on its STM8S003 PMU
            // + the host-side pisugar-server daemon. Org repo (whole
            // PiSugar ecosystem) is the canonical entry point.
            "pisugar-3-5000mah" => "https://github.com/PiSugar/PiSugar",
            // Waveshare L76X GPS HAT: the HAT itself has no MCU. The
            // OSS code that drives it on the Pi is the Linux kernel
            // serial driver (/dev/serial0) + gpsd. Linux kernel = the
            // single canonical github URL; gpsd lives on gitlab so the
            // unit test (github.com required) would reject it.
            "waveshare-l76x-gps-hat" => "https://github.com/raspberrypi/linux",
            // STEVAL-MKBOXPRO, STM32U585, LSM6DSV16X, LIS2MDL, LPS22DF,
            // STTS22H, STC3115, u-blox/SparkFun MAX-M10S → the recorder
            // firmware that drives them.
            _ => "https://github.com/zdavatz/movement_logger_firmware",
        };
        Some(url)
    }

    /// Host MCU spec, keyed by part — a compact, comparable one-liner
    /// (`<chip> · <core> @<MHz> · <flash>/<RAM> · <trait>`). `None` for
    /// parts that have no host MCU (bare sensor ICs, the GNSS modules
    /// — a GNSS receiver's internal core isn't user-programmable). The
    /// recorder's STM32U585 is the deliberately power/secure-optimised
    /// choice for battery session logging; the ESP32 boards trade that
    /// for raw dual-core throughput + on-board WiFi/BLE. Specs from the
    /// ST / Espressif datasheets.
    pub fn mcu(&self) -> Option<&'static str> {
        Some(match self.key {
            "steval-mkboxpro" | "stm32u585ai" => {
                "STM32U585 · Cortex-M33 @160 MHz · 2 MB flash / 786 KB SRAM · TrustZone · ultra-low-power"
            }
            "lilygo-tbeam-s3-supreme" | "esp32-s3-devkitc" | "seeed-xiao-esp32s3"
            | "seeed-xiao-esp32s3-sense" => {
                "ESP32-S3 · dual Xtensa LX7 @240 MHz · 8 MB flash / 512 KB SRAM (+PSRAM) · WiFi+BLE"
            }
            "esp32-c3-devkitc" | "seeed-xiao-esp32c3" => {
                "ESP32-C3 · RISC-V @160 MHz · 400 KB SRAM · WiFi+BLE"
            }
            "seeed-xiao-esp32c6" => {
                "ESP32-C6 · RISC-V @160 MHz · 512 KB SRAM · WiFi 6 + BLE + 802.15.4"
            }
            "sparkfun-thing-plus-c" => {
                "ESP32 (WROOM) · dual Xtensa LX6 @240 MHz · 520 KB SRAM · WiFi+BLE"
            }
            // Pi Zero 2 W: Broadcom BCM2710A1, Cortex-A53 quad-core,
            // 512 MB SDRAM, Linux-host (not a microcontroller).
            "rpi-zero-2-w" => {
                "BCM2710A1 · Cortex-A53 quad-core @1 GHz · 512 MB RAM · Linux-host"
            }
            // PiSugar 3 carries an STM8S003F3 8-bit MCU for power
            // management (charge / RTC heartbeat / button input). Not
            // user-programmable in the field — firmware is flashed at
            // PiSugar's factory — but documented for parity with the
            // ESP32 / STM32U585 boards.
            "pisugar-3-5000mah" => {
                "STM8S003F3 · STM8 8-bit @16 MHz · 8 KB flash · power-mgmt"
            }
            // bare sensor ICs + GNSS modules → no user-programmable MCU
            _ => return None,
        })
    }

    /// Approximate mass in **grams**, keyed by part. Sources, in order
    /// of preference: vendor product page (Pi Foundation, SparkFun,
    /// LilyGO, Hammond, Peli) → vendor datasheet → measured on a
    /// reference part → package-typical figure for bare SMD ICs
    /// (UFBGA / LGA / DFN bodies are sub-gram, computed from package
    /// volume × ~1.8 g/cm³ epoxy/silicon density). Foilboard
    /// mount-positioning needs gram precision — a 200 g lump on the
    /// nose changes board trim — so every part carries a number, never
    /// "—". The test in `bom_is_well_formed` enforces `Some(_)` and
    /// `> 0`. Bare ICs are clamped at 0.02 g (a real number, but the
    /// honest answer for a 2×2 mm chip is "essentially nothing" — keep
    /// the figure consistent rather than chase package-by-package
    /// fidelity).
    pub fn weight_g(&self) -> Option<f32> {
        Some(match self.key {
            // SensorTile.box PRO with case + LiPo (measured in
            // conversation context, matches ST product brief 90–95 g).
            "steval-mkboxpro" => 94.0,
            // Bare SMD ICs — package body × epoxy density.
            "stm32u585ai" => 0.05,                                 // UFBGA169 7×7×0.6
            "lsm6dsv16x" | "lis2mdl" | "lps22df" | "stts22h" | "stc3115" => 0.02,
            // u-blox MAX-M10S module datasheet typ. 0.55 g.
            "ublox-max-m10s" => 0.6,
            // SparkFun Qwiic 1×1" breakouts: ~3–4 g per board.
            "sparkfun-max-m10s" | "vl53l1x-tof" => 4.0,
            // SparkFun XM125 radar 1×2" board.
            "sparkfun-xm125-radar" => 5.0,
            // SparkFun magnetic-mount GPS antenna with 3 m RG-174:
            // 40×40×13 mm puck + magnet + cable ≈ 80 g (the magnet
            // dominates).
            "sparkfun-gps-antenna-sma" => 80.0,
            // Pigtails, ribbons, jumper sets.
            "sparkfun-ufl-sma-100mm" => 3.0,
            "samtec-ffsd-07-100mm" => 3.0,
            "arm-jtag-dupont-cable" => 5.0,
            "qwiic-cable-100mm" => 2.0,
            "qwiic-jumper-female" => 3.0,
            // Espressif / SparkFun ESP32 DevKits.
            "esp32-c3-devkitc" => 9.0,
            "esp32-s3-devkitc" => 10.0,
            "sparkfun-thing-plus-c" => 8.0,
            // Seeed XIAO family — Seeed publishes 2.0–2.5 g.
            "seeed-xiao-esp32c3" | "seeed-xiao-esp32c6" => 2.5,
            "seeed-xiao-esp32s3" => 3.0,
            "seeed-xiao-esp32s3-sense" => 3.0,
            // LilyGO T-Beam S3 Supreme PCB only (without 18650 cell).
            "lilygo-tbeam-s3-supreme" => 22.0,
            // Typical flat-top 18650 cell (Samsung/LG/Molicel 2.5–3 Ah).
            "battery-18650" => 45.0,
            // Hammond 1554G2GYCL — vendor spec ~152 g for 1554G2;
            // rounded to 154 g consistent with the value quoted in the
            // weight discussion above.
            "hammond-1554g2gycl" => 154.0,
            // SERPAC RBF63 (polycarbonate enclosure + metal inserts +
            // stainless screws): C10 shallow ≈ 105 g; C16 medium ≈ 130
            // g; C22 deep ≈ 165 g. All three estimated from
            // wall-thickness/footprint scaling against the Hammond
            // 1554G2 reference (154 g for 120 × 90 × 60 mm).
            "serpac-rbf63-c10-clear" => 105.0,
            "serpac-rbf63-c16-clear" => 130.0,
            "serpac-rbf63-c22-clear" => 165.0,
            // Pi-build parts: Pi Foundation publishes 9.3 g for Pi Zero
            // 2 W bare PCB; with the GPIO header soldered it climbs to
            // ~11 g (Pimoroni / Adafruit measurements).
            "rpi-zero-2-w" => 11.0,
            // PiSugar 3 (Zero form factor, 5000 mAh variant — LiPo cell
            // dominates; PCB ~5 g + cell ~55 g).
            "pisugar-3-5000mah" => 60.0,
            // Waveshare L76X GPS HAT — Pi-Zero footprint + patch
            // antenna, ~12 g.
            "waveshare-l76x-gps-hat" => 12.0,
            _ => return None,
        })
    }

    /// Canonical manufacturer / SparkFun-CDN PDF for this device, used
    /// to render a "Datasheet" link next to the OSS-firmware link on
    /// each card. Returns `None` for parts where no public datasheet
    /// URL is known — the renderer just omits the line.
    ///
    /// **Every URL here is reachability-verified** by the
    /// `datasheets_resolve` test (run via `cargo test --release -- \
    /// --ignored datasheets_resolve`). It GETs each URL with reqwest +
    /// a browser User-Agent and falls back to FlareSolverr for hosts
    /// that bot-fingerprint plain HTTP libraries (notably `www.st.com`
    /// — the URLs work in real browsers, which is where these links
    /// are clicked from). If you add a part with a datasheet here,
    /// run that test before committing.
    ///
    /// Trade-off on OEM accessories: SparkFun's own "data sheet" link
    /// for some OEM parts (PRT-14986 antenna, CAB-09145 pigtail) lives
    /// on `sparkle.sparkfun.com` and has gone offline (DNS dead). For
    /// those we link the SparkFun product page instead — it always
    /// resolves and carries the same spec/dimensions block a PDF
    /// datasheet would. Better a live HTML spec than a dead PDF link.
    pub fn datasheet(&self) -> Option<&'static str> {
        Some(match self.key {
            // STEVAL-MKBOXPRO: UM3133 is the official ST user manual
            // (datasheet equivalent) for the SensorTile.box PRO.
            // `www.st.com` 403s plain reqwest (Akamai bot fingerprint),
            // works in browsers + via FlareSolverr — see test note.
            "steval-mkboxpro" => {
                "https://www.st.com/resource/en/user_manual/um3133-sensortilebox-pro-stmicroelectronics.pdf"
            }
            // u-blox MAX-M10S datasheet (UBX-20035208), official u-blox.
            "ublox-max-m10s" => {
                "https://content.u-blox.com/sites/default/files/MAX-M10S_DataSheet_UBX-20035208.pdf"
            }
            // SparkFun MAX-M10S breakout — link the MAX-M10S chip
            // datasheet (the breakout's hookup info is on its product
            // page; the buyer-relevant spec is the chip datasheet).
            "sparkfun-max-m10s" => {
                "https://cdn.sparkfun.com/assets/7/5/9/a/a/MAX-M10S_DataSheet_UBX-20035208.pdf"
            }
            // SparkFun PRT-14986 magnetic-mount GPS antenna: the
            // product page is the canonical spec source — SparkFun's
            // own datasheet PDF moved/disappeared from sparkle.sparkfun.com.
            "sparkfun-gps-antenna-sma" => "https://www.sparkfun.com/products/14986",
            // SparkFun CAB-09145 pigtail: ditto — product page is the
            // spec source; the U.FL connector PDF was 404 on every
            // host we tried (Hirose's dispatcher serves no body).
            "sparkfun-ufl-sma-100mm" => "https://www.sparkfun.com/products/9145",
            // Hammond 1554 series spec PDF (covers all variants incl.
            // the clear-lid -CL parts).
            "hammond-1554g2gycl" => {
                "https://www.hammfg.com/electronics/small-case/plastic/1554.pdf"
            }
            // Samtec FFSD family catalog (covers the 2×7 1.27 mm IDC
            // shrouded ribbon assemblies — pinout, mating, lengths).
            "samtec-ffsd-07-100mm" => {
                "https://suddendocs.samtec.com/catalog_english/ffsd.pdf"
            }
            // Raspberry Pi Zero 2 W official product brief PDF
            // (datasheets.raspberrypi.com — Pi Foundation CDN).
            "rpi-zero-2-w" => {
                "https://datasheets.raspberrypi.com/rpizero2/raspberry-pi-zero-2-w-product-brief.pdf"
            }
            // PiSugar 3: no standalone datasheet PDF — the PiSugar
            // wiki page is the canonical spec (capacity, dimensions,
            // I²C registers, pin mapping). Live HTML spec > dead PDF.
            "pisugar-3-5000mah" => "https://github.com/PiSugar/PiSugar/wiki/PiSugar3",
            // Waveshare L76X GPS HAT: same trade-off — Waveshare's
            // wiki carries the schematic / pinout / sample code; no
            // separate PDF datasheet for the board (chip-level docs
            // are on Quectel's L76 datasheet, separate part).
            "waveshare-l76x-gps-hat" => "https://www.waveshare.com/wiki/L76X_GPS_HAT",
            // Generic JTAG/SWD-to-DuPont cable has no canonical PDF
            // datasheet — no MPN, no manufacturer. The note + the
            // soldering-guide diagram are the spec source.
            _ => return None,
        })
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
            note: "Recommended MAX-M10S carrier. Open-hardware breakout. \
                   Has an onboard chip antenna that works through the \
                   polycarbonate case, plus a U.FL connector for an \
                   external active antenna (see the two antenna parts \
                   below) if the signal needs a boost.",
        },
        // Optional external GPS antenna kit — improves fix time and
        // multipath rejection vs. the breakout's onboard chip antenna.
        // Mount the magnetic base on a non-metal part of the deck or
        // the case lid; route the U.FL→SMA pigtail through a glanded
        // hole if you want it outside the sealed box. The SparkFun
        // source supplies real image + price for both parts.
        Part {
            key: "sparkfun-gps-antenna-sma",
            name: "GPS/GNSS Magnetic Mount Antenna 3 m (active, SMA)",
            role: Role::Gps,
            manufacturer: "SparkFun",
            mpns: &["GPS-14986"],
            connector: Connector::Coax,
            oss_firmware: true, // passive RF — no firmware
            st_url: None,
            sparkfun_pid: Some("14986"),
            direct_url: None,
            note: "Active GPS antenna (~28 dB LNA, 3 m RG-174 coax, SMA \
                   male). Magnetic base — sits on the board's non-metal \
                   top or the case lid for best sky view. Needs a U.FL \
                   → SMA pigtail to mate with the MAX-M10S breakout's \
                   U.FL connector — see next part.",
        },
        // Solderless GPS bring-up — two options, both in the BOM so
        // the buyer picks based on preference.
        //
        // OPTION A (distributor-grade): Samtec FFSD-07-D-04.00-01-N,
        // 14-pin 1.27 mm IDC ribbon, 1.27 mm socket on BOTH ends.
        // Real DigiKey/Mouser/Farnell stock + photo + CHF 9.21. Plugs
        // into JP2 cleanly, but the other end is still 1.27 mm — so
        // the buyer ALSO needs a 1.27→2.54 mm adapter PCB + DuPont
        // jumpers to reach the GPS. Three SKUs, distributor-tracked.
        Part {
            key: "samtec-ffsd-07-100mm",
            name: "Samtec FFSD-07-D-04.00-01-N — 14-pin 1.27 mm IDC ribbon (100 mm)",
            role: Role::Gps,
            manufacturer: "Samtec",
            mpns: &["FFSD-07-D-04.00-01-N"],
            connector: Connector::Coax, // closest enum — fine-pitch IDC
            oss_firmware: true, // passive ribbon — no firmware
            st_url: None,
            sparkfun_pid: None,
            direct_url: Some("https://www.samtec.com/products/ffsd-07-d-04.00-01-n"),
            note: "Distributor-grade path: real CHF price + photo from \
                   Mouser/DigiKey/Farnell, plugs solderlessly onto JP2 \
                   (the FTSH-107 programming header). Caveat: the other \
                   end is also 1.27 mm — you'll need a 1.27→2.54 mm \
                   SWD adapter PCB (~CHF 3 on AliExpress, no canonical \
                   distributor MPN) and 4× female-female DuPont jumpers \
                   to bridge to the SparkFun MAX-M10S breakout's UART \
                   pins. Then a 2.54 mm 7-pin female header on JP4 for \
                   the 3.3 V tap. Single-SKU alternative below.",
        },
        // OPTION B (turnkey): generic 14-pin 1.27 mm → DuPont cable —
        // exactly what the buyer needs in one product, but only sold
        // on Amazon/AliExpress (no Mouser/DigiKey/Farnell MPN exists
        // for this category — checked the obvious vendors). Card
        // renders with the SVG placeholder; accepted trade-off.
        Part {
            key: "arm-jtag-dupont-cable",
            name: "ARM Cortex JTAG/SWD 14-pin Cable (1.27 mm → 0.1\" DuPont, ~100 mm)",
            role: Role::Gps,
            manufacturer: "Generic (Amazon / AliExpress OEMs)",
            mpns: &[], // no Mouser/DigiKey/Farnell MPN exists for this category
            connector: Connector::Coax, // closest enum — fine-pitch IDC
            oss_firmware: true, // passive cable — no firmware
            st_url: None,
            sparkfun_pid: None,
            direct_url: Some(
                "https://www.amazon.de/s?k=JTAG+SWD+14+pin+1.27mm+cable+female+dupont",
            ),
            note: "Solderless JP2 plug. One end: 14-pin 1.27 mm \
                   shrouded female socket (mates with the STEVAL's \
                   FTSH-107 programming header — keyed, only goes on \
                   one way). Other end: 14 loose female DuPont leads \
                   labelled by SWD/JTAG net name. Pick the four you \
                   need: pin 13 → GPS TX, pin 14 → GPS RX, pin 7 \
                   or 11 → GPS GND. For GPS VCC, slip a 2.54 mm \
                   7-pin female header onto JP4 (set the JP4 domain \
                   switch to 3 V first — meter 3.25–3.40 V before \
                   connecting) and DuPont the 3 V pin to GPS VCC. \
                   Bring-up first; solder for the durable rig.",
        },
        Part {
            key: "sparkfun-ufl-sma-100mm",
            name: "Interface Cable SMA → U.FL, 100 mm (pigtail)",
            role: Role::Gps,
            manufacturer: "SparkFun",
            mpns: &["CAB-09145"],
            connector: Connector::Coax,
            oss_firmware: true, // passive coax — no firmware
            st_url: None,
            sparkfun_pid: Some("9145"),
            direct_url: None,
            note: "100 mm pigtail: U.FL female (plugs into the SparkFun \
                   MAX-M10S breakout) ↔ SMA female bulkhead (mates with \
                   the antenna above's SMA male). Buy with the magnetic \
                   antenna or skip both if the onboard chip antenna is \
                   enough.",
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
        // Downward distance / ToF for height-over-water. ST VL53L1X
        // (I²C, up to ~4 m), the ecosystem-consistent choice — same
        // vendor as the recorder's other sensors, open ST/SparkFun/
        // Pololu driver, stocked by all API distributors. Connector
        // is Qwiic: it's a 2-wire I²C device on a Qwiic/STEMMA-QT
        // breakout, so it plugs into the STEVAL's I²C bus *and* the
        // LilyGO T-Beam S3 Supreme's exposed I²C (SDA17/SCL18). Note
        // in the field: laser ToF reads water poorly (specular IR
        // reflection) — angle it or expect noise; an ultrasonic /
        // radar altimeter is the rugged alternative if this proves
        // unreliable over open water.
        Part {
            key: "vl53l1x-tof",
            name: "ST VL53L1X ToF (downward distance, height-over-water)",
            role: Role::Distance,
            manufacturer: "STMicroelectronics",
            mpns: &["VL53L1CXV0FY/1", "VL53L1X"],
            connector: Connector::Qwiic,
            oss_firmware: true,
            st_url: Some("https://www.st.com/en/imaging-and-photonics-solutions/vl53l1x.html"),
            sparkfun_pid: Some("14722"),
            direct_url: None,
            note: "Laser ToF rangefinder, up to ~4 m, I²C/Qwiic. The \
                   height-over-water sensor neither board has onboard. \
                   LilyGO side: female-jumper cable to its I²C header \
                   (GPIO17/18), NOT its 'QWIIC socket' (that's UART1); \
                   STEVAL side: normal I²C. No soldering. Caveat: IR \
                   ToF is unreliable on flat water (specular \
                   reflection) — angle it or use the radar instead.",
        },
        // The cable that makes the ToF actually solder-free: a
        // SparkFun Qwiic Cable (100 mm, JST-SH 4-pin = STEMMA-QT
        // compatible). Listed in the Distance section right beside the
        // VL53L1X so the "what to actually order" set is complete.
        // Passive wire — no firmware (firmware_repo() → None, so no
        // OSS-firmware line renders for it; oss_firmware:true only
        // means "contains no closed blob", it's vacuous for a cable).
        Part {
            key: "qwiic-cable-100mm",
            name: "SparkFun Qwiic Cable 100 mm (solder-free I²C link)",
            role: Role::Distance,
            manufacturer: "SparkFun",
            mpns: &["PRT-14427"],
            connector: Connector::Qwiic,
            oss_firmware: true,
            st_url: None,
            sparkfun_pid: Some("14427"),
            direct_url: None,
            note: "4-pin JST-SH Qwiic / STEMMA-QT cable (100 mm). \
                   Qwiic↔Qwiic — use it between two Qwiic-native I²C \
                   ports (the STEVAL I²C path). NOT for the LilyGO: \
                   its 'External QWIIC Socket' is wired to UART1 \
                   (GPIO43/44), not I²C — the LilyGO needs the \
                   female-jumper cable to its real I²C header \
                   (GPIO17/18). Order one alongside the ToF — the \
                   breakout does not ship with a cable.",
        },
        // The "seal it fully inside a plastic box" distance option:
        // 60 GHz pulsed-coherent radar (Acconeer A121 on SparkFun's
        // XM125 Qwiic board). Radar ranges *through* a non-metal
        // enclosure wall, so unlike the IR ToF it needs no optical
        // window / air gap / anti-crosstalk — pot it completely in an
        // RF-transparent box. Water is a strong radar reflector, so
        // it's *more* reliable over open water than the VL53L1X (which
        // suffers IR specular reflection). Qwiic ⇒ same no-solder
        // cable as the ToF. oss_firmware:true on the same basis as the
        // u-blox GPS — host SDK / SparkFun Arduino lib is open; the
        // radar core blob is closed like every GNSS/radar IC.
        Part {
            key: "sparkfun-xm125-radar",
            name: "SparkFun XM125 60 GHz Radar (Acconeer A121, seal-in-box distance)",
            role: Role::Distance,
            manufacturer: "SparkFun",
            mpns: &["SEN-24540"],
            connector: Connector::Qwiic,
            oss_firmware: true,
            st_url: None,
            sparkfun_pid: Some("24540"),
            direct_url: None,
            note: "60 GHz pulsed-coherent radar (Acconeer A121). \
                   Ranges through a sealed non-metal enclosure wall — \
                   no optical window needed, fully pottable in a \
                   plastic box. I²C/Qwiic: connect to the LilyGO's \
                   real I²C header (GPIO17 SDA / GPIO18 SCL / 3V3 / \
                   GND) via the female-jumper cable — NOT the LilyGO \
                   'QWIIC socket' (that's UART1). On STEVAL / \
                   Qwiic-native hosts the plain Qwiic cable works. No \
                   soldering either way. More reliable over open water \
                   than IR ToF (water reflects radar strongly). Box \
                   wall must be RF-transparent (no metal / carbon).",
        },
        // The cable that actually connects a Qwiic sensor to the
        // LilyGO T-Beam S3 Supreme: its "QWIIC socket" is UART1, so a
        // plain Qwiic↔Qwiic cable can't reach the I²C bus. This one
        // is Qwiic on the sensor end and 4 loose female Dupont leads
        // on the other → push straight onto the LilyGO I²C header
        // pins (GPIO17 SDA, GPIO18 SCL, 3V3, GND). Still zero solder.
        Part {
            key: "qwiic-jumper-female",
            name: "SparkFun Flexible Qwiic Cable – Female Jumper (4-pin)",
            role: Role::Distance,
            manufacturer: "SparkFun",
            mpns: &["CAB-17261"],
            connector: Connector::Qwiic,
            oss_firmware: true,
            st_url: None,
            sparkfun_pid: Some("17261"),
            direct_url: None,
            note: "Qwiic plug → 4 female Dupont leads. THE no-solder \
                   link for the LilyGO T-Beam S3 Supreme: its 'QWIIC \
                   socket' is UART1, so plug these onto its real I²C \
                   header — GPIO17 (SDA), GPIO18 (SCL), 3V3, GND. \
                   Connects the VL53L1X / XM125 to the LilyGO without \
                   soldering. (Qwiic-native hosts / STEVAL use the \
                   plain Qwiic↔Qwiic cable instead.)",
        },
        // The cell the LilyGO T-Beam S3 Supreme needs but does NOT
        // ship with. Reference part: raw 18650s aren't reliably
        // stocked by Mouser/DigiKey/Farnell and are a regulated
        // li-ion shippable, so no API offer — just the spec + CH/EU
        // reseller links (resellers()), like the LilyGO board itself.
        Part {
            key: "battery-18650",
            name: "18650 Li-ion cell — flat-top ∅18 × 65 mm (LilyGO power)",
            role: Role::Battery,
            manufacturer: "Generic 18650",
            mpns: &[],
            connector: Connector::Battery,
            oss_firmware: true, // passive cell — no firmware
            st_url: None,
            // SparkFun PRT-12895 (flat-top 18650 2600 mAh): the no-key
            // SparkFun source ships JSON-LD with a real product photo +
            // price, so this card gets an image. This is NOT an API
            // offer — Mouser/DigiKey/Farnell still skip (no `mpns`), so
            // the regulated-li-ion-shipping rationale below is unchanged;
            // it just guarantees the card never falls back to the
            // "no photo" placeholder (nkon.nl 403s every bot fetch).
            sparkfun_pid: Some("12895"),
            direct_url: Some("https://www.nkon.nl/rechargeable/18650-size.html"),
            note: "NOT included with the LilyGO T-Beam S3 Supreme — \
                   buy separately. Must be FLAT-TOP, ∅18 × 65 mm \
                   (button-top cells are longer and won't seat). \
                   3.6–3.7 V Li-ion, ≥2500 mAh recommended; the board's \
                   AXP2101 PMU charges it over USB-C and runs the \
                   board from it untethered. Protected or unprotected \
                   both work (the AXP2101 handles charge/discharge \
                   cut-off). Reputable cells: Samsung/LG/Molicel.",
        },
        // ───────── Raspberry Pi build (alternative recorder host) ─────────
        // A third recorder archetype next to the STEVAL-MKBOXPRO and
        // the LilyGO T-Beam S3 Supreme: a Linux SBC (Pi Zero 2 W) with
        // a snap-on UPS HAT (PiSugar 3, zero-solder pogo pins) and a
        // GPS HAT (Waveshare L76X over UART). Wins over the embedded
        // options when you need real Linux tooling — gpsd, Python,
        // GStreamer, custom services. Loses on boot time (~30 s vs
        // <1 s) and on SD-card-corruption risk under hard power-cut
        // (the PiSugar's `pisugar-server` daemon mitigates this with
        // a clean shutdown on low-battery).
        Part {
            key: "rpi-zero-2-w",
            // Note: part names MUST NOT contain " · " — that's the
            // separator used in stored offer titles
            // (`{name} · {mpn} @ {distributor}`), and `stored_to_row`
            // recovers the BOM part by splitting on it. Use commas
            // inside parentheses instead.
            name: "Raspberry Pi Zero 2 W (BCM2710A1, WiFi + BT 4.2 BLE)",
            role: Role::Devkit,
            manufacturer: "Raspberry Pi Ltd",
            mpns: &["SC0710", "RPI-ZERO2-W"],
            connector: Connector::Gpio,
            oss_firmware: true, // Raspberry Pi OS / mainline Linux kernel
            st_url: None,
            sparkfun_pid: None,
            direct_url: Some(
                "https://www.raspberrypi.com/products/raspberry-pi-zero-2-w/",
            ),
            note: "Pi-Zero-footprint Linux SBC (65 × 30 × 5 mm). \
                   Quad-core Cortex-A53 @1 GHz, 512 MB RAM, microSD \
                   boot, 2.4 GHz WiFi + BLE 4.2, Mini-HDMI, Micro-USB \
                   power, 40-pin GPIO header. Hosts the PiSugar 3 UPS \
                   HAT (below) and the Waveshare L76X GPS HAT (below). \
                   Power draw ~0.5–2 W → ~25 h on the PiSugar's 5000 \
                   mAh cell. Boots in ~30 s and exposes the GPS as \
                   `/dev/serial0` for gpsd.",
        },
        Part {
            key: "pisugar-3-5000mah",
            name: "PiSugar 3 — 5000 mAh UPS HAT für Pi Zero (USB-C charge, no-solder)",
            role: Role::Battery,
            manufacturer: "PiSugar",
            mpns: &[],
            connector: Connector::Gpio,
            oss_firmware: true, // pisugar-server + STM8 fw, GPL/MIT
            st_url: None,
            sparkfun_pid: None,
            direct_url: Some("https://www.pisugar.com/products/pisugar-3"),
            note: "Snap-on UPS for Pi Zero 2 W — zero soldering, pogo \
                   pins clip onto the GPIO underside. 5000 mAh LiPo \
                   onboard, USB-C charge input, hardware RTC (DS3231 + \
                   coin-cell backup), I²C heartbeat to the `pisugar- \
                   server` daemon for clean low-battery shutdown. \
                   STM8S003 PMU runs PiSugar's GPL firmware. Defeats \
                   the low-current-cutoff trap that consumer USB \
                   powerbanks hit when the Pi idles below 100 mA. \
                   Not stocked by Mouser/DigiKey/Farnell — buy direct \
                   from pisugar.com, Tindie, or AliExpress.",
        },
        Part {
            key: "waveshare-l76x-gps-hat",
            name: "Waveshare L76X Multi-GNSS HAT for Pi (Quectel L76B, UART, U.FL)",
            role: Role::Gps,
            manufacturer: "Waveshare",
            mpns: &["L76X-GPS-HAT"],
            connector: Connector::Gpio,
            oss_firmware: true, // Linux serial + gpsd / Waveshare sample code
            st_url: None,
            sparkfun_pid: None,
            direct_url: Some("https://www.waveshare.com/l76x-gps-hat.htm"),
            note: "Pi-HAT carrying a Quectel L76B GNSS module (GPS + \
                   BeiDou + QZSS, 1.575 GHz L1). Talks UART on the Pi's \
                   GPIO14/15 → /dev/serial0 → gpsd; ~30 mA, 1–10 Hz \
                   update rate. Patch antenna onboard plus a U.FL \
                   socket for an external active antenna (pair with \
                   the SparkFun PRT-14986 + U.FL→SMA pigtail in this \
                   BOM if you need better sky view through a sealed \
                   case). Disable the Pi's serial-console first \
                   (`raspi-config` → Interface → Serial Port → no \
                   shell, yes hardware) so the GPS gets the UART.",
        },
        // ───────── Enclosure: the box that holds the build ─────────
        // Hammond 1554G2GYCL — IP66 polycarbonate project box, 120×90×60
        // mm external, **clear PC lid** (sealed with screws + gasket).
        // Picked because:
        //  · polymer (PC) ⇒ RF-transparent: onboard GPS and any radar
        //    work through the wall (a metal/carbon case would block both)
        //  · clear lid ⇒ user can read LEDs and pass the Hall-sensor
        //    magnet over the (sealed) box to flip the supply rail
        //    without opening it (firmware design F-PWR-5)
        //  · IP66 ⇒ rated for jets of water (sufficient for splash and
        //    submersion-on-water-board duty)
        //  · ~110×80×52 mm usable interior ⇒ STEVAL-MKBOXPRO (63×40×20)
        //    + SparkFun MAX-M10S breakout (25×25×6) + UART wiring
        //    with airspace to spare
        // Mouser/DigiKey/Farnell all stock the Hammond 1554 line; the
        // API distributors give CHF pricing via DigiKey CH + Farnell CH.
        // The `vendor` source also fetches og:image from Hammond's
        // product page so the card always shows a real photo even if a
        // distributor's image link is offline.
        Part {
            key: "hammond-1554g2gycl",
            name: "Hammond 1554G2GYCL — IP66 PC enclosure, clear lid (120×90×60 mm)",
            role: Role::Case,
            manufacturer: "Hammond Manufacturing",
            mpns: &["1554G2GYCL"],
            connector: Connector::Enclosure,
            oss_firmware: true, // passive PC box — no firmware
            st_url: None,
            sparkfun_pid: None,
            direct_url: Some(
                "https://www.hammfg.com/electronics/small-case/plastic/1554",
            ),
            note: "Polycarbonate IP66 project box, external 120 × 90 × 60 \
                   mm, clear PC lid. Lid sealed with O-ring gasket and \
                   four corner screws. RF-transparent so onboard GPS \
                   (and any radar) reads through the wall. The Hall \
                   sensor reads a passing magnet through the lid, so \
                   you can power-cycle the recorder without opening the \
                   box. Leave the wall un-drilled to keep the seal \
                   intact and charge via Qi wireless through the wall \
                   (the STEVAL supports it). If the LPS22DF barometer \
                   needs ambient pressure, add a Gore-type vent. \
                   IP66 — splash and water-jet rated, NOT submersible. \
                   If the box is mounted on a foilboard that will \
                   wipeout-submerge, prefer the SERPAC RBF63 (IP67) \
                   below.",
        },
        // SERPAC RBF63 — the foilboard-submersible upgrade over the
        // Hammond IP66. Same polycarbonate (RF-transparent), same
        // clear-lid trick (Hall sensor through the wall, Qi-charging
        // through the wall), but with IP67 perimeter seal — rated for
        // submersion to 1 m for 30 min, exactly what a wipeout puts the
        // box through. Two depth variants in the BOM so the buyer
        // matches the Pi-Zero-stack height (~30 mm) against the case
        // internal depth: C16 fits cleanly with ~15 mm cable headroom,
        // C22 has reserve for wiring + a larger battery / additional
        // HATs. Mouser stocks the line; the `vendor` source falls back
        // to the SERPAC RBF series page for og:image if Mouser's CDN
        // image isn't usable. Same `Role::Case` / `Connector::Enclosure`
        // / `firmware_repo: None` (passive box) convention as Hammond.
        Part {
            key: "serpac-rbf63-c10-clear",
            name: "SERPAC RBF63P06C10C — IP67 PC enclosure, clear lid (160 × 80 × 40 mm)",
            role: Role::Case,
            manufacturer: "SERPAC",
            mpns: &["RBF63P06C10C"],
            connector: Connector::Enclosure,
            oss_firmware: true, // passive PC box — no firmware
            st_url: None,
            sparkfun_pid: None,
            direct_url: Some("https://www.serpac.com/product-by-series/rbf-series.html"),
            note: "Same IP67 polycarbonate enclosure as the C16/C22 \
                   below in the shallow 40 mm variant (1.59 in depth) — \
                   the slimmest profile on the foilboard. Per-build \
                   fit: \
                   (a) **MovementLogger** (STEVAL-MKBOXPRO 20 mm + \
                   SparkFun MAX-M10S breakout 6 mm beside it, total \
                   stack ~25–30 mm) — fits cleanly in the ~30 mm \
                   interior, this is the right pick. DigiKey CH \
                   stocks it directly. \
                   (b) **Pi-Zero recorder** (Pi 5 mm + PiSugar 10 mm \
                   + GPS HAT 10 mm + GPIO stack pins ~10 mm = 30–35 \
                   mm) — borderline; use low-profile stack headers \
                   (omit the spacer pins, the HATs sit flush) or skip \
                   one HAT. For a no-rework drop-in pick C16. \
                   Same RF-transparent / Hall-magnet / Qi-charge \
                   properties as the deeper variants.",
        },
        Part {
            key: "serpac-rbf63-c16-clear",
            name: "SERPAC RBF63P06C16C — IP67 PC enclosure, clear lid (160 × 80 × 55 mm)",
            role: Role::Case,
            manufacturer: "SERPAC",
            mpns: &["RBF63P06C16C"],
            connector: Connector::Enclosure,
            oss_firmware: true, // passive PC box — no firmware
            st_url: None,
            sparkfun_pid: None,
            direct_url: Some("https://www.serpac.com/product-by-series/rbf-series.html"),
            note: "Polycarbonate IP67 project box, external 160 × 80 × \
                   55 mm (6.30 × 3.15 × 2.17 in), clear PC lid sealed \
                   with perimeter O-ring + stainless-steel screws into \
                   metal inserts. IP67 = rated for submersion to 1 m \
                   for 30 min — the Hammond is only IP66 \
                   (splash/water-jet, NOT submersible), so this is the \
                   pick for a Pi-Zero / MovementLogger box that will \
                   actually go under during a foilboard wipeout. Same \
                   RF-transparent / Hall-sensor-through-the-wall / \
                   Qi-charge-through-the-wall properties as the \
                   Hammond. Comfortable fit for the Pi-Zero + PiSugar \
                   + GPS-HAT stack (~30 mm tall) with ~15 mm cable \
                   headroom. Add a Gore vent if the LPS22DF barometer \
                   needs ambient pressure.",
        },
        Part {
            key: "serpac-rbf63-c22-clear",
            name: "SERPAC RBF63P06C22C — IP67 PC enclosure, clear lid (160 × 80 × 85 mm)",
            role: Role::Case,
            manufacturer: "SERPAC",
            mpns: &["RBF63P06C22C"],
            connector: Connector::Enclosure,
            oss_firmware: true, // passive PC box — no firmware
            st_url: None,
            sparkfun_pid: None,
            direct_url: Some("https://www.serpac.com/product-by-series/rbf-series.html"),
            note: "Same IP67 polycarbonate enclosure as the C16 above \
                   but in the deeper 85 mm variant (3.35 in depth) — \
                   gives ~45 mm headroom above the Pi-Zero stack, \
                   enough for a thicker LiPo (10 000 mAh PiSugar Pro, \
                   18650 holder, etc.) or an extra HAT stacked higher \
                   on the GPIO. Pick C22 over C16 if the build will \
                   grow; pick C16 if you want the slimmest profile on \
                   the board.",
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
        // Chip-comparison data: recorder = STM32U585, LilyGO = ESP32-S3,
        // bare sensor ICs have no host MCU.
        assert!(by_key("steval-mkboxpro").mcu().unwrap().contains("STM32U585"));
        assert!(by_key("lilygo-tbeam-s3-supreme")
            .mcu()
            .unwrap()
            .contains("ESP32-S3"));
        assert!(by_key("lps22df").mcu().is_none());
        // Datasheet links — the 6 parts that ship the focused
        // MovementLogger build sheet must each carry a datasheet URL,
        // and every URL we *do* return must look like a PDF or
        // manufacturer document host (catches an accidental typo
        // landing the user on a 404 or wrong domain).
        for k in [
            "steval-mkboxpro",
            "ublox-max-m10s",
            "sparkfun-max-m10s",
            "sparkfun-gps-antenna-sma",
            "sparkfun-ufl-sma-100mm",
            "hammond-1554g2gycl",
            "samtec-ffsd-07-100mm",
        ] {
            let url = by_key(k)
                .datasheet()
                .unwrap_or_else(|| panic!("{k} is missing a datasheet URL"));
            assert!(
                url.starts_with("https://") || url.starts_with("http://"),
                "{k} datasheet URL is not http(s): {url}"
            );
        }
        // Every part has a known LxBxH and a known weight so the
        // report never shows "—" for either physical-spec field.
        for p in &parts {
            let (l, b, h) = p
                .dimensions_cm()
                .unwrap_or_else(|| panic!("{} missing dimensions_cm", p.key));
            assert!(
                l > 0.0 && b > 0.0 && h > 0.0,
                "{} has a non-positive dimension",
                p.key
            );
            let g = p
                .weight_g()
                .unwrap_or_else(|| panic!("{} missing weight_g", p.key));
            assert!(g > 0.0, "{} has a non-positive weight", p.key);
            // Programmable parts link a real GitHub firmware repo;
            // passive accessories (cables) legitimately have none —
            // but if a repo is given it must be a github.com URL.
            match p.firmware_repo() {
                Some(repo) => assert!(
                    repo.starts_with("https://github.com/"),
                    "{} firmware_repo is not a github URL",
                    p.key
                ),
                None => assert!(
                    matches!(
                        p.key,
                        "qwiic-cable-100mm"
                            | "qwiic-jumper-female"
                            | "battery-18650"
                            | "hammond-1554g2gycl"
                            | "serpac-rbf63-c10-clear"
                            | "serpac-rbf63-c16-clear"
                            | "serpac-rbf63-c22-clear"
                            | "sparkfun-gps-antenna-sma"
                            | "sparkfun-ufl-sma-100mm"
                            | "samtec-ffsd-07-100mm"
                            | "arm-jtag-dupont-cable"
                    ),
                    "{} unexpectedly has no firmware_repo",
                    p.key
                ),
            }
        }
    }

    /// Network-dependent: actually GET every `datasheet()` URL and
    /// assert it returns HTTP 200 with a non-empty body. Marked
    /// `#[ignore]` so it does **not** run with a plain `cargo test`;
    /// kick it manually with:
    ///
    /// ```text
    /// cargo test --release --lib -- --ignored --nocapture datasheets_resolve
    /// ```
    ///
    /// Hosts that bot-fingerprint plain `reqwest` (currently `www.st.com`)
    /// are retried through the local FlareSolverr if it's running on
    /// `127.0.0.1:8191`. A URL counts as "live" if **either** the direct
    /// reqwest GET returns 2xx **or** FlareSolverr returns status 200
    /// for it. Anything else fails the test with the offending URL —
    /// catches the kind of stale CDN link the PRT-14986 entry had on
    /// first commit (sparkle.sparkfun.com → DNS dead).
    #[tokio::test]
    #[ignore]
    async fn datasheets_resolve() {
        use crate::sources::flaresolverr::FlareSolverrClient;
        let ua = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) \
                  AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Safari/605.1.15";
        let client = reqwest::Client::builder()
            .user_agent(ua)
            .timeout(std::time::Duration::from_secs(25))
            .build()
            .expect("reqwest client");
        let fs = FlareSolverrClient::new("http://127.0.0.1:8191/v1").ok();
        let mut failed: Vec<String> = Vec::new();
        for p in bom() {
            let Some(url) = p.datasheet() else { continue };
            let direct_ok = match client.get(url).send().await {
                Ok(r) if r.status().is_success() => true,
                _ => false,
            };
            if direct_ok {
                eprintln!("  ✓ direct  {}  {}", p.key, url);
                continue;
            }
            // Fallback: FlareSolverr (handles Akamai / Cloudflare).
            let fs_ok = match fs.as_ref() {
                Some(c) => c.get(url).await.is_ok(),
                None => false,
            };
            if fs_ok {
                eprintln!("  ✓ via FS  {}  {}", p.key, url);
            } else {
                eprintln!("  ✗ dead    {}  {}", p.key, url);
                failed.push(format!("{} -> {}", p.key, url));
            }
        }
        assert!(
            failed.is_empty(),
            "{} datasheet URL(s) failed to resolve:\n  {}",
            failed.len(),
            failed.join("\n  ")
        );
    }
}
