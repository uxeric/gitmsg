//! One-line status while the commit title is written.
//!
//! The glyph is the braille "dots" cycle: ten frames, 80ms apart, one full
//! turn every 0.8s. Each glyph is a single column, so "Generating" stays still.

use std::io::{self, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

const FRAMES: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
const INTERVAL: Duration = Duration::from_millis(80);

pub struct Spinner {
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl Spinner {
    /// Show `message` beside the spinner when stderr is a terminal.
    pub fn start(message: &str) -> Self {
        if !io::IsTerminal::is_terminal(&io::stderr()) || is_dumb_term() {
            return Self::idle();
        }
        install_interrupt_restore();
        let stop = Arc::new(AtomicBool::new(false));
        let stop_for_thread = Arc::clone(&stop);
        let message = message.to_string();
        let color = color_enabled();
        let handle = thread::spawn(move || {
            let mut err = io::stderr();
            let _ = write!(err, "\x1b[?25l");
            let _ = err.flush();
            let mut index = 0;
            while !stop_for_thread.load(Ordering::SeqCst) {
                let _ = paint(&mut err, FRAMES[index % FRAMES.len()], &message, color);
                index = index.wrapping_add(1);
                thread::park_timeout(INTERVAL);
            }
            let _ = clear(&mut err);
        });
        Self {
            stop,
            handle: Some(handle),
        }
    }

    fn idle() -> Self {
        Self {
            stop: Arc::new(AtomicBool::new(true)),
            handle: None,
        }
    }
}

impl Drop for Spinner {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(handle) = self.handle.take() {
            handle.thread().unpark();
            let _ = handle.join();
        }
    }
}

fn paint(out: &mut impl Write, frame: &str, message: &str, color: bool) -> io::Result<()> {
    if color {
        write!(out, "\r\x1b[2K{frame} \x1b[2m{message}\x1b[0m")?;
    } else {
        write!(out, "\r\x1b[2K{frame} {message}")?;
    }
    out.flush()
}

fn clear(out: &mut impl Write) -> io::Result<()> {
    write!(out, "\r\x1b[2K\x1b[0m\x1b[?25h")?;
    out.flush()
}

fn color_enabled() -> bool {
    std::env::var_os("NO_COLOR").is_none()
}

fn is_dumb_term() -> bool {
    matches!(std::env::var("TERM").as_deref(), Ok("dumb"))
}

fn install_interrupt_restore() {
    unsafe {
        libc::signal(
            libc::SIGINT,
            on_interrupt as *const () as libc::sighandler_t,
        );
    }
}

extern "C" fn on_interrupt(_signal: libc::c_int) {
    let restore = b"\r\x1b[2K\x1b[0m\x1b[?25h";
    unsafe {
        libc::write(
            libc::STDERR_FILENO,
            restore.as_ptr().cast::<libc::c_void>(),
            restore.len(),
        );
        libc::signal(libc::SIGINT, libc::SIG_DFL);
        libc::raise(libc::SIGINT);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::os::fd::{AsRawFd, FromRawFd};
    use std::time::Duration;

    #[test]
    fn frames_are_one_column_and_the_turn_takes_under_a_second() {
        for frame in FRAMES {
            assert_eq!(frame.chars().count(), 1, "{frame}");
            let code = frame.chars().next().unwrap() as u32;
            assert!(
                (0x2800..=0x28FF).contains(&code),
                "{frame} is not a braille glyph"
            );
        }
        assert_eq!(FRAMES.len(), 10);
        assert_eq!(INTERVAL.as_millis(), 80);
        let mut seen = FRAMES.to_vec();
        seen.sort_unstable();
        seen.dedup();
        assert_eq!(seen.len(), FRAMES.len());
    }

    #[test]
    fn paint_rewrites_one_line_and_clear_puts_the_cursor_back() {
        let mut painted = Vec::new();
        paint(&mut painted, FRAMES[0], "Generating", true).unwrap();
        let text = String::from_utf8(painted).unwrap();
        assert!(!text.contains('\n'), "{text:?}");
        assert!(text.starts_with("\r\x1b[2K"));
        assert!(text.contains('⠋'));
        assert!(text.contains("\x1b[2mGenerating\x1b[0m"));

        let mut cleared = Vec::new();
        clear(&mut cleared).unwrap();
        let text = String::from_utf8(cleared).unwrap();
        assert_eq!(text, "\r\x1b[2K\x1b[0m\x1b[?25h");
    }

    #[test]
    fn spinner_cycles_on_one_line_then_erases_itself() {
        let (master, slave) = match open_pty() {
            Ok(pair) => pair,
            Err(_) => return,
        };
        let stop = Arc::new(AtomicBool::new(false));
        let stop_for_thread = Arc::clone(&stop);
        let handle = thread::spawn(move || {
            let mut out = slave;
            let _ = write!(out, "\x1b[?25l");
            let mut index = 0;
            while !stop_for_thread.load(Ordering::SeqCst) {
                let _ = paint(&mut out, FRAMES[index % FRAMES.len()], "Generating", true);
                index = index.wrapping_add(1);
                thread::park_timeout(INTERVAL);
            }
            let _ = clear(&mut out);
        });
        thread::sleep(Duration::from_millis(250));
        stop.store(true, Ordering::SeqCst);
        handle.thread().unpark();
        handle.join().unwrap();

        let mut captured = String::new();
        let _ = set_nonblocking(master.as_raw_fd());
        let mut buf = [0_u8; 256];
        loop {
            match std::io::Read::read(&mut &master, &mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => captured.push_str(&String::from_utf8_lossy(&buf[..n])),
            }
        }
        assert!(!captured.contains('\n'), "{captured:?}");
        assert!(captured.contains("Generating"), "{captured:?}");
        let seen = FRAMES
            .iter()
            .filter(|frame| captured.contains(**frame))
            .count();
        assert!(seen >= 2, "only {seen} frames in {captured:?}");
        assert!(captured.contains("\x1b[?25h"), "{captured:?}");
    }

    fn open_pty() -> io::Result<(File, File)> {
        let mut master: libc::c_int = -1;
        let mut slave: libc::c_int = -1;
        let rc = unsafe {
            libc::openpty(
                &mut master,
                &mut slave,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        if rc != 0 {
            return Err(io::Error::last_os_error());
        }
        unsafe { Ok((File::from_raw_fd(master), File::from_raw_fd(slave))) }
    }

    fn set_nonblocking(fd: libc::c_int) -> io::Result<()> {
        let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
        if flags < 0 {
            return Err(io::Error::last_os_error());
        }
        let rc = unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) };
        if rc < 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }
}
