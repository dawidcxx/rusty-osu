use crate::components::*;
use crate::consts::{BASE_CIRCLE_RADIUS, HIT_WINDOW, LIFETIME};
use crate::game_thread::EventLoopMsg;
use crate::resources::*;
use crate::utils::{circle_contains_point, lerp, btree_gt, btree_less};
use kira::instance::{InstanceSettings, StopInstanceSettings};
use kira::manager::{AudioManager, AudioManagerSettings};
use kira::sound::{SoundSettings};
use specs::{
    Builder, Entities, Join, LazyUpdate, Read, ReadStorage, System, WorldExt, Write,
    WriteStorage,
};
use std::{ops::Deref};
use std::time::{Instant};
use crate::osu_parser::*;
use kira::sound::handle::SoundHandle;
use kira::instance::handle::InstanceHandle;
use kira::parameter::tween::{Tween};

pub struct ObjectSpawnerSystem {
    beatmap: OsuBeatMap,
    current_hit_object_index: usize,
    timing_points_lookup: std::collections::BTreeMap<u64, f64>,
}

impl Default for ObjectSpawnerSystem {
    fn default() -> Self {
        const OSU_MAP: &'static str = include_str!("Niko - Made of Fire (lesjuh) [Oni].osu");
        let beatmap = parse_osu_file(
            OSU_MAP.lines(),
            OsuBeatMapParseConfig {
                pre_add_audio_lead_in: true
            },
        );

        let mut timings_lookup = std::collections::BTreeMap::new();

        beatmap.timing_points.iter().for_each(|timing| {
            timings_lookup.insert(timing.time_offset_in_millis, timing.beat_length);
        });

        return Self {
            beatmap,
            current_hit_object_index: 0,
            timing_points_lookup: timings_lookup,
        };
    }
}

impl<'a> System<'a> for ObjectSpawnerSystem {
    type SystemData = (
        Read<'a, Time>,
        Entities<'a>,
        Read<'a, LazyUpdate>,
    );

    fn run(&mut self, (
        time,
        entities,
        updater
    ): Self::SystemData) {
        if let Some(obj) = self.beatmap.hit_objects.get(self.current_hit_object_index) {
            if time.secs_since_start + LIFETIME >= obj.time_offset_in_secs {
                let mut builder = updater
                    .create_entity(entities.deref())
                    .with(GamePos {
                        x: obj.x,
                        y: obj.y,
                    })
                    .with(Lifetime {
                        remaining: obj.time_offset_in_secs - time.secs_since_start,
                    })
                    .with(CircleHitRating::default())
                    .with(HitSound { value: obj.hit_sound });

                builder = if let Some(params) = &obj.object_params {
                    match params {
                        OsuBeatMapHitObjectParams::HitCircle => {
                            builder
                                .with(Circle)
                        }
                        OsuBeatMapHitObjectParams::Slider(slider_data) => {
                            let mut path = skia_safe::Path::new();
                            path.move_to((obj.x, obj.y));
                            let slider_curve = match slider_data.curve_points.len() {
                                1 => {
                                    let end = slider_data.curve_points[0];
                                    path.line_to(end);
                                    SliderCurve::Linear(SliderCurveLinear { start: (obj.x, obj.y), end })
                                }
                                2 => {
                                    let p1 = slider_data.curve_points[0];
                                    let p2 = slider_data.curve_points[1];
                                    path.quad_to(p1, p2);
                                    SliderCurve::QuadBezier(SliderCurveQuadBezier {
                                        start: (obj.x, obj.y),
                                        control_point: p1,
                                        end: p2,
                                    })
                                }
                                3 => {
                                    let p1 = slider_data.curve_points[0];
                                    let p2 = slider_data.curve_points[1];
                                    let p3 = slider_data.curve_points[2];
                                    path.cubic_to(p1, p2, p3);
                                    SliderCurve::CubicBezier(SliderCurveCubicBezier {
                                        start: (obj.x, obj.y),
                                        control_point: p1,
                                        control_point_2: p2,
                                        end: p3,
                                    })
                                }
                                point_count => {
                                    unimplemented!("Sliders with point count {} is not implemented", point_count)
                                }
                            };

                            // find the closest timing point
                            let timing_point = btree_gt(&self.timing_points_lookup, obj.time_offset_in_millis)
                                .unwrap_or_else(|| btree_less(&self.timing_points_lookup, obj.time_offset_in_millis).unwrap());

                            // do some osu math, https://osu.ppy.sh/wiki/fi/osu!_File_Formats/Osu_(file_format)#sliders
                            let slider_duration = slider_data.length / (self.beatmap.slider_multiplier * 100.0) * timing_point / 1000.0;

                            builder.with(Slider {
                                curve_points: slider_data.curve_points.clone(),
                                duration_in_secs: slider_duration,
                                progress: 0.0,
                                skia_path: path,
                                curve: slider_curve,
                                state: SliderState::UNTOUCHED,
                            })
                        }
                    }
                } else {
                    builder
                };

                builder.build();

                self.current_hit_object_index += 1;
            }
        };
    }
    fn setup(&mut self, world: &mut specs::World) {
        let mut events = world.fetch_mut::<GameEvents>();
        events.emit(GameEvent::SongLoad("./assets/Niko - Made of Fire.mp3".to_string()));
    }
}

pub struct CircleLifetimeSystem;

impl<'a> System<'a> for CircleLifetimeSystem {
    type SystemData = (
        ReadStorage<'a, Lifetime>,
        ReadStorage<'a, Circle>,
        Entities<'a>,
        Read<'a, LazyUpdate>,
    );

    fn run(&mut self, (
        lifetimes,
        circles,
        entities,
        updater,
    ): Self::SystemData) {
        for (_, lifetime, entity) in (&circles, &lifetimes, &entities).join() {
            if lifetime.remaining <= -HIT_WINDOW {
                updater.insert(
                    entity,
                    DespawnObject {
                        reason: DespawnObjectReason::CircleHit(CircleHitRating::MISS),
                        despawned_at: Instant::now(),
                    },
                );
            }
        }
    }
}

pub struct SliderLifetimeSystem;

impl<'a> System<'a> for SliderLifetimeSystem {
    type SystemData = (
        ReadStorage<'a, Lifetime>,
        WriteStorage<'a, Slider>,
        WriteStorage<'a, GamePos>,
        Entities<'a>,
        Read<'a, LazyUpdate>,
        Read<'a, Time>,
        Write<'a, GameEvents>,
    );

    fn run(&mut self, (
        lifetimes,
        mut sliders,
        mut game_poses,
        entities,
        updater,
        time,
        mut game_events,
    ): Self::SystemData) {
        for (
            lifetime,
            slider,
            pos,
            entity,
        ) in (&lifetimes, &mut sliders, &mut game_poses, &entities).join() {
            if lifetime.remaining <= 0.0 {
                // start progressing the slider
                slider.progress = lifetime.remaining.abs() / slider.duration_in_secs;
                let t = slider.progress as f32;
                match &slider.curve {
                    SliderCurve::Linear(line) => {
                        pos.x = lerp(line.start.0, line.end.0, t);
                        pos.y = lerp(line.start.1, line.end.1, t);
                    }
                    SliderCurve::QuadBezier(quad) => {
                        pos.x = (1.0 - t).powi(2) * quad.start.0 + (1.0 - t) * 2.0 * t
                            * quad.control_point.0 + t * t * quad.end.0;
                        pos.y = (1.0 - t).powi(2) * quad.start.1 + (1.0 - t) * 2.0 * t
                            * quad.control_point.1 + t * t * quad.end.1;
                    }
                    SliderCurve::CubicBezier(c) => {
                        pos.x = (1.0 - t).powi(3) * c.start.0 +
                            (1.0 - t).powi(2) * 3.0 * t * c.control_point.0 +
                            (1.0 - t) * 3.0 * t * t * c.control_point_2.0 +
                            t * t * t * c.end.0;
                        pos.y = (1.0 - t).powi(3) * c.start.1 +
                            (1.0 - t).powi(2) * 3.0 * t * c.control_point.1 +
                            (1.0 - t) * 3.0 * t * t * c.control_point_2.1 +
                            t * t * t * c.end.1;
                    }
                };

                if lifetime.remaining.abs() >= slider.duration_in_secs {
                    if let SliderState::DRAGGING(v) = slider.state {
                        let change = slider.go_to_state(SliderState::FINISHED(v / slider.duration_in_secs, time.now));
                        game_events.emit_on_slider_change(change);
                    }
                    updater.insert(
                        entity,
                        DespawnObject {
                            reason: DespawnObjectReason::SliderEnd(slider.state),
                            despawned_at: Instant::now(),
                        },
                    );
                }
            };
        }
    }
}

pub struct LifetimeSystem;

impl<'a> System<'a> for LifetimeSystem {
    type SystemData = (
        Read<'a, Time>,
        WriteStorage<'a, Lifetime>,
        WriteStorage<'a, DespawnObject>,
        Entities<'a>,
        Read<'a, LazyUpdate>,
    );

    fn run(
        &mut self,
        (
            time,
            mut lifetimes,
            mut despawn_objects,
            entities,
            updater
        ): Self::SystemData,
    ) {
        // progress lifetime
        for lifetime in (&mut lifetimes).join() {
            lifetime.remaining -= time.delta_seconds;
        }

        // delete flagged entities
        for (_, entity) in (&mut despawn_objects, &entities).join() {
            updater.exec_mut(move |world| {
                world.delete_entity(entity).unwrap();
            });
        }
    }
}

pub struct TrailSystem;

impl<'a> System<'a> for TrailSystem {
    type SystemData = (
        Read<'a, Time>,
        Read<'a, GameCursor>,
        Write<'a, Trail>,
        Write<'a, TrailTimer>,
    );

    fn run(&mut self, (time, game_cursor, mut trail, mut timer): Self::SystemData) {
        if timer.0.tick(&time) {
            trail.add((game_cursor.window_x, game_cursor.window_y));
            timer.0.reset();
        }
    }
}

pub struct HitSystem;

impl<'a> System<'a> for HitSystem {
    type SystemData = (
        Write<'a, GameEvents>,
        Read<'a, Time>,
        Read<'a, GameArea>,
        Read<'a, GameInputState>,
        Read<'a, GameCursor>,
        ReadStorage<'a, Circle>,
        WriteStorage<'a, Slider>,
        ReadStorage<'a, Lifetime>,
        ReadStorage<'a, GamePos>,
        WriteStorage<'a, CircleHitRating>,
        Read<'a, LazyUpdate>,
        Entities<'a>,
    );

    fn run(
        &mut self,
        (
            mut game_events,
            time,
            game_area,
            input_state,
            cursor,
            circles,
            mut sliders,
            lifetimes,
            game_poses,
            mut hit_rating,
            updater,
            entities,
        ): Self::SystemData,
    ) {
        let scaled_circle_radius = BASE_CIRCLE_RADIUS * game_area.scale();
        let scaled_slider_circle_radius = scaled_circle_radius * 1.2;
        let hit_bindings: Vec<&'static GameInputKeyBinding> = vec![
            &GameInputKeyBinding::Hit1,
            &GameInputKeyBinding::Hit2,
        ];

        fn is_hit(
            hit: (f32, f32),
            circle_cords: (f32, f32),
            scaled_circle_radius: f32,
        ) -> bool {
            circle_contains_point(hit.0, hit.1, circle_cords.0, circle_cords.1, scaled_circle_radius)
        }

        let mut process_circle_hit = |binding: &'static GameInputKeyBinding| {
            if input_state.is_key_active(binding) {
                for (
                    _, // <- unused, but still needed so we only select circles
                    lifetime,
                    pos,
                    hit_rating,
                    entity,
                ) in (&circles, &lifetimes, &game_poses, &mut hit_rating, &entities).join() {
                    if lifetime.is_in_hit_zone() {
                        let circle_cords = game_area.game_cords_to_screen((pos.x, pos.y));
                        if is_hit((cursor.window_x, cursor.window_y), circle_cords, scaled_circle_radius) {
                            *hit_rating = if lifetime.is_in_perfect_hit_zone() {
                                CircleHitRating::GREAT
                            } else {
                                CircleHitRating::GOOD
                            };
                            updater.insert(
                                entity,
                                DespawnObject {
                                    reason: DespawnObjectReason::CircleHit(hit_rating.clone()),
                                    despawned_at: time.now.clone(),
                                },
                            );
                            break;
                        }
                    }
                }
            }
        };

        let mut process_slider_hold = |bindings: Vec<&'static GameInputKeyBinding>| {
            let is_holding = bindings.into_iter()
                .any(|b| input_state.is_key_hold(b));
            for (slider, lifetime, pos) in (&mut sliders, &lifetimes, &game_poses).join() {
                if lifetime.is_in_hit_zone() || lifetime.remaining < 0.0 {
                    let circle_cords = game_area.game_cords_to_screen((pos.x, pos.y));
                    let hit_check = || is_hit((cursor.window_x, cursor.window_y), circle_cords, scaled_slider_circle_radius);
                    let mut change = SliderStateChange::NoChange;

                    match slider.state {
                        SliderState::UNTOUCHED => {
                            if is_holding && hit_check() {
                                change = slider.go_to_state(SliderState::DRAGGING(0.0));
                            }
                        }
                        SliderState::DRAGGING(mut completed_secs) => {
                            if is_holding && hit_check() {
                                if slider.progress <= 1.0 {
                                    completed_secs += time.delta_seconds;
                                    change = slider.go_to_state(SliderState::DRAGGING(completed_secs));
                                } else {
                                    change = slider.go_to_state(SliderState::FINISHED(completed_secs / slider.duration_in_secs, time.now));
                                }
                            } else {
                                change = slider.go_to_state(SliderState::FINISHED(completed_secs / slider.duration_in_secs, time.now));
                            }
                        }
                        SliderState::FINISHED(percent_complete, _) => {
                            if slider.progress < 1.0 && is_holding && hit_check() {
                                change = slider.go_to_state(SliderState::DRAGGING(percent_complete * slider.duration_in_secs));
                            }
                        }
                    }

                    game_events.emit_on_slider_change(change);
                }
            }
        };


        hit_bindings.iter().for_each(|&binding| process_circle_hit(binding));
        process_slider_hold(hit_bindings);
    }
}

pub struct ScoringSystem;

impl<'a> System<'a> for ScoringSystem {
    type SystemData = (
        ReadStorage<'a, DespawnObject>,
        Write<'a, Hp>,
        Write<'a, Combo>,
        Write<'a, Score>,
    );

    fn run(&mut self, (objects, mut hp, mut combo, mut score): Self::SystemData) {
        for object in (&objects).join() {
            match &object.reason {
                DespawnObjectReason::CircleHit(rating) => match rating {
                    CircleHitRating::MISS => {
                        hp.drain();
                        combo.reset();
                    }
                    CircleHitRating::GOOD => {
                        combo.maintain();
                        hp.fill();
                        score.on_good(&combo);
                    }
                    CircleHitRating::GREAT => {
                        combo.maintain();
                        hp.fill();
                        hp.fill();
                        score.on_great(&combo);
                    }
                },
                DespawnObjectReason::SliderEnd(slider_score) => {
                    match &slider_score {
                        SliderState::UNTOUCHED => {
                            hp.drain();
                            combo.reset();
                        }
                        SliderState::DRAGGING(_) => { unreachable!("Despawned a dragging slider") }
                        SliderState::FINISHED(percent_completed, _) => {
                            if percent_completed > &0.6 {
                                hp.fill();
                                combo.maintain();
                                score.on_great(&combo);
                            } else if percent_completed > &0.2 {
                                hp.fill();
                                combo.maintain();
                                score.on_good(&combo);
                            }
                        }
                    }
                }
            }
        }
    }
}

pub struct InputSystem;

impl<'a> System<'a> for InputSystem {
    type SystemData = (
        Read<'a, Vec<EventLoopMsg>>,
        Read<'a, Time>,
        Write<'a, GameCursor>,
        Write<'a, GameInputState>,
    );

    fn run(
        &mut self,
        (event_loop_messages, time, mut game_cursor, mut game_input_state): Self::SystemData,
    ) {
        game_input_state.clear_frame();


        for event_loop_msg in event_loop_messages.iter() {
            match event_loop_msg {
                EventLoopMsg::Quit => {
                    game_input_state.quitting = true;
                }
                EventLoopMsg::KeyPressed(key) => {
                    game_input_state.active_set.insert(key.clone());
                    game_input_state.last_active_keys_map.insert(key.clone(), time.now);
                    game_input_state.hold_set.insert(key.clone());
                }
                EventLoopMsg::Resized(_, _) => {}
                EventLoopMsg::MouseMovedBy(x, y) => {
                    game_cursor.window_x = *x as f32;
                    game_cursor.window_y = *y as f32;
                }
                EventLoopMsg::KeyReleased(key) => {
                    game_input_state.hold_set.remove(key);
                }
            }
        }
    }
}

pub struct AudioSystem {
    pub manager: AudioManager,
    pub current_song: Option<InstanceHandle>,
    pub hit_sound_normal: SoundHandle,
    pub hit_sound_finish: SoundHandle,
    pub hit_sound_clap: SoundHandle,
    pub hit_sound_whistle: SoundHandle,
    pub slider: SoundHandle,
}

impl Default for AudioSystem {
    fn default() -> Self {
        let mut audio_manager = AudioManager::new(AudioManagerSettings::default())
            .unwrap();

        let mut load = |url: &'static str, settings: SoundSettings| audio_manager
            .load_sound(url, settings)
            .expect(format!("Failed to load sound {}", url).as_str());

        let hit_normal = load("assets/soft-hitnormal.wav", SoundSettings::default());
        let hit_whistle = load("assets/soft-hitwhistle.wav", SoundSettings::default());
        let hit_finish = load("assets/soft-hitfinish.wav", SoundSettings::default());
        let hit_clap = load("assets/soft-hitclap.wav", SoundSettings::default());
        let slider = load("assets/soft-sliderslide.wav", SoundSettings {
            default_loop_start: Some(0.0),
            ..SoundSettings::default()
        });

        Self {
            manager: audio_manager,
            current_song: None,
            hit_sound_normal: hit_normal,
            hit_sound_finish: hit_finish,
            hit_sound_clap: hit_clap,
            hit_sound_whistle: hit_whistle,
            slider,
        }
    }
}

impl<'a> System<'a> for AudioSystem {
    type SystemData = (
        Read<'a, GameEvents>,
        ReadStorage<'a, DespawnObject>,
        ReadStorage<'a, HitSound>,
    );

    fn run(&mut self, (events, despawn_objects, hit_sounds): Self::SystemData) {
        events.on_song_load(|song| {
            let mut song = self
                .manager
                .load_sound(song, SoundSettings::default())
                .expect("Failed to load song");
            let handle = song.play(InstanceSettings::default())
                .unwrap();
            self.current_song = Some(handle);
        });

        events.on_slider_start(|| {
            self.slider.play(InstanceSettings::default()).unwrap();
        });

        events.on_slider_end(|| {
            let mut stop_settings = StopInstanceSettings::default();
            stop_settings.fade_tween = Some(Tween::linear(0.300));
            self.slider.stop(StopInstanceSettings::default()).unwrap();
        });

        for (despawn, hit_sound) in (&despawn_objects, &hit_sounds).join() {
            match &despawn.reason {
                DespawnObjectReason::CircleHit(reason) => {
                    match reason {
                        CircleHitRating::MISS => {}
                        CircleHitRating::GOOD | CircleHitRating::GREAT => {
                            let sound = match &hit_sound.value {
                                OsuHitObjectHitSound::Normal => &mut self.hit_sound_normal,
                                OsuHitObjectHitSound::Whistle => &mut self.hit_sound_whistle,
                                OsuHitObjectHitSound::Finish => &mut self.hit_sound_finish,
                                OsuHitObjectHitSound::Clap => &mut self.hit_sound_clap,
                            };
                            sound.play(InstanceSettings::default())
                                .unwrap();
                        }
                    };
                }
                DespawnObjectReason::SliderEnd(_) => {
                    self.hit_sound_finish.play(InstanceSettings::default())
                        .unwrap();

                }
            }
        }
    }
}
