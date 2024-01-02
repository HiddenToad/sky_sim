use nannou::color::{Gradient, IntoLinSrgba};
use nannou::noise::{Billow, Exponent, NoiseFn};
use nannou::prelude::*;
use rayon::prelude::*;
use std::marker::PhantomData;
use std::ops::Deref;

const SUN_RADIUS: u32 = 30;
const SUN_AURA_SIZE: u32 = 30;
const SUN_START_X: f32 = SCREEN_SIZE_F / 2.;
const SUN_START_Y: f32 = SCREEN_SIZE_F * 0.8;
const SUN_ROTATE_POINT: (f32, f32) = (SCREEN_SIZE_F / 2., 0.);
const SUN_CYCLE_SPEED: f32 = 0.07;

const STAR_COUNT: usize = 30;
const STAR_RADIUS: f32 = 2.;
const STAR_AURA_SIZE: u32 = 6;

const MOON_RADIUS: u32 = (SUN_RADIUS / 2) + (SUN_RADIUS / 5);
const MOON_POS: (f32, f32) = (SCREEN_SIZE_F / 4., SUN_START_Y * 1.13);
const MOON_AURA_SIZE: u32 = MOON_RADIUS / 2;
const MOON_SPOTS_COLOR: Srgb<u8> = DARKGRAY;

const CLOUD_NIGHT_COLOR: Srgb<u8> = GRAY;
const NIGHT_SKY_COLOR: Srgb<u8> = rgb(20, 30, 37);
const SUNSET_SKY_COLOR: Srgb<u8> = rgb(254, 172, 39);

const BILLOW_OCTAVES: usize = 6;
const WIND_SPEED: f64 = 20.;
const SPEED_MULTIPLIER: f64 = 0.00005;
const SCREEN_SIZE: u32 = 450;
const SCREEN_SIZE_F: f32 = SCREEN_SIZE as f32;
const PIXELS_PER_POINT: u32 = 3;
const PIXELS_PER_POINT_F: f32 = PIXELS_PER_POINT as f32;
const NUM_POINTS: u32 = SCREEN_SIZE / PIXELS_PER_POINT;
const ZERO_ALPHA_THRESHOLD: f64 = 0.6;
const ALPHA_ZERO_SCALING: f64 = 1.2;
const SPEEDUP_FACTOR: f64 = 9.5;
const Y_OFFSET: f64 = 50.;

const fn rgb(red: u8, green: u8, blue: u8) -> Srgb<u8> {
    Rgb {
        red,
        green,
        blue,
        standard: PhantomData,
    }
}

type Points = [[f64; (NUM_POINTS) as usize]; (NUM_POINTS) as usize];
type Color = Rgba<u8>;

fn main() {
    nannou::app(model).update(update).run();
}

#[inline(always)]
fn with_alpha(c: Color, alpha: f64) -> Color {
    let alpha = map_range(alpha, 0., 1., 0, 255);
    Rgba { alpha, ..c }
}

#[inline(always)]
fn u_fmul(u: u8, f: f64) -> u8 {
    (u as f64 * f) as u8
}

#[inline]
fn darken_by(c: Color, factor: f64) -> Color {
    let red = u_fmul(c.red, 1. - factor);
    let green = u_fmul(c.green, 1. - factor);
    let blue = u_fmul(c.blue, 1. - factor);

    Color {
        color: Rgb {
            red,
            green,
            blue,
            ..c.color
        },
        ..c
    }
}

#[inline(always)]
fn collide_circle_point(p: Point2, cp: Point2, r: f32) -> bool {
    p.distance(cp) <= r
}

#[inline(always)]
fn white_with_alpha(alpha: f64) -> Color {
    with_alpha(WHITE.into(), alpha)
}

struct Sun {
    pos: Point2,
}

impl Sun {
    fn new(pos: Point2) -> Self {
        Self { pos }
    }
    fn advance_sun_pos(&mut self, frames: u64) {
        let sx = SUN_START_X;
        let sy = SUN_START_Y;
        let frames = frames as f32;
        let increments = 360. / SUN_CYCLE_SPEED;
        let angle = -deg_to_rad((frames % increments) * SUN_CYCLE_SPEED);
        let x = SUN_ROTATE_POINT.0 + angle.cos() * (sx - SUN_ROTATE_POINT.0)
            - angle.sin() * (sy - SUN_ROTATE_POINT.1);
        let y = SUN_ROTATE_POINT.1
            + angle.sin() * (sx - SUN_ROTATE_POINT.0)
            + angle.cos() * (sy - SUN_ROTATE_POINT.1);
        self.pos = pt2(x, y)
    }

    fn transition_sky_color(amount: f32) -> Rgb<u8> {
        let gradient = Gradient::new(
            [
                LIGHTSKYBLUE.into_lin_srgba(),
                SUNSET_SKY_COLOR.into_lin_srgba(),
                NIGHT_SKY_COLOR.into_lin_srgba(),
            ]
            .into_iter(),
        );
        let mut take = gradient.take(101);
        let c = Rgba::from_linear(take.nth(map_range(amount, 0., 1., 0, 100)).unwrap());
        let red = map_range(c.red, 0., 1., 0, 255);
        let green = map_range(c.green, 0., 1., 0, 255);
        let blue = map_range(c.blue, 0., 1., 0, 255);
        Rgb::new(red, green, blue)
    }

    fn rising_amount(&self) -> Option<f32> {
        let p = &self.pos;
        let edge_x = p.x - SUN_RADIUS as f32;
        if edge_x <= 0. && self.has_set() {
            let amt = map_range(edge_x, (SUN_RADIUS as f32) * -2., 0., 0., 1.);
            let amt = clamp(amt, 0., 1.);
            let amt = 1. - amt.log10().abs();
            let amt = clamp(amt, 0., 1.);

            Some(amt)
        } else {
            None
        }
    }

    fn setting_amount(&self) -> Option<f32> {
        let p = &self.pos;
        let edge_x = p.x + SUN_RADIUS as f32;
        if edge_x >= SCREEN_SIZE_F && !self.has_set() {
            let amt = map_range(
                edge_x,
                SCREEN_SIZE_F,
                SCREEN_SIZE_F + (SUN_RADIUS as f32) * 2.,
                0.,
                1.,
            );
            let amt = clamp(amt, 0., 1.);
            let amt = 1. - amt.log10().abs();
            let amt = clamp(amt, 0., 1.);
            Some(amt)
        } else {
            None
        }
    }
    fn has_set(&self) -> bool {
        let p = &self.pos;
        !((p.x - SUN_RADIUS as f32) > 0. && p.y > 0. && p.x - (SUN_RADIUS as f32 + SUN_AURA_SIZE as f32) < SCREEN_SIZE_F)
    }
}

struct Stars {
    points: [Point2; STAR_COUNT],
}

impl Stars {
    fn random_sky() -> Self {
        let mut stars = vec![];
        for _ in 0..STAR_COUNT {
            stars.push(Point2::new(
                random_f32() * SCREEN_SIZE_F,
                random_f32() * SCREEN_SIZE_F,
            ))
        }
        Stars {
            points: stars
                .try_into()
                .expect("stars slice equals length of star count"),
        }
    }
}

impl Deref for Stars {
    type Target = [Point2; STAR_COUNT];
    fn deref(&self) -> &Self::Target {
        &self.points
    }
}

struct Moon {
    texture: Vec<(Point2, f64)>,
}

impl Moon {
    fn new() -> Self {
        let mut texture = vec![];
        let mut billow = Billow::new();
        billow.persistence = 0.15;
        let noise = Exponent::<[f64; 2]>::new(&billow);
        let x = MOON_POS.0;
        let y = MOON_POS.1;
        let center = pt2(x, y);
        let r_i = MOON_RADIUS as i64;

        for i in -r_i..r_i {
            for j in -r_i..r_i {
                let px = x - i as f32;
                let py = y - j as f32;
                let point = pt2(px, py);
                if point.distance(center) < MOON_RADIUS as f32 {
                    let mut alpha = noise.get([px as f64 / 2., py as f64 / 2.]).abs() * 0.75;
                    if alpha < 0.2 {
                        alpha = 0.;
                    } else {
                        alpha = map_range(alpha, 0.2, 1., 0., 0.85);
                    }

                    texture.push((pt2(px, py), alpha));
                }
            }
        }

        Self { texture }
    }
}

struct Model {
    _window: window::Id,
    points: Points,
    billow: Billow,
    sun: Sun,
    sky_color: Color,
    darkened_sky_color: Color,
    stars: Stars,
    moon: Moon,
    speedup: bool,
}

fn model(app: &App) -> Model {
    let _window = app
        .new_window()
        .view(view)
        .event(event)
        .size(SCREEN_SIZE, SCREEN_SIZE)
        .build()
        .unwrap();
    let points = [[0.; (NUM_POINTS) as usize]; (NUM_POINTS) as usize];
    let mut billow = Billow::new();
    billow.octaves = BILLOW_OCTAVES;
    let sun = Sun::new(pt2(SUN_START_X, SUN_START_Y));
    let moon = Moon::new();
    Model {
        _window,
        points,
        billow,
        sun,
        sky_color: LIGHTSKYBLUE.into(),
        darkened_sky_color: LIGHTSKYBLUE.into(),
        stars: Stars::random_sky(),
        moon,
        speedup: false,
    }
}

fn update(app: &App, model: &mut Model, _update: Update) {
    let delta = (app.elapsed_frames() as f64)
        * SPEED_MULTIPLIER
        * if model.speedup { SPEEDUP_FACTOR } else { 1. };
    let temp_x = delta;

    let mut iter_x = Some((0..NUM_POINTS).into_par_iter());
    let iter_y = (0..NUM_POINTS).into_iter();
    model.sun.advance_sun_pos(
        app.elapsed_frames()
            * if model.speedup {
                SPEEDUP_FACTOR as u64
            } else {
                1
            },
    );
    model.points = iter_x
        .take()
        .unwrap()
        .map(|x| {
            let noisefn = Exponent::<[f64; 3]>::new(&model.billow);
            let spat_x = x as f64 / 550. - ((delta + 120.) * WIND_SPEED);
            iter_y
                .clone()
                .map(|y| {
                    let spat_y = (y as f64 / 550.) - Y_OFFSET;
                    let mut alpha = noisefn.get([spat_x, spat_y, temp_x]).abs();
                    if alpha < ZERO_ALPHA_THRESHOLD {
                        alpha = 0.;
                    } else {
                        alpha = map_range(
                            alpha,
                            ZERO_ALPHA_THRESHOLD,
                            1. * ALPHA_ZERO_SCALING,
                            0.0,
                            1.,
                        )
                    }

                    alpha
                })
                .collect::<Vec<_>>()
                .try_into()
                .unwrap()
        })
        .collect::<Vec<_>>()
        .try_into()
        .unwrap();

    let color = Sun::transition_sky_color(if let Some(amt) = model.sun.rising_amount() {
        1. - amt
    } else if let Some(amt) = model.sun.setting_amount() {
        amt
    } else if model.sun.has_set() {
        1.
    } else {
        0.
    });
    model.sky_color = color.into();

    if !model.sun.has_set() {
        let mut covered_points = 0.;
        for x in 0..model.points.len() {
            for y in 0..model.points[x].len() {
                if !model.points[x][y].is_zero() {
                    if collide_circle_point(
                        pt2(x as f32 * PIXELS_PER_POINT_F, y as f32 * PIXELS_PER_POINT_F),
                        model.sun.pos,
                        SUN_RADIUS as f32,
                    ) {
                        covered_points += model.points[x][y];
                    }
                }
            }
        }
        let factor = map_range(covered_points, 0., 120., 0., 0.4);
        model.darkened_sky_color = darken_by(model.sky_color, factor);
    } else {
        model.darkened_sky_color = NIGHT_SKY_COLOR.into();
    }
}

fn event(app: &App, model: &mut Model, event: WindowEvent) {
    match event {
        WindowEvent::KeyPressed(k) => match k {
            Key::Space => {
                println!("{}", app.fps());
            }
            Key::S => {
                println!("{:?}", model.stars.points);
                println!("{}", model.sun.has_set());
            }
            Key::Right => {
                model.speedup = true;
            }
            _ => {}
        },
        WindowEvent::KeyReleased(k) => match k {
            Key::Right => {
                model.speedup = false;
            }
            _ => {}
        },
        _ => {}
    }
}

fn view(app: &App, model: &Model, frame: Frame) {
    let draw = app.draw();
    let draw = draw.x_y(-(SCREEN_SIZE_F) / 2., -(SCREEN_SIZE_F) / 2.);
    frame.clear(model.darkened_sky_color);

    if !model.sun.has_set() {
        //draw sun
        draw.ellipse()
            .x_y(model.sun.pos.x, model.sun.pos.y)
            .color(WHITE)
            .radius(SUN_RADIUS as f32)
            .finish();
        for i in 0..SUN_AURA_SIZE {
            let alpha = map_range(i, 0, SUN_AURA_SIZE, 0.101, 1.).log10().abs();
            let color = with_alpha(GAINSBORO.into(), alpha);
            draw.ellipse()
                .no_fill()
                .stroke_weight(1.)
                .x_y(model.sun.pos.x, model.sun.pos.y)
                .stroke_color(color)
                .radius((SUN_RADIUS + i) as f32)
                .finish();
        }
    } else {
        //moon aura
        for i in 0..MOON_AURA_SIZE {
            let alpha = map_range(i, 0, MOON_AURA_SIZE, 0.7, 1.).log10().abs();
            let color = with_alpha(GAINSBORO.into(), alpha);
            draw.ellipse()
                .no_fill()
                .stroke_weight(1.)
                .x_y(MOON_POS.0, MOON_POS.1)
                .stroke_color(color)
                .radius((MOON_RADIUS + i) as f32)
                .finish();
        }
    }

    for star in model.stars.iter() {
        let star_alpha = if let Some(amt) = model.sun.rising_amount() {
            1. - amt
        } else if let Some(amt) = model.sun.setting_amount() {
            if amt > 0.85{
                let amt = map_range(amt, 0.85, 1., 0., 1.);
                amt
            } else {
                0.
            }
        } else if model.sun.has_set() {
            1.
        } else {
            0.
        };
        if star_alpha > 0. {
            draw.ellipse()
                .x_y(star.x, star.y)
                .color(white_with_alpha(star_alpha as f64))
                .radius(STAR_RADIUS)
                .finish();

            for i in 0..STAR_AURA_SIZE {
                let alpha = map_range(i, 0, STAR_AURA_SIZE, 0.8, 1.).log10().abs();
                let color = with_alpha(GAINSBORO.into(), alpha * star_alpha as f64);
                draw.ellipse()
                    .no_fill()
                    .stroke_weight(1.)
                    .x_y(star.x, star.y)
                    .stroke_color(color)
                    .radius(STAR_RADIUS + i as f32)
                    .finish();
            }
        }
    }

    //draw moon
    draw.ellipse()
        .x_y(MOON_POS.0, MOON_POS.1)
        .radius(MOON_RADIUS as f32)
        .color(if model.sun.has_set() {
            CORNSILK.into()
        } else {
            Rgb::new(215, 239, 253)
        })
        .finish();

    //moon spots
    for (point, alpha) in &model.moon.texture {
        let alpha = if !model.sun.has_set() {
            *alpha * 0.75
        } else {
            *alpha
        };
        draw.ellipse()
            .x_y(point.x, point.y)
            .color(with_alpha(
                if model.sun.has_set() {
                    MOON_SPOTS_COLOR.into()
                } else {
                    Rgb::new(143, 198, 232).into()
                },
                alpha,
            ))
            .radius(1.5)
            .finish()
    }

    //draw clouds
    for x in 0..model.points.len() {
        let row = model.points[x];
        for y in 0..row.len() {
            let alpha = row[y];
            draw.ellipse()
                .x_y(x as f32 * PIXELS_PER_POINT_F, y as f32 * PIXELS_PER_POINT_F)
                .color(if !model.sun.has_set() {
                    white_with_alpha(alpha)
                } else {
                    with_alpha(CLOUD_NIGHT_COLOR.into(), alpha)
                })
                .radius(PIXELS_PER_POINT_F * 3.)
                .finish();
        }
    }

    draw.to_frame(app, &frame).unwrap();
}
