mod direct_input;

use std::mem::size_of;
use std::thread::sleep;
use std::time::Duration;
use std::time::Instant;

use anyhow::Error;
use anyhow::Result;
use direct_input::create_device;
use direct_input::get_state;
use windows::Win32::UI::Input::KeyboardAndMouse::SendInput;
use windows::Win32::UI::Input::KeyboardAndMouse::INPUT;
use windows::Win32::UI::Input::KeyboardAndMouse::INPUT_0;
use windows::Win32::UI::Input::KeyboardAndMouse::INPUT_KEYBOARD;
use windows::Win32::UI::Input::KeyboardAndMouse::KEYBDINPUT;
use windows::Win32::UI::Input::KeyboardAndMouse::KEYBD_EVENT_FLAGS;
use windows::Win32::UI::Input::KeyboardAndMouse::KEYEVENTF_EXTENDEDKEY;
use windows::Win32::UI::Input::KeyboardAndMouse::KEYEVENTF_KEYUP;
use windows::Win32::UI::Input::KeyboardAndMouse::VK_DOWN;
use windows::Win32::UI::Input::KeyboardAndMouse::VK_UP;
use windows::Win32::UI::WindowsAndMessaging::GetMessageExtraInfo;

fn send_inputs(up: bool) {
    // 選曲はマウスホイールのほうが安定するが、メニューはカーソルでしか操作できない
    // let mut pos = Default::default();
    // unsafe { GetCursorPos(&mut pos) }.unwrap();
    // let inputs = [INPUT {
    //     r#type: INPUT_MOUSE,
    //     Anonymous: INPUT_0 {
    //         mi: MOUSEINPUT {
    //             dx: 0,
    //             dy: 0,
    //             mouseData: (WHEEL_DELTA as i32 * if up { 1 } else { -1 }) as u32,
    //             dwFlags: MOUSEEVENTF_WHEEL,
    //             time: 0,
    //             dwExtraInfo: unsafe { GetMessageExtraInfo() }.0 as usize,
    //         },
    //     },
    // }];
    let key = if up { VK_UP } else { VK_DOWN };
    let scan = if up { 0xe048 } else { 0xe050 };
    let inputs = [INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: key,
                wScan: scan,
                dwFlags: KEYEVENTF_EXTENDEDKEY | KEYBD_EVENT_FLAGS::default(),
                time: 0,
                dwExtraInfo: unsafe { GetMessageExtraInfo() }.0 as usize,
            },
        },
    }];
    assert_eq!(inputs.len() as u32, unsafe {
        SendInput(&inputs, size_of::<INPUT>() as i32)
    });
    sleep(Duration::from_millis(16));
    let inputs = [INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: key,
                wScan: scan,
                dwFlags: KEYEVENTF_EXTENDEDKEY | KEYEVENTF_KEYUP,
                time: 0,
                dwExtraInfo: unsafe { GetMessageExtraInfo() }.0 as usize,
            },
        },
    }];
    assert_eq!(inputs.len() as u32, unsafe {
        SendInput(&inputs, size_of::<INPUT>() as i32)
    });
    sleep(Duration::from_millis(8));
}

fn main() -> Result<(), Error> {
    let device = create_device();

    let mut time = Instant::now();
    let mut x = get_state(&device)?.lX as i16;
    let mut acc_x = 0i32;

    loop {
        let state = get_state(&device)?;
        let current_x = state.lX as i16;
        // i16 -32768 .. -16385,-16384 ..    -1,0 .. 16384,16385 .. 32767
        // u16  32768 ..  49151, 49152 .. 65535,0 .. 16384,16385 .. 32767
        // -16384 ~ 16384 は i16 で処理、-32768 ~ -16385, 16385 ~ 32767 は u16 で処理
        let diff_x = if (-16384..=16384).contains(&current_x) {
            current_x as i32 - x as i32
        } else {
            current_x as u16 as i32 - x as u16 as i32
        };
        x = current_x;
        if diff_x == 0 {
            continue;
        }
        // IIDX は時計回りで下に移動
        // 左右どちらのサイドでもそうなので、リストが右ではなく左であっても合わせるのがよさそう

        let expired = time.elapsed() > Duration::from_millis(100);
        time = Instant::now();
        // 時間が経過した || 回転方向が変わった
        if expired || diff_x * acc_x < 0 {
            send_inputs(diff_x > 0);
            acc_x = diff_x;
            continue;
        }
        acc_x += diff_x;
        const THRESHOLD: i32 = 3000;
        if acc_x.abs() > THRESHOLD {
            send_inputs(diff_x > 0);
            acc_x += if diff_x < 0 { THRESHOLD } else { -THRESHOLD };
        }
    }
}
