// /src/rtc.rs
use embassy_stm32::pac::{PWR, RCC, RTC};

#[derive(Debug, Clone, Copy, defmt::Format)]
pub struct GsmTime{
    pub year: u8,
    pub month: u8,
    pub day: u8,
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
}

/// RTC control using LSE/LSI as clock source.
pub struct RtcControl {
    _private: (),
}

impl RtcControl {
    /// Initialize RTC; uses LSI (~37 kHz) as source.
    pub fn init() -> Self {
        // Critical section not strictly required if called in main before tasks
        
        // Enable PWR clock and backup access
        RCC.apb1enr().modify(|w| w.set_pwren(true));
        PWR.cr().modify(|w| w.set_dbp(true));

        // Enable LSI
        RCC.csr().modify(|w| w.set_lsion(true));
        while !RCC.csr().read().lsirdy() {}

        // Select LSI, enable RTC
        // Note: RTCSEL is often in BDCR
        RCC.bdcr().modify(|w| {
            w.set_rtcsel(embassy_stm32::pac::rcc::vals::Rtcsel::LSI);
            w.set_rtcen(true);
        });

        let rtc = RTC;

        // Disable write protection
        rtc.wpr().write(|w| w.set_key(0xCA));
        rtc.wpr().write(|w| w.set_key(0x53));

        // Clear RSF
        rtc.isr().modify(|w| w.set_rsf(false));

        // Enter init mode
        rtc.isr().modify(|w| w.set_init(true));
        while !rtc.isr().read().initf() {}

        // Configure prescalers for ~37kHz -> 1Hz
        // Synch = 0x0120 (288), Asynch = 0x7F (127) => 40kHz approx correction
        rtc.prer().modify(|w| {
            w.set_prediv_a(0x7F);
            w.set_prediv_s(0x0120);
        });

        // Exit init mode
        rtc.isr().modify(|w| w.set_init(false));

        // Re-enable write protection
        rtc.wpr().write(|w| w.set_key(0xFF));

        RtcControl { _private: () }
    }

    pub fn set_time(&mut self, time: GsmTime) {
        let rtc = RTC;

        rtc.wpr().write(|w| w.set_key(0xCA));
        rtc.wpr().write(|w| w.set_key(0x53));

        rtc.isr().modify(|w| w.set_init(true));
        while !rtc.isr().read().initf() {}

        // BCD conversion
        let bcd = |v: u8| ((v / 10) << 4) | (v % 10);

        rtc.dr().write(|w| {
            w.set_dt(bcd(time.day));
            w.set_du(bcd(time.day) & 0xF); // Actually PAC handles BCD splitting usually, but simplified here
            // Note: STM32F0 PAC usually expects pre-formatted BCD in bits or raw values depending on crate version.
            // Using generic manual bit shifting based on original code logic:
            
            // Re-implementing based on standard registers
            w.set_dt(time.day / 10);
            w.set_du(time.day % 10);
            w.set_mt((time.month / 10) > 0);
            w.set_mu(time.month % 10);
            w.set_yt(time.year / 10);
            w.set_yu(time.year % 10);
        });

        rtc.tr().write(|w| {
            w.set_ht(time.hour / 10);
            w.set_hu(time.hour % 10);
            w.set_mnt(time.minute / 10);
            w.set_mnu(time.minute % 10);
            w.set_st(time.second / 10);
            w.set_su(time.second % 10);
        });

        rtc.isr().modify(|w| w.set_init(false));
        rtc.wpr().write(|w| w.set_key(0xFF));
    }

    pub fn get_time(&self) -> GsmTime {
        let rtc = RTC;
        rtc.isr().modify(|w| w.set_rsf(false));
        while !rtc.isr().read().rsf() {}

        let tr = rtc.tr().read();
        let dr = rtc.dr().read();

        let day = dr.dt() * 10 + dr.du();
        let month = (dr.mt() as u8) * 10 + dr.mu();
        let year = dr.yt() * 10 + dr.yu();
        
        let hour = tr.ht() * 10 + tr.hu();
        let minute = tr.mnt() * 10 + tr.mnu();
        let second = tr.st() * 10 + tr.su();

        GsmTime { year, month, day, hour, minute, second }
    }
}