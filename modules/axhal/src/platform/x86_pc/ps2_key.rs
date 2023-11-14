use pc_keyboard::{DecodedKey, HandleControl, Keyboard, ScancodeSet1};
use pc_keyboard::layouts::Us104Key;

use x86_64::instructions::port::Port;

use spinlock::SpinNoIrq;

#[cfg(feature = "irq")]
use crate::irq::register_handler;

static KEYBOARD: SpinNoIrq<Keyboard<Us104Key, ScancodeSet1>> = SpinNoIrq::new(Keyboard::new(
    ScancodeSet1::new(),
    Us104Key,
    HandleControl::Ignore,
));

/// 键盘中断号
const KEYBOARD_IRQ_NUM: u8 = 0x21;
/// 键盘输入端口
const KEYBOARD_INPUT_PORT: u16 = 0x60;

use crate::console::put2stdin;

/// 键盘中断处理函数
fn keyboard_irq_handler() {
    let mut port = Port::new(KEYBOARD_INPUT_PORT);
    let scancode = unsafe { port.read() };

    let mut keyboard = KEYBOARD.lock();
    if let Ok(Some(key_event)) = keyboard.add_byte(scancode) {
        if let Some(key) = keyboard.process_keyevent(key_event) {
            if let DecodedKey::Unicode(c) = key {
                put2stdin(c as u8);
            }
        }
    }
}

/// 初始化键盘
pub(super) fn init() {
    #[cfg(feature = "irq")]
    {
        register_handler(KEYBOARD_IRQ_NUM as usize, keyboard_irq_handler);
    }
}