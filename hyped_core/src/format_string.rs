use core::cmp::min;
use core::fmt;
use defmt::error;

pub struct FormatString<'a> {
    buffer: &'a mut [u8],
    // on write error (i.e. not enough space in buffer) this grows beyond
    // `buffer.len()`.
    used: usize,
}

impl<'a> FormatString<'a> {
    pub fn new(buffer: &'a mut [u8]) -> Self {
        FormatString { buffer, used: 0 }
    }

    pub fn as_str(self) -> Option<&'a str> {
        if self.used <= self.buffer.len() {
            // only successful concats of str - must be a valid str.
            use core::str::from_utf8;
            Some(from_utf8(&self.buffer[..self.used]).unwrap())
        } else {
            error!("FormatString buffer overflow");
            None
        }
    }
}

impl<'a> fmt::Write for FormatString<'a> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        if self.used > self.buffer.len() {
            return Err(fmt::Error);
        }
        let remaining_buf = &mut self.buffer[self.used..];
        let raw_s = s.as_bytes();
        let write_num = min(raw_s.len(), remaining_buf.len());
        remaining_buf[..write_num].copy_from_slice(&raw_s[..write_num]);
        self.used += raw_s.len();
        if write_num < raw_s.len() {
            Err(fmt::Error)
        } else {
            Ok(())
        }
    }
}

pub fn show<'a>(buffer: &'a mut [u8], args: fmt::Arguments) -> Result<&'a str, fmt::Error> {
    let mut w = FormatString::new(buffer);
    fmt::write(&mut w, args)?;
    w.as_str().ok_or(fmt::Error)
}
