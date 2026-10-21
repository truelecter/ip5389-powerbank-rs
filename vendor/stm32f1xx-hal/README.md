# `stm32f1xx-hal` for the AT32F403A

This is [stm32f1xx-hal](https://github.com/stm32-rs/stm32f1xx-hal) 0.11.0 from crates.io, adapted for the Artery AT32F403A on this board. The AT32F403A peripherals are register-compatible with the STM32F103 except for the clock tree and a few field widths, so the `at32f403a` feature builds the `stm32f103` variant with the changes below.

## Changes

- `src/rcc.rs`: AT32 clock tree behind the `at32f403a` feature. 6-bit PLL multiplier (`at32::pllmult_bits`), the PLL range bit above 72 MHz, the automatic frequency step (`CRM_MISC3`) around the system clock switch, AT32 limits (SYSCLK and HCLK 240 MHz, PCLK1 and PCLK2 120 MHz, ADC 28 MHz, PLL x64) and no flash wait states. Unit tests `rcc_config_at32_240mhz`, `rcc_config_at32_rejects_overclock` and `at32_pllmult_bits` cover it.
- `src/rcc.rs`: `Rcc::reconfigure` (AT32 only) switches the clock tree at runtime: it runs from HSI while the PLL is reprogrammed and leaves the PLL off for a configuration without it. `freeze` shares the programming code with it.
- `src/i2c.rs`: the I2C input clock field is 8 bits wide on the AT32 (up to 120 MHz), and the rise time is clamped to the 6-bit field, as Artery's own driver does.
- `src/timer/hal_1.rs`: `DelayNs::delay_ns` for timer delays treated nanoseconds as microseconds, so every delay was 1000 times too long. Fixed with `nanos_at_least()`. This bug is also in upstream 0.11.0 and master.
- Unused `use` lines for local macros removed from `src/afio.rs` and `src/rcc.rs` (warnings on current Rust).

## Removed

Drivers the firmware doesn't use, with their dependencies: ADC (`adc`, `pacext`), CAN (`can`, `bxcan`), CRC, DAC, serial (`serial`, `embedded-io`) and USB (`usb`, `stm32-usbd`). Also the examples, dev-dependencies and the changelog. To bring one back, copy it from the 0.11.0 crate and re-add its `pub mod` line in `src/lib.rs` and its prelude entries.

## Silicon notes

The chip on this board is revision A (`0x1FFFF7F1` bits 6:4 read 0). Artery's errata ES0002 lists two revision A I2C master bugs with slaves that stretch the clock: the first clock after a stretch can be too short, and a repeated START can fail. The hardware I2C therefore can't talk to the BQ4050 reliably; the firmware uses the bit-banged master in `peripherals/src/soft_i2c.rs` for it.

## Running the tests

The RCC tests run on the host. The `stm32f1` PAC places data in a link section that Mach-O rejects, so on macOS build a temporary copy with `stm32f1 = { default-features = false, features = ["atomics", "critical-section"] }`, then run `cargo test --features at32f403a,medium` on it.
