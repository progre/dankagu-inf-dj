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
use direct_input::init_event_notification;
use keyboard_and_mouse::send_inputs;

#[derive(clap::Parser)]
#[command(group(ArgGroup::new("conflicts").arg("refresh_rate").conflicts_with("stroke")))]
struct Args {
    #[arg(short, long, default_value_t = 1000)]
    threshold: u16,
    #[arg(short, long, value_name = "STROKE_MILLISECONDS")]
    stroke: Option<u16>,
    #[arg(short, long, default_value_t = 60)]
    refresh_rate: u8,
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

    let mut time = Instant::now();
    let mut x = get_state(&device, event)?.lX as i16;
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
