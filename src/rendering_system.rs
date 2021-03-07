use std::{cell::RefCell, rc::Rc};
use glutin::window::Window;
use skia_safe::*;
use specs::{Join, Read, ReadStorage, System, WriteExpect};
use crate::{consts::*, resources::GameEvents};
use crate::components::{Circle, GamePos, Lifetime, Slider, SliderState};
use crate::resources::{Graphics, Time, GameCursor, Trail, Hp, GameArea, GameInputState, Combo, Score, GameInputKeyBinding};
use splines::{Spline, Interpolation, Key};
use crate::utils::{min_f32};
use skia_safe::font_style::{Weight, Width, Slant};
use skia_safe::gpu::Context as GpuContext;

pub struct RenderingSystem {
    graphics: Graphics,
    gpu_context: Rc<RefCell<GpuContext>>,
    window_ctx: Rc<glutin::ContextWrapper<glutin::PossiblyCurrent, Window>>,
    shapes: Shapes,
    paints: Paints,
    splines: Splines,
    fonts: Fonts,
}

struct Fonts {
    default: Font,
}

struct Shapes {
    circle: Picture,
}

struct Paints {
    circle_base_paint: Paint,
    approach_circle: Paint,
    follow_circle: Paint,
    cursor: Paint,
    key_cap_on: Paint,
    key_cap_off: Paint,
    font_default: Paint,
    slider: Paint,
    slider_drag: Paint,
}

struct Splines {
    circle_fade_away_spline: Spline<f64, f32>,
    circle_life_spline: Spline<f64, f32>,
    key_cap_light_on_spline: Spline<f32, f32>,
    slider_hold_circle: Spline<f64, f32>,
}

impl<'a> System<'a> for RenderingSystem {
    type SystemData = (
        Read<'a, Time>,
        Read<'a, GameCursor>,
        Read<'a, Trail>,
        Read<'a, Hp>,
        Read<'a, Score>,
        Read<'a, Combo>,
        Read<'a, GameInputState>,
        Read<'a, GameEvents>,
        WriteExpect<'a, GameArea>,
        ReadStorage<'a, GamePos>,
        ReadStorage<'a, Circle>,
        ReadStorage<'a, Slider>,
        ReadStorage<'a, Lifetime>,
    );

    fn run(&mut self, (
        time,
        cursor,
        trail,
        hp,
        score,
        combo,
        input_state,
        events,
        mut game_area,
        positions,
        circles,
        sliders,
        lifetimes,
    ): Self::SystemData) {
        events.on_resized(|_| {
            self.on_resize();
        });

        let mut surface = self.graphics.surface.clone();
        surface.canvas().clear(Color::from_rgb(24, 24, 24));

        { // do all the game area drawing here
            let mut ctx = RenderingCtx {
                canvas: surface.canvas(),
                splines: &self.splines,
                paints: &self.paints,
                fonts: &self.fonts,
                shapes: &self.shapes,
            };

            const PADDING: f32 = 100.0;

            let width_scale = (self.graphics.width_f32 - PADDING) / 640.0;
            let height_scale = (self.graphics.height_f32 - PADDING) / 480.0;
            let scale = {
                let smaller_scale = min_f32(width_scale, height_scale);
                (smaller_scale, smaller_scale)
            };
            let translation = {
                let x_trx = (self.graphics.width_f32 - 640.0 * scale.0) / 2.0;
                let y_trx = (self.graphics.height_f32 - 480.0 * scale.1) / 2.0;
                (x_trx, y_trx)
            };

            ctx.canvas.save();

            ctx.canvas.translate(translation);
            ctx.canvas.scale(scale);
            game_area.set_game_area_matrix(ctx.canvas.total_matrix());

            ctx.canvas.draw_rect(
                Rect::new(0.0, 0.0, 640.0, 480.0),
                &ctx.paints.approach_circle,
            );


            for (_, pos, lifetime) in (&circles, &positions, &lifetimes).join() {
                ctx.draw_circle(
                    pos, lifetime,
                );
            }

            for (slider, pos, lifetime) in (&sliders, &positions, &lifetimes).join() {
                ctx.draw_slider(
                    slider,
                    pos,
                    lifetime,
                    &time,
                );
            }

            ctx.draw_user_hit(&input_state, &time);

            ctx.canvas.restore();

            ctx.draw_text(format!("Combo: {}", combo.value), Point::new(12.0, self.graphics.height_f32 - 50.0));
            ctx.draw_text(format!("Score: {}", score.value), Point::new(12.0, self.graphics.height_f32 - 25.0));
        }

        for (i, cords) in trail.iter().skip(1).enumerate() {
            let prev = trail.get(i).unwrap();
            surface.canvas().draw_line(
                (cords.0, cords.1),
                prev,
                &self.paints.follow_circle,
            );
        }

        surface.canvas().draw_circle(
            (cursor.window_x, cursor.window_y),
            12.0,
            &self.paints.cursor,
        );

        surface.canvas().draw_rect(
            Rect::new(0.0, 0.0, self.graphics.width_f32, 10.0),
            &self.paints.follow_circle,
        );
        surface.canvas().draw_rect(
            Rect::new(0.0, 0.0, self.graphics.width_f32 * (hp.value as f32), 10.0),
            &self.paints.cursor,
        );

        surface.canvas().flush();
    }
}


// shared data that all
// drawing functions might be interested in
struct RenderingCtx<'a> {
    canvas: &'a mut Canvas,
    splines: &'a Splines,
    paints: &'a Paints,
    fonts: &'a Fonts,
    shapes: &'a Shapes,
}

impl<'a> RenderingCtx<'a> {
    fn draw_user_hit(
        &mut self,
        input_state: &GameInputState,
        time: &Time,
    ) {
        self.draw_user_hit_indicator(0.0, &input_state, &GameInputKeyBinding::Hit1, time);
        self.draw_user_hit_indicator(1.0, &input_state, &GameInputKeyBinding::Hit2, time);
    }

    fn draw_user_hit_indicator(
        &mut self,
        index: f32,
        state: &GameInputState,
        binding: &'static GameInputKeyBinding,
        time: &Time,
    ) {
        let margin = 3.0;
        let width = 30.0;
        let height = 30.0;

        let pos_x = 640.0 + margin;
        let pos_y = ((480.0 / 2.0) + height / 2.0) + (index * height) + (index * margin);

        let alpha = if state.is_key_hold(binding) {
            1.0
        } else if let Some(last_pressed_at) = state.last_pressed_at(binding) {
            let elapsed = time.now.duration_since(last_pressed_at)
                .as_secs_f32();
            self.splines.key_cap_light_on_spline.clamped_sample(elapsed).unwrap()
        } else {
            0.0
        };

        let fill_paint = {
            let mut paint = self.paints.key_cap_on.clone();
            paint.set_alpha_f(alpha);
            paint
        };

        let pos = Rect::new(
            pos_x,
            pos_y,
            pos_x + width,
            pos_y + width,
        );

        self.canvas.draw_rect(pos, &fill_paint);
        self.canvas.draw_rect(pos, &self.paints.key_cap_off);
    }

    fn draw_text(
        &mut self,
        text: String,
        pos: Point,
    ) {
        self.canvas.draw_text_blob(
            TextBlob::from_str(text.as_str(), &self.fonts.default)
                .expect("Couldn't draw text"),
            pos,
            &self.paints.font_default,
        );
    }

    fn draw_slider(
        &mut self,
        slider: &Slider,
        pos: &GamePos,
        lifetime: &Lifetime,
        time: &Time,
    ) {
        self.canvas.draw_path(&slider.skia_path, &self.paints.slider);
        let lifetime = if lifetime.is_dead() { Lifetime::zero() } else { lifetime };
        self.draw_circle(pos, lifetime);

        if let SliderState::DRAGGING(_) = slider.state {
            self.canvas.draw_circle(
                (pos.x, pos.y),
                BASE_SLIDER_CIRCLE_RADIUS,
                &self.paints.slider_drag,
            );
        } else if let SliderState::FINISHED(_, finished_at) = slider.state {
            let radius = self.splines.slider_hold_circle.clamped_sample(time.now.duration_since(finished_at).as_secs_f64())
                .unwrap();
            self.canvas.draw_circle(
                (pos.x, pos.y),
                radius,
                &self.paints.slider_drag,
            );
        }
    }

    fn draw_circle(
        &mut self,
        pos: &GamePos,
        lifetime: &Lifetime,
    ) {
        self.canvas.save();
        let dead_percentage = self.splines.circle_fade_away_spline.clamped_sample(lifetime.remaining)
            .unwrap(); // note: reversed %
        let mut paint = self.paints.circle_base_paint.clone();
        paint.set_alpha_f(dead_percentage);

        self.canvas.translate((pos.x - BASE_CIRCLE_RADIUS, pos.y - BASE_CIRCLE_RADIUS));

        self.canvas.draw_picture(&self.shapes.circle, None, Some(&paint));

        if lifetime.is_alive() {
            let alive_percentage = self.splines.circle_life_spline.clamped_sample(lifetime.remaining)
                .unwrap();
            self.canvas.draw_circle(
                Point::new(BASE_CIRCLE_RADIUS, BASE_CIRCLE_RADIUS),
                (BASE_CIRCLE_RADIUS * 4.0) - (3.0 * BASE_CIRCLE_RADIUS * alive_percentage),
                &self.paints.approach_circle,
            );
        }

        self.canvas.restore();
    }
}

impl RenderingSystem {
    fn on_resize(&mut self) {
        self.graphics = Graphics::new(&self.window_ctx.clone(), &mut self.gpu_context.clone().borrow_mut());
    }

    pub fn new(
        window_ctx: Rc<glutin::ContextWrapper<glutin::PossiblyCurrent, Window>>,
        gpu_context: Rc<RefCell<GpuContext>>,
    ) -> Self {
        fn get_default_paint() -> Paint {
            let mut default_paint = Paint::default();
            default_paint.set_anti_alias(true);
            default_paint
        }
        let circle_paint = {
            let  builder = get_default_paint();
            builder
        };

        let approach_circle = {
            let mut builder = get_default_paint();
            builder.set_color(Color::from_argb(155, 233, 233, 233));
            builder.set_style(PaintStyle::Stroke);
            builder.set_stroke_width(3.0);
            builder.set_stroke_join(skia_safe::PaintJoin::Round);
            builder.set_stroke_cap(skia_safe::PaintCap::Round);
            builder.set_stroke_miter(6.0);
            builder
        };

        let slider = {
            let mut builder = get_default_paint();
            builder.set_color(Color::from_argb(55, 233, 233, 233));
            builder.set_style(PaintStyle::Stroke);
            builder.set_stroke_width(60.0);
            builder.set_stroke_join(skia_safe::PaintJoin::Round);
            builder.set_stroke_cap(skia_safe::PaintCap::Round);
            builder
        };

        let slider_drag = {
            let mut builder = get_default_paint();
            builder.set_color(Color::from_argb(55, 233, 233, 233));
            builder.set_stroke_join(skia_safe::PaintJoin::Round);
            builder.set_stroke_cap(skia_safe::PaintCap::Round);
            builder
        };

        let cursor = {
            let mut builder = get_default_paint();
            // builder.set_color(Color::from_rgb(25, 118, 210));
            builder.set_color(Color::from_rgb(230, 230, 230));
            builder.set_alpha(170);
            builder.set_style(PaintStyle::Fill);
            builder.set_stroke_width(0.0);
            builder.set_stroke_join(skia_safe::PaintJoin::Round);
            builder.set_stroke_cap(skia_safe::PaintCap::Round);
            builder.set_blend_mode(BlendMode::ColorDodge);
            builder.set_mask_filter(skia_safe::MaskFilter::blur(BlurStyle::Solid, 2.5, None));
            builder
        };


        let follow_circle = {
            let mut builder = get_default_paint();
            builder.set_color(Color::from_rgb(111, 111, 111));
            builder.set_alpha(128);
            builder.set_style(PaintStyle::Fill);
            builder.set_stroke_width(2.0);
            builder
        };

        let default_font_paint = {
            let mut builder = get_default_paint();
            builder.set_color(Color::from_rgb(211, 211, 211));
            builder.set_style(PaintStyle::Fill);
            builder
        };

        let key_cap_off = {
            let mut builder = get_default_paint();
            builder.set_color(Color::from_rgb(111, 111, 111));
            builder.set_alpha(128);
            builder.set_style(PaintStyle::Fill);
            builder.set_stroke_width(2.0);
            builder
        };

        let key_cap_on = {
            let mut builder = get_default_paint();
            builder.set_color(Color::from_rgb(255, 255, 255));
            builder.set_alpha(200);
            builder.set_style(PaintStyle::Fill);
            builder.set_stroke_width(0.0);
            builder
        };

        let circle_fade_away_spline = {
            let start = Key::new(0.0, 1.0, Interpolation::Linear);
            let end = Key::new(-HIT_WINDOW, 0.0, Interpolation::Linear);
            Spline::from_vec(vec![start, end])
        };
        let circle_life_spline = {
            let start = Key::new(LIFETIME, 0.0, Interpolation::Linear);
            let end = Key::new(0.0, 1.0, Interpolation::Linear);
            Spline::from_vec(vec![start, end])
        };

        let slider_hold_circle = {
            let start = Key::new(0.0, BASE_SLIDER_CIRCLE_RADIUS, Interpolation::Linear);
            // not animating for a while
            // to avoid soft jitter if the user smashes his keys 
            let mid = Key::new(0.050, BASE_SLIDER_CIRCLE_RADIUS, Interpolation::Linear);
            let end = Key::new(0.200, BASE_CIRCLE_RADIUS, Interpolation::Linear);
            Spline::from_vec(vec![start, mid, end])
        };

        let key_cap_light_on_spline = {
            let start = Key::new(0.0, 1.0, Interpolation::Linear);
            let end = Key::new(0.2, 0.0, Interpolation::Linear);
            Spline::from_vec(vec![start, end])
        };

        let default_font = {
            let typeface = Typeface::new("Verdana", FontStyle::new(Weight::NORMAL, Width::NORMAL, Slant::Upright))
                .unwrap();
            Font::new(typeface, 18.0)
        };

        let graphics = Graphics::new(&window_ctx.clone(), &mut gpu_context.clone().borrow_mut());


        let circle = {
            let white_paint = {
                let mut b = get_default_paint();
                b.set_color(Color::from_rgb(255, 255, 255));
                b.set_style(PaintStyle::Fill);
                b
            };
            let blue_paint = {
                let mut b = get_default_paint();
                b.set_color(Color::from_rgb(20, 33, 61));
                b.set_style(PaintStyle::Fill);
                b
            };
            let yellow_paint = {
                let mut b = get_default_paint();
                b.set_color(Color::from_rgb(252, 163, 17));
                b.set_style(PaintStyle::Fill);
                b
            };
            let mut recorder = PictureRecorder::new();
            let canvas = recorder.begin_recording(Rect::new(0.0, 0.0, BASE_CIRCLE_DIAMETER, BASE_CIRCLE_DIAMETER), None, None);
            let origin = Point::new(BASE_CIRCLE_RADIUS, BASE_CIRCLE_RADIUS);
            canvas.draw_circle(origin, BASE_CIRCLE_RADIUS, &white_paint);
            canvas.draw_circle(origin, BASE_CIRCLE_RADIUS - 3.0, &blue_paint);
            canvas.draw_circle(origin, BASE_CIRCLE_RADIUS - 9.0, &white_paint);
            canvas.draw_circle(origin, BASE_CIRCLE_RADIUS - 12.0, &yellow_paint);

            recorder.finish_recording_as_picture(None).unwrap()
        };

        return Self {
            graphics,
            gpu_context: gpu_context.clone(),
            window_ctx: window_ctx.clone(),
            shapes: Shapes {
                circle,
            },
            paints: Paints {
                font_default: default_font_paint,
                circle_base_paint: circle_paint,
                approach_circle,
                follow_circle,
                cursor,
                key_cap_on,
                key_cap_off,
                slider,
                slider_drag,
            },
            splines: Splines {
                circle_fade_away_spline,
                circle_life_spline,
                key_cap_light_on_spline,
                slider_hold_circle,
            },
            fonts: Fonts {
                default: default_font,
            },
        };
    }
}
