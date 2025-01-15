#![allow(unused)]

mod date_time;
mod date_time_error;

use bytes::BytesMut;
use date_time::DateTime;
use std::{
    cell::UnsafeCell,
    fmt::{self, Write},
    sync::{Arc, LazyLock},
    time::SystemTime,
};

/// Date length: "Wed, 01 Jan 2025 00:00:00 GMT".len() = 29
const DATE_VALUE_LENGTH: usize = 29;

static CURRENT_DATE: LazyLock<Arc<DataWrap>> = LazyLock::new(|| {
    let date = Arc::new(DataWrap(UnsafeCell::new(Date::now())));

    date
});

struct DataWrap(UnsafeCell<Date>);

unsafe impl Sync for DataWrap {}
// unsafe impl Sync for LazyCell<Arc<DataWrap>> {}

#[inline]
pub fn append_date(dst: &mut BytesMut) {
    let date = unsafe { &*CURRENT_DATE.0.get() };

    dst.extend_from_slice(date.as_bytes())
}

struct Date {
    bytes: [u8; DATE_VALUE_LENGTH],
}

impl Date {
    fn now() -> Date {
        let mut date = Date {
            bytes: [0; DATE_VALUE_LENGTH],
        };
        let t = SystemTime::now();
        let date_time = DateTime::from(t);

        write!(date, "{}", date_time).unwrap();

        date
    }

    #[inline]
    fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }
}

impl fmt::Write for Date {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.bytes.copy_from_slice(s.as_bytes());

        Ok(())
    }
}
