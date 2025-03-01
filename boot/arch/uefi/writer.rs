use crate::arch::uefi::spec::*;
use core::fmt;

impl fmt::Write for Output {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let mut chars = s.chars();
        let mut bufc = [0; 2];
        const BUF_SIZE: usize = 128;
        let mut buf = [0u16; BUF_SIZE];

        let mut i = 0;
        while let Some(c) = chars.next() {
            let encoded = c.encode_utf16(&mut bufc);
            if encoded.len() == 2 {
                // UEFI only supports UCS-2, not UTF-16.
                // Replace with unicode 'replacement character'
                buf[i] = 0xFFFD;
            } else {
                buf[i] = bufc[0];
            }
            i += 1;
            if i == BUF_SIZE - 1 {
                buf[i] = 0;
                unsafe {
                    if (self.output_string)(self, buf.as_ptr()) != 0 {
                        return Err(fmt::Error {});
                    }
                }
                i = 0;
            }
            if c == '\n' {
                // rust use '\n' for new line. uefi needs '\n\r',
                // Another workaround is replace '\n' with unicode `LS(0x2028)`,
                // but ovmf can't display it.
                buf[i] = 0x000d; // unicode '\r'
                i += 1;
                if i == BUF_SIZE - 1 {
                    buf[i] = 0;
                    unsafe {
                        if (self.output_string)(self, buf.as_ptr()) != 0 {
                            return Err(fmt::Error {});
                        }
                    }
                    i = 0;
                }
            }
        }
        buf[i] = 0;
        unsafe {
            (self.output_string)(self, buf.as_ptr());
        }
        Ok(())
    }
}
