use std::time::Duration;

use egui::Color32;

use crate::project::MID_A_FREQUENCY;

pub fn linear_interpolate<T>(arr: &[T], index: f32) -> T
where
    T: Into<f32> + From<f32> + Clone,
{
    if arr.is_empty() {
        return 0.0.into();
    }

    let prev = index as usize % arr.len();
    let next = (prev + 1) % arr.len();
    // todo: make this look cleaner
    ((arr[next].clone().into() - arr[prev].clone().into()) * (index - prev as f32)
        + arr[prev].clone().into())
    .into()
}

pub fn ticks_to_duration(ticks: f32, tempo: f32) -> Duration {
    let ms_per_tick = ((1.0 / { tempo }) / 4.0) * 60000.0;
    Duration::from_millis((ticks * ms_per_tick) as u64)
}

pub fn semitone_from_frequency(frequency: f32) -> u8 {
    (12.0 * f32::log2(frequency / MID_A_FREQUENCY) + 57.0) as u8
}

pub fn frequency_from_semitone(semitone: f32) -> f32 {
    let diff = semitone - 57.0;
    let ratio = 2.0_f32.powf(1.0 / 12.0);
    MID_A_FREQUENCY * ratio.powf(diff)
}

pub fn to_colour32(arr: [u8; 3]) -> Color32 {
    Color32::from_rgb(arr[0], arr[1], arr[2])
}
