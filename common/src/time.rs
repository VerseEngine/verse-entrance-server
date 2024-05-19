pub fn is_expired(now_msec: u64, last_modified: u64, expire_msec: u64) -> bool {
    now_msec >= last_modified + expire_msec
}

cfg_if::cfg_if! {
    if #[cfg(target_family = "wasm")] {
        pub fn get_now_msec() -> u64 {
            js_sys::Date::now() as u64
        }
        pub fn get_now_sec() -> u64 {
            (js_sys::Date::now() / 1000.0) as u64
        }
    } else {
        pub fn get_now_msec() -> u64 {
            let v = chrono::Local::now();
            (v.timestamp() * 1000 + v.timestamp_subsec_millis() as i64) as u64
        }
        pub fn get_now_sec() -> u64 {
            let v = chrono::Local::now();
            v.timestamp() as u64
        }
    }
}

pub mod mock {
    use std::cell::Cell;

    thread_local! {
        static MOCK_TIME: Cell<u64> = Cell::new(0);
    }
    pub fn set_mock_now(msec: u64) {
        MOCK_TIME.with(|cell| cell.set(msec));
    }
    pub fn clear_mock_now() {
        MOCK_TIME.with(|cell| cell.set(0));
    }
    pub fn get_now_msec() -> u64 {
        let v = MOCK_TIME.with(|cell| cell.get());
        if v == 0 {
            super::get_now_msec()
        } else {
            v
        }
    }
    pub fn get_now_sec() -> u64 {
        get_now_msec() / 1000
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_is_expired() {
        assert!(is_expired(100, 90, 10));
        assert!(!is_expired(99, 90, 10));
        assert!(is_expired(101, 90, 10));
    }
    #[test]
    fn test_mock() {
        assert_ne!(mock::get_now_msec(), 0);
        assert_ne!(mock::get_now_sec(), 0);

        mock::set_mock_now(10000);
        assert_eq!(mock::get_now_msec(), 10000);
        assert_eq!(mock::get_now_sec(), 10);

        mock::clear_mock_now();
        assert_ne!(mock::get_now_msec(), 10000);
        assert_ne!(mock::get_now_sec(), 10);
    }
}
