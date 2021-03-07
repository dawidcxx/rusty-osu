use std::time::{Duration, Instant};

use glutin::{ContextWrapper, PossiblyCurrent};
use glutin::window::Window;
use skia_safe::{ColorType, Surface};
use skia_safe::gpu::{BackendRenderTarget, SurfaceOrigin};
use skia_safe::gpu::Context as GpuContext;
use skia_safe::gpu::gl::FramebufferInfo;
use std::collections::{VecDeque, HashMap, HashSet};
use std::collections::vec_deque::Iter;
use crate::{utils::{Timer, max_f64, min_f64}};
use crate::consts::{TRIAL_POINTS, TRAIL_SAMPLE_EACH};
use glutin::event::VirtualKeyCode;
use crate::components::{SliderStateChange};

#[derive(Debug, Default)]
pub struct GameCursor {
    pub window_x: f32,
    pub window_y: f32,
}

#[derive(Debug)]
pub struct Hp {
    pub value: f64,
    pub drain: f64,
}

impl Hp {
    pub fn drain(&mut self) {
        self.value -= self.drain;
        self.value = max_f64(self.value, 0.0);
    }
    pub fn fill(&mut self) {
        self.value += self.drain;
        self.value = min_f64(self.value, 1.0);
    }
}

impl Default for Hp {
    fn default() -> Self {
        return Self {
            value: 1.0,
            drain: 0.1,
        };
    }
}


#[derive(Debug)]
pub struct Trail {
    storage: VecDeque<(f32, f32)>,
    index: usize,
}

impl Default for Trail {
    fn default() -> Self {
        Self {
            storage: (0..TRIAL_POINTS).map(|_| (-1.0, -1.0)).collect(),
            index: 0,
        }
    }
}

impl Trail {
    pub fn add(&mut self, cords: (f32, f32)) {
        self.storage.push_front(cords);
        self.storage.pop_back();
    }

    pub fn iter(&self) -> Iter<'_, (f32, f32)> {
        self.storage.iter()
    }

    pub fn get(&self, index: usize) -> Option<(f32, f32)> {
        self.storage.get(index).cloned()
    }
}

pub struct TrailTimer(pub Timer);

impl Default for TrailTimer {
    fn default() -> Self {
        TrailTimer(Timer::new(TRAIL_SAMPLE_EACH))
    }
}


#[derive(Debug)]
pub struct CircleSpawnerData {}

#[derive(Debug)]
pub struct Time {
    pub delta: Duration,
    pub delta_seconds: f64,
    pub secs_since_start: f64,
    pub now: Instant,
}

impl Default for Time {
    fn default() -> Self {
        Time {
            delta: Duration::from_millis(0),
            delta_seconds: 0.0,
            secs_since_start: 0.0,
            now: Instant::now(),
        }
    }
}

#[derive(Debug, Default)]
pub struct GameArea {
    matrix: Option<skia_safe::matrix::Matrix>,
    scale: f32,
}

impl GameArea {
    pub fn game_cords_to_screen(&self, xy: (f32, f32)) -> (f32, f32) {
        let p = self.matrix.unwrap().map_point(xy);
        (p.x, p.y)
    }

    pub fn set_game_area_matrix(&mut self, new_matrix: skia_safe::matrix::Matrix) {
        assert_eq!(new_matrix.scale_x(), new_matrix.scale_y(), "Canvas x/y scale must be the same");
        self.scale = new_matrix.scale_x();
        self.matrix = Some(new_matrix);
    }

    pub fn scale(&self) -> f32 {
        self.scale
    }
}

#[derive(Debug, Default)]
pub struct GameInputState {
    pub quitting: bool,

    // keys that are pressed for this frame
    pub active_set: HashSet<VirtualKeyCode>,

    // keeps track of the last time we pressed a key
    // useful for animation
    pub last_active_keys_map: HashMap<VirtualKeyCode, Instant>,

    pub hold_set: HashSet<VirtualKeyCode>,

}

pub enum GameInputKeyBinding {
    Hit1,
    Hit2,
}

const fn key_vk_for_key_binding(kb: &'static GameInputKeyBinding) -> &'static VirtualKeyCode {
    match kb {
        GameInputKeyBinding::Hit1 => &VirtualKeyCode::G,
        GameInputKeyBinding::Hit2 => &VirtualKeyCode::H,
    }
}

impl GameInputState {
    pub fn clear_frame(&mut self) {
        self.active_set.clear();
    }

    pub fn is_key_active(&self, bind: &'static GameInputKeyBinding) -> bool {
        self.active_set.contains(key_vk_for_key_binding(bind))
    }

    pub fn is_key_hold(&self, bind: &'static GameInputKeyBinding) -> bool {
        self.hold_set.contains(key_vk_for_key_binding(bind))
    }

    pub fn last_pressed_at(&self, bind: &'static GameInputKeyBinding) -> Option<Instant> {
        let key = key_vk_for_key_binding(bind);
        self.last_active_keys_map.get(key).cloned()
    }
}

#[derive(Default)]
pub struct Combo {
    pub value: u64,
}

impl Combo {
    pub fn maintain(&mut self) {
        self.value += 1;
    }
    pub fn reset(&mut self) {
        self.value = 0;
    }
}

#[derive(Default)]
pub struct Score {
    pub value: u64,
}

impl Score {
    pub fn on_good(&mut self, c: &Combo) {
        self.value += c.value * 100;
    }
    pub fn on_great(&mut self, c: &Combo) {
        self.value += c.value * 300;
    }
}

#[derive(PartialOrd, PartialEq)]
pub enum GameEvent {
    SongLoad(String),
    WindowResized((u32, u32)),
    SliderStart,
    SliderStop,
}

#[derive(Default)]
pub struct GameEvents {
    storage: Vec<GameEvent>,
    has_events: bool,
}

impl GameEvents {
    pub fn clear(&mut self) {
        self.storage.clear();
        self.has_events = false;
    }

    pub fn emit(&mut self, ev: GameEvent) {
        self.storage.push(ev);
        self.has_events = true;
    }

    pub fn emit_on_slider_change(&mut self, slider_change: SliderStateChange) {
        match slider_change {
            SliderStateChange::NoChange => {
                // nothing to do
            }
            SliderStateChange::Start => {
                self.emit(GameEvent::SliderStart);
            }
            SliderStateChange::Stop => {
                self.emit(GameEvent::SliderStop);
            }
        }
    }

    // todo: ungopher this pattern
    pub fn on_song_load<CB>(&self, cb: CB) where CB: FnOnce(&String) {
        if !self.has_events {
            return;
        }
        for event in self.storage.iter() {
            if let GameEvent::SongLoad(song) = event {
                cb(song);
                break;
            }
        }
    }
    pub fn on_resized<CB>(&self, cb: CB) where CB: FnOnce(&(u32, u32)) {
        if !self.has_events {
            return;
        }
        for event in self.storage.iter() {
            if let GameEvent::WindowResized(cords) = event {
                cb(cords);
                break;
            }
        }
    }

    pub fn on_slider_start<CB>(&self, cb: CB) where CB: FnOnce() {
        if !self.has_events {
            return;
        }
        for event in self.storage.iter() {
            if event == &GameEvent::SliderStart {
                cb();
                break;
            }
        }
    }

    pub fn on_slider_end<CB>(&self, cb: CB) where CB: FnOnce() {
        if !self.has_events {
            return;
        }
        for event in self.storage.iter() {
            if event == &GameEvent::SliderStop {
                cb();
                break;
            }
        }
    }
}

pub struct Graphics {
    pub surface: Surface,
    pub width: u32,
    pub width_f32: f32,
    pub width_f64: f64,
    pub height: u32,
    pub height_f32: f32,
    pub height_f64: f64,
}


impl Graphics {
    pub fn new(window_ctx: &ContextWrapper<PossiblyCurrent, Window>, mut gpu_context: &mut GpuContext) -> Graphics {
        let w = window_ctx.window().inner_size().width;
        let h = window_ctx.window().inner_size().height;
        Graphics {
            surface: make_surface(&window_ctx, &mut gpu_context),
            width: w,
            height: h,
            width_f64: w as f64,
            height_f64: h as f64,
            width_f32: w as f32,
            height_f32: h as f32,
        }
    }
}


fn make_surface(
    window_ctx: &ContextWrapper<PossiblyCurrent, Window>,
    mut gpu_context: &mut GpuContext,
) -> Surface {
    use std::convert::TryInto;
    use gl::types::*;

    let frame_buffer_info = {
        let mut id: GLint = 0;
        unsafe { gl::GetIntegerv(gl::FRAMEBUFFER_BINDING, &mut id) };
        FramebufferInfo {
            fboid: id.try_into().unwrap(),
            format: skia_safe::gpu::gl::Format::RGBA8.into(),
        }
    };

    let pixel_format = window_ctx.get_pixel_format();
    let size = window_ctx.window().inner_size();
    let backend_render_target = BackendRenderTarget::new_gl(
        (
            size.width.try_into().unwrap(),
            size.height.try_into().unwrap(),
        ),
        pixel_format.multisampling.map(|s| s.try_into().unwrap()),
        pixel_format.stencil_bits.try_into().unwrap(),
        frame_buffer_info,
    );

    Surface::from_backend_render_target(
        &mut gpu_context,
        &backend_render_target,
        SurfaceOrigin::BottomLeft,
        ColorType::RGBA8888,
        None,
        None,
    ).unwrap()
}