#![no_main]
#![no_std]

// Required for panic handler
extern crate alloc;
extern crate flipperzero_alloc;
extern crate flipperzero_rt;

use core::ffi::CStr;

use alloc::sync::Arc;
use flipperzero::furi::sync::Mutex;
use flipperzero::furi::thread::{Timer, sleep};
use flipperzero::gui::Gui;
use flipperzero::gui::view_port::{ViewPort, ViewPortCallbacks};
use flipperzero::info;
use flipperzero_rt::{entry, manifest};
use flipperzero_sys::furi::UnsafeRecord;

use core::f64;
use core::ops::AddAssign;
use core::time::Duration;
use flipperzero::furi::time::FuriDuration;
use siderust::affn::frames::{EquatorialTrueOfDate, Horizontal};
use siderust::astro::nutation::Iau2006;
use siderust::bodies::Moon;
use siderust::chrono::{self, DateTime, Utc};
use siderust::coordinates::centers::{Geodetic, Topocentric};
use siderust::event::horizontal::equatorial_to_horizontal_true_of_date_with_ctx;
use siderust::event::lunar::phase::moon_phase;
use siderust::qtty::{
    AstronomicalUnit, DEG, Degree, IlluminationFraction, Kilometer, M, Quantity, Radian, Turn,
};
use ufmt::{uDisplay, uwrite};

// Define the FAP Manifest for this application
manifest!(
    name = "Moon",
    app_version = 1,
    has_icon = true,
    // See https://github.com/flipperzero-rs/flipperzero/blob/v0.11.0/docs/icons.md for icon format
    icon = "rustacean-10x10.icon",
);

// Define the entry function
entry!(main);

const HEIGHT: i32 = 8;

struct Context {
    channel: Arc<Mutex<State>>,
}

enum State {
    MoonUp {
        illuminated_fraction: Quantity<IlluminationFraction>,
        // Measured North through East, the point of greatest illumination
        bright_limb_position_angle: Quantity<Radian>,
    },
}

impl Default for State {
    fn default() -> Self {
        State::MoonUp {
            illuminated_fraction: Quantity::one(),
            bright_limb_position_angle: Quantity::zero(),
        }
    }
}

struct RightAscension(Quantity<Degree>);

impl uDisplay for RightAscension {
    fn fmt<W>(&self, f: &mut ufmt::Formatter<'_, W>) -> Result<(), W::Error>
    where
        W: ufmt::uWrite + ?Sized,
    {
        let this = self.0.to::<Turn>().value();
        let hours = this * 24.0;
        let minutes = (hours * 60.0) % 24.0;
        let seconds = (minutes * 60.0) % 60.0;
        let hours = unsafe { f64::to_int_unchecked::<u8>(hours) };
        let minutes = unsafe { f64::to_int_unchecked::<u8>(minutes) };
        let seconds = unsafe { f64::to_int_unchecked::<u8>(seconds) };

        uwrite!(f, "{}h{}m{}s", hours, minutes, seconds)
    }
}

struct Degrees(Quantity<Degree>);

impl uDisplay for Degrees {
    fn fmt<W>(&self, f: &mut ufmt::Formatter<'_, W>) -> Result<(), W::Error>
    where
        W: ufmt::uWrite + ?Sized,
    {
        let degrees = self.0.value();
        let arcminutes = degrees % 60.0;
        let arcseconds = arcminutes % 60.0;
        let degrees = unsafe { f64::to_int_unchecked::<u8>(degrees) };
        let arcminutes = unsafe { f64::to_int_unchecked::<u8>(arcminutes) };
        let arcseconds = unsafe { f64::to_int_unchecked::<u8>(arcseconds) };

        uwrite!(f, "{}°{}′{}″", degrees, arcminutes, arcseconds)
    }
}

impl State {
    fn get_now() -> Self {
        let time = flipperzero::furi::hal::rtc::datetime();

        info!(
            "Getting current sun and moon position at {:?} in Bath",
            time
        );

        let time: DateTime<Utc> = time.into();

        Self::get_at_time(time)
    }

    fn get_at_time(time: DateTime<Utc>) -> Self {
        // TODO: get location from config?
        let bath = Geodetic::new(51.373983 * DEG, -2.392440 * DEG, 72.0 * M);

        let time: siderust::time::Time<siderust::time::TT, siderust::JD> = time.into();
        use siderust::affn::SphericalPosition;
        use siderust::coordinates::transform::AstroContext;

        let ctx = AstroContext::new();
        let ctx = ctx.with_model::<Iau2006>();

        let moon_topocentric_equatorial_coordinates: SphericalPosition<
            Topocentric,
            EquatorialTrueOfDate,
            Kilometer,
        > = {
            use siderust::coordinates::transform::TransformFrame;

            let moon_geo_ecliptic = Moon::get_geo_position::<Kilometer>(time);
            let moon_geo_eq_j2000 = TransformFrame::to_frame(&moon_geo_ecliptic);

            siderust::event::horizontal::geocentric_j2000_to_apparent_topocentric_with_ctx(
                &moon_geo_eq_j2000,
                bath,
                time,
                &ctx,
            )
        };

        let moon_alt_azimuth_coordinates: SphericalPosition<Topocentric, Horizontal, Kilometer> =
            equatorial_to_horizontal_true_of_date_with_ctx(
                &moon_topocentric_equatorial_coordinates,
                bath,
                time,
                &ctx,
            );

        info!(
            "Moon position: Ra/Dec={}/{}, Az/Alt={}/{}",
            RightAscension(moon_topocentric_equatorial_coordinates.ra()),
            Degrees(moon_topocentric_equatorial_coordinates.dec()),
            Degrees(moon_alt_azimuth_coordinates.az()),
            Degrees(moon_alt_azimuth_coordinates.alt())
        );

        let sun_topo = {
            use siderust::coordinates::cartesian::position::{
                EclipticMeanJ2000, EquatorialMeanJ2000,
            };
            use siderust::coordinates::centers::{Geocentric, Heliocentric};
            use siderust::coordinates::transform::PositionAstroExt as _;

            let helio = EclipticMeanJ2000::<AstronomicalUnit, Heliocentric>::CENTER;

            let geo_ecl_j2000: EclipticMeanJ2000<AstronomicalUnit, Geocentric> =
                helio.to_with(&time, &ctx);

            let geo_cart_j2000: EquatorialMeanJ2000<AstronomicalUnit, Geocentric> =
                geo_ecl_j2000.to_frame_with(&time, &ctx);

            siderust::event::horizontal::geocentric_j2000_to_apparent_topocentric_with_ctx(
                &geo_cart_j2000,
                bath,
                time,
                &ctx,
            )
        };

        let siderust::MoonPhaseGeometry {
            illuminated_fraction,
            ..
        } = { moon_phase(moon_topocentric_equatorial_coordinates, sun_topo) };

        let position_angle = moon_topocentric_equatorial_coordinates
            .direction()
            .position_angle_to(&sun_topo.direction())
            .unwrap_or(Quantity::zero());

        Self::MoonUp {
            illuminated_fraction,
            bright_limb_position_angle: position_angle,
        }
    }
}

struct MoonClockRunner {
    timer: Timer,
}

impl Default for MoonClockRunner {
    fn default() -> Self {
        let gui = Gui::open();
        let channel: Arc<Mutex<State>> = Default::default();
        let view_port = ViewPort::new(Context {
            channel: channel.clone(),
        });
        let view_port = gui.add_view_port(view_port, flipperzero::gui::GuiLayer::Fullscreen);
        //let view_port = gui.add_view_port(view_port, flipperzero::gui::GuiLayer::StatusBarRight);

        let time: Arc<Mutex<DateTime<Utc>>> =
            Arc::new(Mutex::new(flipperzero::furi::hal::rtc::datetime().into()));

        let timer_channel = channel.clone();
        let timer = Timer::new(move || {
            let mut time = time.lock();
            *timer_channel.lock() = State::get_at_time(*time);
            view_port.update();
            (*time).add_assign(chrono::TimeDelta::hours(6));
        });
        Self { timer }
    }
}

fn main(_args: Option<&CStr>) -> i32 {
    //let record: UnsafeRecord<MoonClockRunner> = unsafe { UnsafeRecord::get_or_default(c"moon") };
    let mut clock = MoonClockRunner::default();
    clock
        .timer
        .start(FuriDuration::from_millis(200))
        .expect("Failed to start timer");

    sleep(Duration::from_secs(10));

    clock.timer.stop().expect("Failed to stop timer");
    0
}

impl ViewPortCallbacks for Context {
    fn on_draw(&mut self, mut canvas: flipperzero::gui::canvas::CanvasView<'_>) {
        let diameter = core::cmp::min(canvas.height(), canvas.width()).get();
        let radius = diameter / 2;
        let center = (diameter / 2) as i32;

        match *self.channel.lock() {
            State::MoonUp {
                illuminated_fraction,
                bright_limb_position_angle,
            } => {
                use embedded_graphics::geometry::{Angle, Point};
                use embedded_graphics::pixelcolor::BinaryColor;
                use embedded_graphics::primitives::{
                    Ellipse, PrimitiveStyle, Sector, StyledDrawable,
                };

                // Pacman with a pointy mouth
                Sector::new(
                    Point::zero(),
                    diameter as u32,
                    Angle::from_degrees(
                        (bright_limb_position_angle - Quantity::QUARTER_TURN)
                            .normalize()
                            .value() as _,
                    ),
                    Angle::from_degrees(180.0),
                )
                .draw_styled(&PrimitiveStyle::with_fill(BinaryColor::On), &mut canvas);

                Ellipse::new_with_center_and_rotation(
                    Point::new_equal(center),
                    radius as _,
                    ((radius as f64) * illuminated_fraction.value()) as _,
                    Angle::from_degrees(bright_limb_position_angle.value() as _),
                )
                .draw_styled(
                    &PrimitiveStyle::with_fill(if illuminated_fraction.value() > 0.5 {
                        BinaryColor::On
                    } else {
                        BinaryColor::Off
                    }),
                    &mut canvas,
                );
            }
        }
    }
}
