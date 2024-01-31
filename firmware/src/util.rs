use drivers::time::Duration;

pub fn hours_mins_secs(d: Duration) -> (u32, u32, u32) {
    let seconds = d.as_secs();

    let sec_clock = seconds % 60;
    let minutes = seconds / 60;
    let min_clock = minutes % 60;
    let hours = minutes / 60;

    (hours as _, min_clock as _, sec_clock as _)
}
