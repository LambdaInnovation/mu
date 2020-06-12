use std::time::Instant;

pub struct Time {
    delta_time: f64, //Duration,
    now: Instant,
}

impl Default for Time {
    fn default() -> Time {
        let now = Instant::now();
        Time {
            delta_time: (now.elapsed().as_micros() as f64) / 1e6f64, // Duration::from_secs(0),
            now,
        }
    }
}

impl Time {
    pub fn update_delta_time(&mut self) {
        self.delta_time = (self.now.elapsed().as_micros() as f64) / 1e6f64;
        self.now = Instant::now();
    }

    pub fn get_delta_time(&self) -> f64 {
        self.delta_time
    }
}
