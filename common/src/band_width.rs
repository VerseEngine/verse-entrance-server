const DEFAULT_MIN_SPAN_MS: u64 = 1000;

#[derive(Clone, Default)]
pub struct BandWidth {
    created: u64,
    begin: u64,
    bytes_total: usize,
    bytes_per_second: u64,
    min_span_ms: u64,
}

impl BandWidth {
    pub fn new() -> Self {
        BandWidth {
            min_span_ms: DEFAULT_MIN_SPAN_MS,
            ..Default::default()
        }
    }
    pub fn new_with_min_span_ms(min_span_ms: u64) -> Self {
        BandWidth {
            min_span_ms,
            ..Default::default()
        }
    }
    pub fn get_current_bits_per_seconds(&self) -> u64 {
        self.bytes_per_second * 8
    }
    pub fn add_bytes(&mut self, byte_len: usize, now: u64) {
        if self.begin == 0 {
            self.begin = now
        }
        self.bytes_total += byte_len;
        let ms = now - self.begin;
        if ms < self.min_span_ms {
            return;
        }
        self.bytes_per_second = (self.bytes_total as u64) / (ms / 1000);
        self.created = now;
        self.begin = 0;
        self.bytes_total = 0;
    }
}
