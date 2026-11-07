use crate::gps::gpscontroller::GpsController;
use crate::gps::pps_sync;
use esp_idf_svc::sys::esp_timer_get_time;
use log::info;
use std::net::UdpSocket;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

#[derive(Default, Clone, Copy)]
struct TimeState {
    base_mono_us: i64,
    base_unix_us: i64,
    last_fix_mono_us: i64,
    last_pps_count: u64,
    locked: bool,
    pps_aligned: bool,
}

type SharedTime = Arc<Mutex<TimeState>>;

pub struct NtpServer {
    _updater: Option<thread::JoinHandle<()>>,
    _udp: Option<thread::JoinHandle<()>>,
}

impl NtpServer {
    pub fn start(controller: Arc<GpsController>) -> Self {
        let shared: SharedTime = Arc::new(Mutex::new(TimeState::default()));

        // ----- time keeper -----
        let shared_updater = shared.clone();
        let controller_updater = controller.clone();
        let updater = thread::Builder::new()
            .name("ntp_time".into())
            .stack_size(4096)
            .spawn(move || {
                let mut last_epoch_unix_s: Option<f64> = None;
                let mut last_pps_seen: u64 = pps_sync::pps_pulse_count();

                loop {
                    let data = controller_updater.get_data();
                    let now_mono = unsafe { esp_timer_get_time() };

                    if data.has_fix {
                        if let Some(epoch) = data.epoch_utc {
                            let epoch_unix_s = epoch.to_unix_seconds();
                            let now_pps_count = pps_sync::pps_pulse_count();
                            let last_pps_us = pps_sync::last_pps_timestamp_us();

                            let epoch_changed =
                                last_epoch_unix_s != Some(epoch_unix_s);

                            if epoch_changed {
                                let mut state =
                                    shared_updater.lock().expect("time lock poisoned");

                                // Prefer PPS alignment when a *new* pulse exists.
                                if last_pps_us > 0
                                    && now_pps_count > state.last_pps_count
                                    && last_pps_us as i64 > state.base_mono_us
                                {
                                    state.base_mono_us = last_pps_us as i64;
                                    state.base_unix_us =
                                        (epoch_unix_s * 1_000_000.0) as i64;
                                    state.last_pps_count = now_pps_count;
                                    state.pps_aligned = true;
                                    last_pps_seen = now_pps_count;
                                    //info!("NTP aligned to PPS + GPS epoch");
                                } else {
                                    // Fallback: coarse estimate if PPS absent/stale.
                                    state.base_mono_us = now_mono;
                                    state.base_unix_us =
                                        (epoch_unix_s * 1_000_000.0) as i64;
                                    state.pps_aligned = false;
                                }

                                state.last_fix_mono_us = now_mono;
                                state.locked = true;
                                last_epoch_unix_s = Some(epoch_unix_s);
                            } else {
                                // Refresh staleness timer while fix remains valid.
                                let mut state =
                                    shared_updater.lock().expect("time lock poisoned");
                                state.last_fix_mono_us = now_mono;
                                state.locked = data.has_fix;
                            }
                        }
                    }

                    // Stale after ~5s without fresh UTC.
                    {
                        let mut state =
                            shared_updater.lock().expect("time lock poisoned");
                        if state.locked && now_mono - state.last_fix_mono_us > 5_000_000 {
                            state.locked = false;
                        }
                    }

                    let _ = last_pps_seen;
                    thread::sleep(Duration::from_millis(25));
                }
            })
            .expect("spawn ntp_time");

        // ----- NTP/SNTP UDP server :123 -----
        let shared_udp = shared.clone();
        let udp = thread::Builder::new()
            .name("ntp_udp".into())
            .stack_size(8192)
            .spawn(move || udp_loop(shared_udp))
            .expect("spawn ntp_udp");

        Self {
            _updater: Some(updater),
            _udp: Some(udp),
        }
    }
}

fn current_correction_factor() -> f64 {
    pps_sync::clock_correction_factor()
}

fn now_unix_us(shared: &SharedTime) -> i64 {
    let state = shared.lock().expect("time lock poisoned");
    if state.base_mono_us == 0 {
        return 0;
    }
    let now_mono = unsafe { esp_timer_get_time() };
    let elapsed =
        (now_mono - state.base_mono_us) as f64 * current_correction_factor();
    state.base_unix_us + elapsed as i64
}

fn udp_loop(shared: SharedTime) {
    let sock = match UdpSocket::bind("0.0.0.0:123") {
        Ok(s) => s,
        Err(e) => {
            log::error!("NTP bind :123 failed: {e}");
            return;
        }
    };

    let mut buf = [0u8; 48];

    loop {
        let (n, src) = match sock.recv_from(&mut buf) {
            Ok(x) => x,
            Err(_) => continue,
        };
        if n < 48 {
            continue;
        }
        if buf[0] & 0b0000_0111 != 3 {
            continue;
        }

        let stratum = {
            let state = shared.lock().expect("time lock poisoned");
            if !state.locked {
                16
            } else if state.pps_aligned {
                1
            } else {
                2
            }
        };

        let precision = if stratum == 1 { 0xF6 } else { 0xFA }; // -10 vs -6

        let receive_us = now_unix_us(&shared);

        let mut origin = [0u8; 8];
        origin.copy_from_slice(&buf[40..48]);

        let mut resp = [0u8; 48];
        resp[0] = 0b00_100_100;
        resp[1] = stratum as u8;
        resp[2] = 4;
        resp[3] = precision;

        let ref_id: [u8; 4] = if stratum == 1 {
            *b"PPS\0"
        } else if stratum == 2 {
            *b"GPS\0"
        } else {
            [0; 4]
        };
        resp[12..16].copy_from_slice(&ref_id);

        let ref_ts = unix_us_to_ntp(receive_us);
        resp[16..24].copy_from_slice(&ref_ts.to_be_bytes());
        resp[24..32].copy_from_slice(&origin);

        let recv_ts = unix_us_to_ntp(receive_us);
        resp[32..40].copy_from_slice(&recv_ts.to_be_bytes());

        let tx_ts = unix_us_to_ntp(now_unix_us(&shared));
        resp[40..48].copy_from_slice(&tx_ts.to_be_bytes());

        let _ = sock.send_to(&resp, src);
        info!("Sent an NTP response to {src}");
    }
}

const NTP_UNIX_OFFSET: u64 = 2_208_988_800;

fn unix_us_to_ntp(us: i64) -> u64 {
    if us <= 0 {
        return 0;
    }
    let sec = (us / 1_000_000) as u64 + NTP_UNIX_OFFSET;
    let frac = ((us.rem_euclid(1_000_000)) as u64 * (1u64 << 32)) / 1_000_000;
    (sec << 32) | frac
}