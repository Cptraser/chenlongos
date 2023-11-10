//! VGA text mode.

extern crate alloc;

use alloc::vec::Vec;

use lazy_init::LazyInit;
use spinlock::SpinNoIrq;
use core::fmt;
use core::fmt::Error;
use core::fmt::Write;

use axlog::ColorCode as ConsoleColorCode;

use crate::mem::PhysAddr;

static VGA: SpinNoIrq<VgaTextMode> = SpinNoIrq::new(VgaTextMode::new());

static mut LEVEL_DEBUG: u8 = 3;

/// The height of the vga text buffer (normally 25 lines).
const VGA_BUFFER_HEIGHT: usize = 25;
/// The width of the vga text buffer (normally 80 columns).
const VGA_BUFFER_WIDTH: usize = 80;
/// The MMIO address of VGA buffer.
const VGA_BASE_ADDR: PhysAddr = PhysAddr::from(0xb_8000);

/// The standard color palette in VGA text mode.
#[allow(dead_code)]
#[derive(Clone, Copy)]
#[repr(u8)]
enum VgaTextColor {
    Black = 0,
    Blue = 1,
    Green = 2,
    Cyan = 3,
    Red = 4,
    Purple = 5,
    Brown = 6,
    Gray = 7,
    DarkGray = 8,
    LightBlue = 9,
    LightGreen = 10,
    LightCyan = 11,
    LightRed = 12,
    LightPurple = 13,
    Yellow = 14,
    White = 15,
}

impl VgaTextColor {
    fn from_console_color(color: ConsoleColorCode) -> VgaTextColor {
        match color {
            ConsoleColorCode::Black => VgaTextColor::Black,
            ConsoleColorCode::Red => VgaTextColor::Red,
            ConsoleColorCode::Green => VgaTextColor::Green,
            ConsoleColorCode::Yellow => VgaTextColor::Brown,
            ConsoleColorCode::Blue => VgaTextColor::Blue,
            ConsoleColorCode::Magenta => VgaTextColor::Purple,
            ConsoleColorCode::Cyan => VgaTextColor::Cyan,
            ConsoleColorCode::White => VgaTextColor::Gray,
            ConsoleColorCode::BrightBlack => VgaTextColor::Gray,
            ConsoleColorCode::BrightRed => VgaTextColor::LightRed,
            ConsoleColorCode::BrightGreen => VgaTextColor::LightGreen,
            ConsoleColorCode::BrightYellow => VgaTextColor::Yellow,
            ConsoleColorCode::BrightBlue => VgaTextColor::LightBlue,
            ConsoleColorCode::BrightMagenta => VgaTextColor::LightPurple,
            ConsoleColorCode::BrightCyan => VgaTextColor::LightCyan,
            ConsoleColorCode::BrightWhite => VgaTextColor::White,
        }
    }
}

/// A combination of a foreground and a background color.
#[derive(Clone, Copy)]
#[repr(transparent)]
struct VgaTextColorCode(u8);

impl VgaTextColorCode {
    /// Create a new `VgaTextColorCode` with the given foreground and background colors.
    const fn new(fg: VgaTextColor, bg: VgaTextColor) -> VgaTextColorCode {
        VgaTextColorCode((bg as u8) << 4 | (fg as u8))
    }
}

/// Character for the VGA text buffer, including an ASCII character and a `VgaTextColorCode`.
struct VgaTextChar(u8, VgaTextColorCode);

/// A structure representing the VGA text buffer.
#[repr(transparent)]
struct VgaTextBuffer {
    chars: [[VgaTextChar; VGA_BUFFER_WIDTH]; VGA_BUFFER_HEIGHT],
}

#[derive(Clone, Copy)]
enum VgaTextSetColor {
    // \x1b, to LeftBrackets
    Start,
    // [, to value or end
    LeftBrackets,
    // number
    Value(u8),
    // m, end
    End,
}

#[derive(Clone, Copy)]
enum VgaTextState {
    PutChar,
    SetColor(VgaTextSetColor),
}

struct VgaTextMode {
    current_x: usize,
    current_y: usize,
    current_color: VgaTextColorCode,
    state: VgaTextState,
    buffer: LazyInit<&'static mut VgaTextBuffer>,
}

impl VgaTextMode {
    const fn new() -> Self {
        Self {
            current_x: 0,
            current_y: 0,
            current_color: VgaTextColorCode::new(VgaTextColor::White, VgaTextColor::Black),
            state: VgaTextState::PutChar,
            buffer: LazyInit::new(),
        }
    }

    fn scroll_up(&mut self, line: usize) {
        if line > VGA_BUFFER_HEIGHT {
            return;
        }

        let buffer = &mut self.buffer.chars;

        let size =
            (VGA_BUFFER_HEIGHT - line) * VGA_BUFFER_WIDTH * core::mem::size_of::<VgaTextChar>();
        let src = &buffer[line][0] as *const VgaTextChar;
        let dst = &mut buffer[0][0] as *mut VgaTextChar;
        unsafe {
            core::ptr::copy(src, dst, size);
        }
        self.current_y -= line;
    }

    fn process_char(&mut self, ch: u8) -> VgaTextState {
        match &self.state {
            VgaTextState::PutChar => {
                if ch == 0x1b {
                    self.state = VgaTextState::SetColor(VgaTextSetColor::Start);
                }
            }
            VgaTextState::SetColor(state) => {
                match state {
                    VgaTextSetColor::Start => {
                        if ch == b'[' {
                            self.state = VgaTextState::SetColor(VgaTextSetColor::LeftBrackets);
                        } else {
                            // ignore invalid state and put it
                            self.state = VgaTextState::PutChar;
                        }
                    }
                    VgaTextSetColor::LeftBrackets => {
                        match ch {
                            b'm' => {
                                self.set_color(None);
                                self.state = VgaTextState::SetColor(VgaTextSetColor::End);
                            }
                            ch_val @ b'0'..=b'9' => {
                                self.state =
                                    VgaTextState::SetColor(VgaTextSetColor::Value(ch_val - b'0'));
                            }
                            _ => {
                                // ignore invalid state and put it
                                self.state = VgaTextState::PutChar;
                            }
                        }
                    }
                    VgaTextSetColor::Value(v) => {
                        match ch {
                            b'm' => {
                                let color = match (*v).try_into() {
                                    Ok(c) => Some(VgaTextColorCode::new(
                                        VgaTextColor::from_console_color(c),
                                        VgaTextColor::Black,
                                    )),
                                    Err(_) => None,
                                };
                                self.set_color(color);
                                self.state = VgaTextState::SetColor(VgaTextSetColor::End);
                            }
                            ch_val @ b'0'..=b'9' => {
                                self.state = VgaTextState::SetColor(VgaTextSetColor::Value(
                                    v * 10 + (ch_val - b'0'),
                                ));
                            }
                            _ => {
                                // ignore invalid state and put it
                                self.state = VgaTextState::PutChar;
                            }
                        }
                    }
                    VgaTextSetColor::End => {
                        if ch == 0x1b {
                            self.state = VgaTextState::SetColor(VgaTextSetColor::Start);
                        } else {
                            self.state = VgaTextState::PutChar;
                        }
                    }
                }
            }
        }

        self.state
    }

    fn set_color(&mut self, color: Option<VgaTextColorCode>) {
        self.current_color = color.unwrap_or(VgaTextColorCode::new(
            VgaTextColor::White,
            VgaTextColor::Black,
        ));
    }

    fn putchar(&mut self, ch: u8) {
        match ch {
            b'\r' => {
                self.current_x = 0;
            }
            b'\n' => {
                // treat it as \r\n
                self.current_x = 0;
                self.current_y += 1;
            }
            _ => {
                self.buffer.chars[self.current_y][self.current_x] =
                    VgaTextChar(ch, self.current_color);
                self.current_x += 1;
            }
        }

        if self.current_x >= VGA_BUFFER_WIDTH {
            self.current_x = 0;
            self.current_y += 1;
        }
        if self.current_y >= VGA_BUFFER_HEIGHT {
            self.scroll_up(self.current_y - VGA_BUFFER_HEIGHT + 1);
        }
    }
}



pub fn putchar(c: u8) {
    let mut vga = VGA.lock();

    if matches!(vga.process_char(c), VgaTextState::PutChar) {
        vga.putchar(c);
    }
}

pub fn getchar() -> Option<u8> {
    None
}

impl Write for VgaTextMode {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let bytes = s.as_bytes();
        for c in bytes {
            putchar(*c);
        }
        Ok(())
    }
}

pub(super) fn init_early() {
    let mut vga = VGA.lock();
    unsafe {
        vga.buffer
            .init_by(&mut *(VGA_BASE_ADDR.as_usize() as *mut VgaTextBuffer));
    }
    for y in 0..VGA_BUFFER_HEIGHT {
        for x in 0..VGA_BUFFER_WIDTH {
            vga.buffer.chars[y][x] = VgaTextChar(b' ', vga.current_color);
        }
    }
}

pub(super) fn init() {
    #[cfg(feature = "paging")]
    {
        use crate::mem::phys_to_virt;

        let mut vga = VGA.lock();
        vga.buffer = LazyInit::new();
        unsafe {
            vga.buffer
                .init_by(&mut *(phys_to_virt(VGA_BASE_ADDR).as_usize() as *mut VgaTextBuffer));
        }
    }
}

/// Set the maximum debug level.
///
/// `level` should be one of 0, 1, 2, 3.
pub fn set_max_level(level: u8) {
    unsafe {
        if level > 3 {
            panic!("LEVEL_DEBUG INPUT WRONG RANGE!");
        }
        LEVEL_DEBUG = level;
    }
}

pub fn print_debug(level: u8, args: fmt::Arguments) -> fmt::Result{
    unsafe {
        if level > LEVEL_DEBUG {
            return Err(Error);
        }
    }
    let mut vga = VGA.lock();
    match level {
        1 => {
            vga.set_color(Some(VgaTextColorCode::new(
                VgaTextColor::LightGreen,
                VgaTextColor::Black,
            )));
            vga.write_str("[INFO]  ");
        }
        2 => {
            vga.set_color(Some(VgaTextColorCode::new(
                VgaTextColor::LightBlue,
                VgaTextColor::Black,
            )));
            vga.write_str("[DEV]   ");
        }
        3 => {
            vga.set_color(Some(VgaTextColorCode::new(
                VgaTextColor::Yellow,
                VgaTextColor::Black,
            )));
            vga.write_str("[DEBUG] ");
        },
        _ => return Err(Error)
    }
    vga.set_color(Some(VgaTextColorCode::new(
        VgaTextColor::White,
        VgaTextColor::Black,
    )));
    vga.write_fmt(args)
}