use specs::{Component, VecStorage};
use crate::consts::{HIT_RANGE, PERFECT_HIT_RANGE};
use std::time::{Instant};
use crate::osu_parser::OsuHitObjectHitSound;

#[derive(Debug)]
pub struct GamePos {
    pub x: f32,
    pub y: f32,
}

pub struct Circle;

#[derive(Debug, Copy, Clone)]
pub struct Lifetime {
    pub remaining: f64,
}

#[derive(Copy, Clone)]
pub enum CircleHitRating {
    MISS,
    GOOD,
    GREAT,
}

pub struct DespawnObject {
    pub reason: DespawnObjectReason,
    pub despawned_at: Instant,
}

pub struct HitSound {
    pub value: OsuHitObjectHitSound,
}

pub enum DespawnObjectReason {
    CircleHit(CircleHitRating),
    SliderEnd(SliderState),
}

pub struct Slider {
    pub curve_points: Vec<(f32, f32)>,
    pub skia_path: skia_safe::Path,
    pub duration_in_secs: f64,
    pub progress: f64,
    pub curve: SliderCurve,
    pub state: SliderState,
}

pub enum SliderStateChange {
    NoChange,
    Start,
    Stop,
}

impl Slider {
    pub fn go_to_state(
        &mut self,
        next_state: SliderState,
    ) -> SliderStateChange {
        let result;

        match self.state {
            SliderState::UNTOUCHED => {
                match next_state {
                    SliderState::UNTOUCHED => unreachable!(),
                    SliderState::DRAGGING(_) => {
                        result = SliderStateChange::Start;
                    }
                    SliderState::FINISHED(_, _) => {
                        result = SliderStateChange::NoChange;
                    }
                }
            }
            SliderState::DRAGGING(_) => {
                match next_state {
                    SliderState::UNTOUCHED => unreachable!(),
                    SliderState::DRAGGING(_) => {
                        result = SliderStateChange::NoChange;
                    }
                    SliderState::FINISHED(_, _) => {
                        result = SliderStateChange::Stop;
                    }
                };
            }
            SliderState::FINISHED(_, _) => {
                match next_state {
                    SliderState::UNTOUCHED => unreachable!(),
                    SliderState::DRAGGING(_) => {
                        result = SliderStateChange::Start;
                    }
                    SliderState::FINISHED(_, _) => {
                        result = SliderStateChange::NoChange;
                    }
                };
            }
        }

        self.state = next_state;

        return result;
    }
}

#[derive(Copy, Clone, Debug)]
pub enum SliderState {
    UNTOUCHED,
    // secs_complete ( time the user spend successfully dragging this slider)
    DRAGGING(f64),
    // percent_complete (0.0-1.0), finished_at
    FINISHED(f64, Instant),
}

pub enum SliderCurve {
    Linear(SliderCurveLinear),
    QuadBezier(SliderCurveQuadBezier),
    CubicBezier(SliderCurveCubicBezier),
}

pub struct SliderCurveLinear {
    pub start: (f32, f32),
    pub end: (f32, f32),
}

pub struct SliderCurveQuadBezier {
    pub start: (f32, f32),
    pub control_point: (f32, f32),
    pub end: (f32, f32),
}

pub struct SliderCurveCubicBezier {
    pub start: (f32, f32),
    pub control_point: (f32, f32),
    pub control_point_2: (f32, f32),
    pub end: (f32, f32),
}

impl Lifetime {
    pub fn zero() -> &'static Lifetime {
        const INSTANCE: Lifetime = Lifetime { remaining: 0.0 };
        return &INSTANCE;
    }
    pub fn is_dead(&self) -> bool { self.remaining <= 0.0 }
    pub fn is_alive(&self) -> bool {
        self.remaining > 0.0
    }
    pub fn is_in_hit_zone(&self) -> bool {
        HIT_RANGE.contains(&self.remaining)
    }
    pub fn is_in_perfect_hit_zone(&self) -> bool {
        PERFECT_HIT_RANGE.contains(&self.remaining)
    }
}

impl Component for GamePos {
    type Storage = VecStorage<GamePos>;
}

impl Component for Circle {
    type Storage = VecStorage<Circle>;
}

impl Component for Lifetime {
    type Storage = VecStorage<Lifetime>;
}

impl Component for CircleHitRating {
    type Storage = VecStorage<CircleHitRating>;
}

impl Component for DespawnObject {
    type Storage = VecStorage<DespawnObject>;
}

impl Component for HitSound {
    type Storage = VecStorage<HitSound>;
}

impl Component for Slider {
    type Storage = VecStorage<Slider>;
}

impl Default for CircleHitRating {
    fn default() -> Self {
        CircleHitRating::MISS
    }
}

