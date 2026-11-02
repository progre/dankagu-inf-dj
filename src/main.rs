mod direct_input;
mod keyboard_and_mouse;

use std::ffi::c_void;
use std::time::Duration;
use std::time::Instant;

use anyhow::Error;
use anyhow::Result;
use clap::ArgGroup;
use clap::Parser;
use direct_input::acquire;
use direct_input::create_device;
use direct_input::get_state;
use direct_input::get_state_timeout;
use direct_input::init_event_notification;
use keyboard_and_mouse::press_key;
use keyboard_and_mouse::release_key;
use keyboard_and_mouse::send_inputs;
use windows::Win32::Devices::HumanInterfaceDevice::IDirectInputDevice8W;
use windows::Win32::Foundation::HANDLE;
use windows::Win32::System::Threading::INFINITE;
use windows::Win32::UI::WindowsAndMessaging::SystemParametersInfoW;
use windows::Win32::UI::WindowsAndMessaging::SPI_GETKEYBOARDDELAY;
use windows::Win32::UI::WindowsAndMessaging::SPI_GETKEYBOARDSPEED;
use windows::Win32::UI::WindowsAndMessaging::SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS;

#[derive(clap::Parser)]
#[command(group(ArgGroup::new("conflicts").arg("refresh_rate").conflicts_with("stroke")))]
struct Args {
    #[arg(short, long, default_value_t = 1000)]
    threshold: u16,
    #[arg(short, long, value_name = "STROKE_MILLISECONDS")]
    stroke: Option<u16>,
    #[arg(short, long, default_value_t = 60)]
    refresh_rate: u8,
    /// スクラッチを回している間、キーを連打せず押しっぱなしにする
    #[arg(long, conflicts_with_all = ["threshold", "stroke", "refresh_rate"])]
    hold: bool,
    /// --hold 指定時、回転が止まってからキーを離すまでの時間
    #[arg(
        long,
        value_name = "RELEASE_MILLISECONDS",
        default_value_t = 100,
        requires = "hold"
    )]
    hold_release: u16,
    /// --hold 指定時、押してからキーリピートが始まるまでの時間 [default: Windows のキーボード設定]
    #[arg(long, value_name = "MILLISECONDS", requires = "hold")]
    hold_repeat_delay: Option<u16>,
    /// --hold 指定時、キーリピートの間隔 [default: Windows のキーボード設定]
    #[arg(long, value_name = "MILLISECONDS", requires = "hold")]
    hold_repeat_interval: Option<u16>,
}

fn diff_x(prev_x: &mut i16, current_x: i16) -> i32 {
    // i16 -32768 .. -16385,-16384 ..    -1,0 .. 16384,16385 .. 32767
    // u16  32768 ..  49151, 49152 .. 65535,0 .. 16384,16385 .. 32767
    // -16384 ~ 16384 は i16 で処理、-32768 ~ -16385, 16385 ~ 32767 は u16 で処理
    let diff = if (-16384..=16384).contains(&current_x) {
        current_x as i32 - *prev_x as i32
    } else {
        current_x as u16 as i32 - *prev_x as u16 as i32
    };
    *prev_x = current_x;
    diff
}

fn main() -> Result<(), Error> {
    let args = Args::parse();

    let device = create_device();
    let event = init_event_notification(&device);
    acquire(&device).unwrap();

    let mut x = get_state(&device, event)?.lX as i16;

    if args.hold {
        let (default_delay, default_interval) = keyboard_repeat_settings();
        return hold_loop(
            &device,
            event,
            x,
            args.hold_release as u64,
            args.hold_repeat_delay.map_or(default_delay, |x| x as u64),
            args.hold_repeat_interval
                .map_or(default_interval, |x| x as u64),
        );
    }

    let mut time = Instant::now();
    let mut acc_x = 0i32;

    let stroke = {
        if let Some(stroke) = args.stroke {
            stroke as u64
        } else if args.refresh_rate == 0 {
            0
        } else {
            (1000.0 / args.refresh_rate as f64).ceil() as u64
        }
    };
    let threshold = args.threshold as i32;

    loop {
        let state = get_state(&device, event)?;
        let diff_x = diff_x(&mut x, state.lX as i16);
        if diff_x == 0 {
            continue;
        }
        // IIDX は時計回りで下に移動
        // 左右どちらのサイドでもそうなので、リストが右ではなく左であっても合わせるのがよさそう
        let expired = time.elapsed() > Duration::from_millis(100);
        time = Instant::now();
        // 時間が経過した || 回転方向が変わった
        if expired || diff_x * acc_x < 0 {
            acc_x = diff_x;
            send_inputs(diff_x > 0, stroke);
            continue;
        }
        acc_x += diff_x;
        while acc_x.abs() > threshold {
            send_inputs(diff_x > 0, stroke);
            acc_x += if diff_x < 0 { threshold } else { -threshold };
        }
    }
}

/// Windows のキーボード設定 (リピート開始までの時間, リピート間隔) を取得する
fn keyboard_repeat_settings() -> (u64, u64) {
    let mut delay = 1u32; // 0..=3 (250ms..=1000ms)
    let mut speed = 31u32; // 0..=31 (約 2.5回/秒..=約 30回/秒)
    unsafe {
        let _ = SystemParametersInfoW(
            SPI_GETKEYBOARDDELAY,
            0,
            Some(&mut delay as *mut _ as *mut c_void),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        );
        let _ = SystemParametersInfoW(
            SPI_GETKEYBOARDSPEED,
            0,
            Some(&mut speed as *mut _ as *mut c_void),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        );
    }
    let delay_ms = 250 * (delay.min(3) as u64 + 1);
    let rate_hz = 2.5 + speed.min(31) as f64 * (30.0 - 2.5) / 31.0;
    (delay_ms, (1000.0 / rate_hz).round() as u64)
}

struct Held {
    /// true: 上, false: 下
    up: bool,
    /// 最後に回転を検知した時刻
    last_move: Instant,
    /// 次にキーリピートを送る時刻
    next_repeat: Instant,
}

fn hold_loop(
    device: &IDirectInputDevice8W,
    event: HANDLE,
    mut x: i16,
    release_ms: u64,
    repeat_delay_ms: u64,
    repeat_interval_ms: u64,
) -> Result<(), Error> {
    let release_after = Duration::from_millis(release_ms);
    let repeat_delay = Duration::from_millis(repeat_delay_ms);
    let repeat_interval = Duration::from_millis(repeat_interval_ms.max(1));
    let mut held: Option<Held> = None;

    loop {
        // 期限の処理: 回転が止まっていれば離す、押し続けていればキーリピートを送る
        // (SendInput の押下は実キーボードと違い自動でリピートされないため、自前で送る)
        if let Some(h) = &mut held {
            let now = Instant::now();
            if now.duration_since(h.last_move) >= release_after {
                release_key(h.up);
                held = None;
            } else {
                while h.next_repeat <= now {
                    press_key(h.up);
                    h.next_repeat += repeat_interval;
                }
            }
        }

        let timeout_ms = match &held {
            None => INFINITE,
            Some(h) => {
                let deadline = (h.last_move + release_after).min(h.next_repeat);
                deadline
                    .saturating_duration_since(Instant::now())
                    .as_millis()
                    .min(u32::MAX as u128 - 1) as u32
            }
        };
        let Some(state) = get_state_timeout(device, event, timeout_ms)? else {
            continue;
        };
        let diff_x = diff_x(&mut x, state.lX as i16);
        if diff_x == 0 {
            continue;
        }
        // IIDX は時計回りで下に移動
        let up = diff_x > 0;
        let now = Instant::now();
        match &mut held {
            Some(h) if h.up == up => h.last_move = now,
            _ => {
                if let Some(h) = &held {
                    // 回転方向が変わったので押し直す
                    release_key(h.up);
                }
                press_key(up);
                held = Some(Held {
                    up,
                    last_move: now,
                    next_repeat: now + repeat_delay,
                });
            }
        }
    }
}
