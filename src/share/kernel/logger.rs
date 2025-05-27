use core::fmt::Write;
use log;

pub struct Logger {
    pub writer: Option<*mut dyn Write>,
}

//TODO(fangzhen) concurrency
unsafe impl Sync for Logger {}
unsafe impl Send for Logger {}

impl log::Log for Logger {
    fn enabled(&self, _metadata: &log::Metadata) -> bool {
        self.writer.is_some()
    }

    fn log(&self, record: &log::Record) {
        if self.enabled(record.metadata()) {
            let o = unsafe { self.writer.unwrap().as_mut().unwrap_unchecked() };
            writeln!(
                o,
                "[{}] {}:{} {}",
                record.level(),
                record.file().unwrap_or("<unknown file>"),
                record.line().unwrap_or(0),
                *(record.args())
            )
            .unwrap_or(());
        }
    }

    fn flush(&self) {}
}
