use std::str::{Lines, FromStr};
use std::collections::HashMap;
use crate::utils::{is_bit_set, is_nth_bit_set};
use std::time::Duration;

type OsuDecimal = f64;

#[derive(Debug, Clone)]
pub struct OsuBeatMap {
    pub audio_file_name: String,
    pub audio_lead_in: OsuDecimal,
    pub stack_leniency: OsuDecimal,
    pub slider_multiplier: OsuDecimal,
    pub timing_points: Vec<TimingPoint>,
    pub hit_objects: Vec<OsuBeatMapHitObject>,
}

#[derive(Default, Copy, Clone, Debug)]
pub struct OsuBeatMapParseConfig {
    pub pre_add_audio_lead_in: bool,
}

pub fn parse_osu_file(
    lines: Lines,
    config: OsuBeatMapParseConfig,
) -> OsuBeatMap {
    // main iterator
    let mut it = lines.into_iter();

    // properties we wish to collect while parsing
    let mut strings = HashMap::new();
    let mut decimals = HashMap::new();
    let mut timing_points = Vec::new();
    let mut hit_objects = Vec::with_capacity(1024);

    while let Some(line) = it.next() {
        if line.starts_with("[") && line.ends_with("]") {
            let section = line
                .chars()
                .skip(1)
                .take(line.len() - 2)
                .collect::<String>();

            match section.as_str() {
                "General" => parse_section(&mut it, |line| {
                    let (key, value_raw) = key_value_line(line);
                    match key.as_str() {
                        "AudioFilename" => on_string_key_value(&mut strings, key, value_raw),
                        "AudioLeadIn" => on_decimal_key_value(&mut decimals, key, value_raw),
                        "StackLeniency" => on_decimal_key_value(&mut decimals, key, value_raw),
                        _ => return
                    };
                }),
                "Difficulty" => parse_section(&mut it, |line| {
                    let (key, value_raw) = key_value_line(line);
                    match key.as_str() {
                        "SliderMultiplier" => on_decimal_key_value(&mut decimals, key, value_raw),
                        _ => return
                    };
                }),
                "TimingPoints" => parse_section(&mut it, |line| {
                    let values = line.split(",")
                        .map(OsuDecimal::from_str)
                        .map(Result::unwrap)
                        .collect::<Vec<OsuDecimal>>();
                    let time_offset = values[0] as u64;
                    let beat_length = values[1];
                    let inherited = values[6] == 0.0;
                    timing_points.push(TimingPoint {
                        time_offset_in_millis: time_offset,
                        beat_length,
                        inherited,
                    });
                }),
                "HitObjects" => parse_section(&mut it, |line| {
                    let rows = line.split(",")
                        .collect::<Vec<_>>();
                    let x = f32::from_str(rows[0]).unwrap();
                    let y = f32::from_str(rows[1]).unwrap();

                    let time_offset_in_millis = u64::from_str(rows[2]).unwrap();
                    let time_offset = Duration::from_millis(time_offset_in_millis);
                    let time_offset_in_secs = time_offset.as_secs_f64();

                    let hit_sound = {
                        let raw = u8::from_str(rows[4]).unwrap();
                        if is_bit_set(raw, 0) {
                            OsuHitObjectHitSound::Normal
                        } else if is_bit_set(raw, 1) {
                            OsuHitObjectHitSound::Whistle
                        } else if is_bit_set(raw, 2) {
                            OsuHitObjectHitSound::Finish
                        } else if is_bit_set(raw, 3) {
                            OsuHitObjectHitSound::Clap
                        } else {
                            unreachable!("Unparsed Hit Sound {:?}", raw)
                        }
                    };
                    let params = {
                        let hit_obj_type = u8::from_str(rows[3]).unwrap();
                        if is_nth_bit_set(hit_obj_type, 0) {
                            Some(OsuBeatMapHitObjectParams::HitCircle)
                        } else if is_nth_bit_set(hit_obj_type, 1) {
                            let params = rows[5].split("|")
                                .collect::<Vec<_>>();
                            let curve_type = match params[0] {
                                "B" => OsuBeatSliderCurveType::Bezier,
                                "C" => OsuBeatSliderCurveType::ComRom,
                                "L" => OsuBeatSliderCurveType::Linear,
                                "P" => OsuBeatSliderCurveType::PerfectCircle,
                                curve_type => {
                                    unreachable!("Unexpected curve type given {}", curve_type);
                                }
                            };

                            let points = params.iter().skip(1)
                                .map(|&point_raw| {
                                    let xy = point_raw.split(":").collect::<Vec<_>>();
                                    let x_raw = xy.get(0)
                                        .expect("HitObject/Slider Parse Error: curve points x");
                                    let y_raw = xy.get(1)
                                        .expect("HitObject/Slider Parse Error: curve points y");
                                    let x = f32::from_str(x_raw).unwrap();
                                    let y = f32::from_str(y_raw).unwrap();
                                    (x, y)
                                })
                                .collect::<Vec<_>>();

                            let slides = i32::from_str(rows[6]).unwrap();
                            let length = f64::from_str(rows[7]).unwrap();

                            let params = OsuBeatMapHitObjectSliderParams {
                                curve_type,
                                curve_points: points,
                                slides,
                                length,
                            };
                            Some(OsuBeatMapHitObjectParams::Slider(params))
                        } else {
                            None
                        }
                    };

                    hit_objects.push(OsuBeatMapHitObject {
                        x,
                        y,
                        time_offset_in_secs,
                        time_offset_in_millis,
                        hit_sound,
                        object_params: params,
                    });
                }),
                section => {
                    log::debug!("OsuParser: unhandled section {}", section);
                }
            };
        }
    };

    if config.pre_add_audio_lead_in {
        let audio_lead_in_in_ms = decimals["AudioLeadIn"].clone() as u64;
        let audio_lead_in_in_secs = Duration::from_millis(audio_lead_in_in_ms)
            .as_secs_f64();
        for hit_object in hit_objects.iter_mut() {
            hit_object.time_offset_in_secs += audio_lead_in_in_secs;
            hit_object.time_offset_in_millis += audio_lead_in_in_ms;
        }
        for timing_point in timing_points.iter_mut() {
            timing_point.time_offset_in_millis += audio_lead_in_in_ms;
        }
    };

    return OsuBeatMap {
        audio_file_name: strings["AudioFilename"].clone(),
        audio_lead_in: decimals["AudioLeadIn"].clone(),
        stack_leniency: decimals["StackLeniency"].clone(),
        slider_multiplier: decimals["SliderMultiplier"].clone(),
        timing_points,
        hit_objects,
    };
}

#[derive(Debug, Clone)]
pub struct TimingPoint {
    pub time_offset_in_millis: u64,
    pub beat_length: f64,
    pub inherited: bool,
}

#[derive(Debug, Clone)]
pub struct OsuBeatMapHitObject {
    pub x: f32,
    pub y: f32,
    pub time_offset_in_secs: f64,
    pub time_offset_in_millis: u64,
    pub hit_sound: OsuHitObjectHitSound,
    pub object_params: Option<OsuBeatMapHitObjectParams>,
}

#[derive(Debug, Clone)]
pub enum OsuBeatMapHitObjectParams {
    HitCircle,
    Slider(OsuBeatMapHitObjectSliderParams),
}

#[derive(Debug, Clone)]
pub struct OsuBeatMapHitObjectSliderParams {
    pub curve_type: OsuBeatSliderCurveType,
    pub curve_points: Vec<(f32, f32)>,
    pub slides: i32,
    pub length: f64,
}

#[derive(Debug, Copy, Clone)]
pub enum OsuBeatSliderCurveType {
    Bezier,
    ComRom,
    Linear,
    PerfectCircle,
}

#[derive(Debug, Copy, Clone)]
pub enum OsuHitObjectHitSound {
    Normal,
    Whistle,
    Finish,
    Clap,
}

// functions


fn on_decimal_key_value(decimals: &mut HashMap<String, OsuDecimal>, key: String, value_raw: String) {
    let value = get_decimal_value(&key, &value_raw);
    decimals.insert(key, value);
}

fn on_string_key_value(
    strings: &mut HashMap<String, String>,
    key: String,
    value_raw: String,
) {
    strings.insert(key, value_raw);
}

// helper functions
fn parse_section<F: FnMut(&str)>(
    it: &mut Lines,
    mut on_line: F,
) {
    while let Some(line) = it.next() {
        if line.is_empty() {
            break;
        }
        on_line(line);
    }
}

fn key_value_line(line: &str) -> (String, String) {
    let key_value_vector = line.split(":")
        .map(str::trim)
        .map(str::to_string)
        .collect::<Vec<_>>();
    (key_value_vector[0].clone(), key_value_vector[1].clone())
}

fn get_decimal_value(key: &String, value_raw: &String) -> OsuDecimal {
    let value = OsuDecimal::from_str(value_raw.as_str())
        .expect(format!("Failed to parse {} as a decimal value = {}", key, value_raw).as_str());
    value
}
