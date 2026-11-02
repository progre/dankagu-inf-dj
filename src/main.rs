mod direct_input;
mod keyboard_and_mouse;

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
        return hold_loop(&device, event, x, args.hold_release as u64);
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

fn hold_loop(
    device: &IDirectInputDevice8W,
    event: HANDLE,
    mut x: i16,
    release_ms: u64,
) -> Result<(), Error> {
    let release_after = Duration::from_millis(release_ms);
    // 押下中のキー (true: 上, false: 下) と最後に回転を検知した時刻
    let mut held: Option<(bool, Instant)> = None;

    loop {
        let timeout_ms = match held {
            None => INFINITE,
            Some((_, last)) => release_after
                .saturating_sub(last.elapsed())
                .as_millis()
                .min(u32::MAX as u128 - 1) as u32,
        };
        let Some(state) = get_state_timeout(device, event, timeout_ms)? else {
            // 一定時間回転がなかったのでキーを離す
            if let Some((up, _)) = held.take() {
                release_key(up);
            }
            continue;
        };
        let diff_x = diff_x(&mut x, state.lX as i16);
        if diff_x == 0 {
            if let Some((up, last)) = held {
                if last.elapsed() >= release_after {
                    release_key(up);
                    held = None;
                }
            }
            continue;
        }
        // IIDX は時計回りで下に移動
        let up = diff_x > 0;
        match held {
            Some((held_up, _)) if held_up == up => {}
            Some((held_up, _)) => {
                // 回転方向が変わったので押し直す
                release_key(held_up);
                press_key(up);
            }
            None => press_key(up),
        }
        held = Some((up, Instant::now()));
    }
}
