use drivers::time::Duration;

pub fn hours_mins_secs(d: Duration) -> (u32, u32, u32) {
    let seconds = d.as_secs();

    let sec_clock = seconds % 60;
    let minutes = seconds / 60;
    let min_clock = minutes % 60;
    let hours = minutes / 60;

    (hours as _, min_clock as _, sec_clock as _)
}

pub struct RingBuffer<const N: usize, T> {
    ring_buffer: [T; N],
    next: usize,
    num_total: usize,
}

impl<const N: usize, T: Default> Default for RingBuffer<N, T> {
    fn default() -> Self {
        Self {
            ring_buffer: core::array::from_fn(|_| Default::default()),
            next: 0,
            num_total: 0,
        }
    }
}
impl<const N: usize, T> RingBuffer<N, T> {
    pub fn add(&mut self, v: T) {
        self.num_total += 1;
        self.ring_buffer[self.next] = v;
        self.next = (self.next + 1) % N;
    }
    pub fn inner(&self) -> &[T; N] {
        &self.ring_buffer
    }
    pub fn num_valid(&self) -> usize {
        self.num_total.min(N)
    }
    //pub fn values(&self) -> &[T] {
    //    &self.ring_buffer[..self.num_total.min(N)]
    //}
}
