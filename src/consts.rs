use std::ops::Range;
use std::time::Duration;

pub const DURATION_ZERO: Duration = Duration::from_nanos(0);
pub const BASE_CIRCLE_RADIUS: f32 = 35.0;
pub const BASE_CIRCLE_DIAMETER: f32 = BASE_CIRCLE_RADIUS * 2.0;
pub const BASE_SLIDER_CIRCLE_RADIUS: f32 = 60.0;

pub const LIFETIME: f64 = 0.700;
pub const HIT_WINDOW: f64 = 0.200;
pub const HIT_RANGE: Range<f64> = -HIT_WINDOW..HIT_WINDOW;
pub const PERFECT_HIT_RANGE: Range<f64> = -(HIT_WINDOW / 3.0)..(HIT_WINDOW / 3.0);
pub const TRIAL_POINTS: usize = 32;
pub const TRAIL_SAMPLE_EACH: Duration = Duration::from_millis(10);

