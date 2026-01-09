# Embassy800c - Async Rust GSM Alarm System

**Embassy800c** is an asynchronous embedded application written in Rust using the [Embassy framework](https://embassy.dev/). It runs on an **STM32F051** microcontroller and manages a **SIM800C** GSM module to monitor analog sensors, report alarms via SMS/DTMF, and remotely control relay outputs.

## 🌟 Features

*   **Async Architecture:** Uses the Embassy executor for efficient, non-blocking task management without complex state machines.
*   **Intrusion Monitoring:** Monitors 3 Analog Inputs with configurable voltage window thresholds.
*   **Dual Communication Modes:**
    *   **SMS:** Sends alerts with timestamped snapshots of sensor states (`PPP_<code>_<timestamp>`).
    *   **DTMF:** Makes voice calls and transmits compressed sensor states via DTMF tones. Uses a robust "Retry-Until-Confirmed" logic.
*   **Remote Control:** Parses incoming SMS and DTMF codes to toggle 3 Relay Outputs (e.g., to mirror the state of a remote transmitter).
*   **System Reliability:**
    *   **Watchdog:** 4.5-hour safety timer to reset relays if communication is lost.
    *   **RTC Synchronization:** Syncs internal Real-Time Clock (RTC) with GSM Network Time via `+CCLK`.
    *   **Deduplication:** Prevents spamming alerts for the same event within short windows.
*   **Hardware Abstraction:** Custom async driver for the SIM800C UART interface using DMA.

## 🔌 Hardware Configuration

### Microcontroller
*   **Device:** STM32F051R8
*   **Clock:** 48 MHz (HSE 8MHz + PLL)

### Pinout

| Peripheral | Pin | Function | Description |
| :--- | :--- | :--- | :--- |
| **USART2** | PA2 | TX | SIM800C UART Transmit |
| | PA3 | RX | SIM800C UART Receive |
| **Control** | PC7 | Output | SIM800C Power/Enable Key |
| | PC6 | Output | SIM800C TTL Logic Enable |
| **Sensors** | PA4 | ADC_IN4 | Alarm Input 1 |
| | PA5 | ADC_IN5 | Alarm Input 2 |
| | PA6 | ADC_IN6 | Alarm Input 3 |
| | PA7 | Output | Alarm Pull-up Power |
| **Relays** | PB3 | Output | Alarm Output Relay 1 |
| | PB4 | Output | Alarm Output Relay 2 |
| | PB5 | Output | Alarm Output Relay 3 |
| **Debug** | PA9 | USART1_TX | Log Output (115200 baud) |
| | PA10 | USART1_RX | Log Input |
| **Status** | PC8 | Output | LED 4 (Status) |
| | PC9 | Output | LED 3 (Status) |

### Sensor Logic
The system reads raw ADC values (12-bit). An alarm is registered if the value falls within the intrusion window:
*   **Low Threshold:** 1000
*   **High Threshold:** 1500

## 🚀 Getting Started

### Prerequisites
1.  **Rust Toolchain:** Install Rust via [rustup.rs](https://rustup.rs/).
2.  **Thumbv6 Target:** `rustup target add thumbv6m-none-eabi`
3.  **Probe-rs:** `cargo install probe-rs --features cli`
4.  **Hardware:** ST-Link or compatible debugger connected to SWD pins.

### SIM Card Setup
The device relies on the SIM card's internal phonebook for configuration.
1.  Insert the SIM card into a phone.
2.  Save the target destination phone number(s) to the **SIM Phonebook** (Storage "SM").
3.  The firmware automatically loads the number at **Index 1** as the primary alarm recipient.

### Build and Run

```bash
# Check compilation
cargo check

# Flash and run with logging (Release mode recommended for code size)
cargo run --release
```

*Note: This project uses `defmt` for logging. You need a probe that supports RTT (Real-Time Transfer) to see the logs.*

## 📡 Protocol Details

### Outgoing SMS Format
When `USE_SMS` is enabled, alerts are sent in the following format:
```text
PPP_<DATA>_<TIMESTAMP>
```
*   `PPP`: Message Prefix.
*   `DATA`: 3-character string representing the binary state of the 3 sensors (compressed).
*   `TIMESTAMP`: YY/MM/DD,HH:MM:SS+ZZ (Network time).

### Outgoing DTMF
When calling, the device transmits a 3-digit DTMF code representing the sensor states. It waits for a `#` DTMF tone from the receiver to confirm delivery. If not confirmed, it retries every 10 seconds.

### Incoming Control
*   **SMS:** Sends a command containing `PPP;<code>` to set relays.
*   **DTMF:** During a call, receiving 3 digits sets the relay states locally to match the received code.

## 📂 Project Structure

*   `src/main.rs`: Application entry point, task spawning, and high-level logic loop.
*   `src/sim800.rs`: Async Actor driver for the SIM800C module. Handles AT commands and URC parsing.
*   `src/hardware.rs`: HAL initialization and pin mapping.
*   `src/alarms_handler.rs`: Logic for compressing sensor history (debouncing/stacking).
*   `src/rtc.rs`: STM32F0 RTC register abstraction.

## 🛠️ Dependencies
*   `embassy-stm32`: Hardware Abstraction Layer.
*   `embassy-executor`: Async runtime.
*   `embassy-time`: Timekeeping.
*   `defmt`: High-performance logging.
*   `heapless`: Static friendly data structures.

***
License: GPL v2
