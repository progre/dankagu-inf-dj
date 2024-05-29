use std::mem::size_of;
use std::thread::sleep;
use std::time::Duration;

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

pub fn send_inputs(up: bool, input_stroke_ms: u64) {
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
    let extra_info = unsafe { GetMessageExtraInfo() }.0 as usize;
    let key_down = INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: key,
                wScan: scan,
                dwFlags: KEYEVENTF_EXTENDEDKEY | KEYBD_EVENT_FLAGS::default(),
                time: 0,
                dwExtraInfo: extra_info,
            },
        },
    };
    let key_up = INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: key,
                wScan: scan,
                dwFlags: KEYEVENTF_EXTENDEDKEY | KEYEVENTF_KEYUP,
                time: 0,
                dwExtraInfo: extra_info,
            },
        },
    };
    if input_stroke_ms == 0 {
        let ret = unsafe { SendInput(&[key_down, key_up], size_of::<INPUT>() as i32) };
        assert_eq!(ret, 2);
    } else {
        let ret = unsafe { SendInput(&[key_down], size_of::<INPUT>() as i32) };
        assert_eq!(ret, 1);
        sleep(Duration::from_millis(input_stroke_ms));
        let ret = unsafe { SendInput(&[key_up], size_of::<INPUT>() as i32) };
        assert_eq!(ret, 1);
    }
}
