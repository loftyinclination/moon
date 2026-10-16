#![no_main]
#![no_std]

// Required for panic handler
extern crate alloc;
extern crate flipperzero_alloc;
extern crate flipperzero_rt;

use core::ffi::CStr;

use alloc::sync::Arc;
use flipperzero::furi::sync::Mutex;
use flipperzero::furi::thread::Timer;
use flipperzero::gui::Gui;
use flipperzero::gui::view_port::{ViewPort, ViewPortCallbacks};
use flipperzero::info;
use flipperzero_rt::{entry, manifest};
use flipperzero_sys::furi::UnsafeRecord;

use core::f64;
use siderust::affn::frames::{EclipticTrueOfDate, EquatorialTrueOfDate, Horizontal};
use siderust::affn::transform::Transform;
use siderust::affn::{Rotation3, SphericalPosition};
use siderust::bodies::{Moon, Sun};
use siderust::chrono::{DateTime, Utc};
use siderust::coordinates::centers::{Geodetic, Topocentric};
use siderust::ephemeris::Vsop87Ephemeris;
use siderust::event::horizontal::equatorial_to_horizontal;
use siderust::event::lunar::phase::moon_phase_topocentric;
use siderust::qtty::{AstronomicalUnit, DAY, DEG, M, Quantity, Turn};
use siderust::time::Interval;
use siderust::{AltitudeEventsExt, AltitudeProvider};
use siderust::{SearchOpts, twilight_classification};
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

struct Context {
    channel: Arc<Mutex<State>>,
}

enum State {
    SunsetSoon,
    SunriseSoon,
    MoonriseSoon,
    MoonUp,
    MoonsetSoon,
}

struct RightAscension(Quantity<siderust::qtty::Degree>);

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

struct Degrees(Quantity<siderust::qtty::Degree>);

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
        // TODO: get location from config?
        let bath = Geodetic::new(51.373983 * DEG, -2.392440 * DEG, 72.0 * M);

        let now = flipperzero::furi::hal::rtc::datetime();
        info!("Getting current sun and moon position at {:?} in Bath", now);

        let now: DateTime<Utc> = now.into();

        let start: siderust::time::Time<siderust::time::TT, siderust::MJD> = now.into();

        let moon_topocentric_equatorial_coordinates: SphericalPosition<
            Topocentric,
            EquatorialTrueOfDate,
            AstronomicalUnit,
        > = Moon::get_apparent_topocentric_equ(start.to::<siderust::time::JD>(), bath);

        let moon_alt_azimuth_coordinates: SphericalPosition<
            Topocentric,
            Horizontal,
            AstronomicalUnit,
        > = equatorial_to_horizontal(
            &moon_topocentric_equatorial_coordinates,
            bath,
            start.to::<siderust::time::JD>(),
        );

        let transform = Transform::<
            Topocentric,
            EquatorialTrueOfDate,
            Topocentric,
            EclipticTrueOfDate,
            Rotation3,
        >::new(Rotation3::IDENTITY);

        let moon_ecliptical_coordinates: SphericalPosition<_, EclipticTrueOfDate, _> = transform
            .apply(moon_topocentric_equatorial_coordinates.to_cartesian())
            .to_spherical();

        info!(
            "Moon position: Ra/Dec={}/{}, Az/Alt={}/{}, Ecliptic Lat/Long={}/{}",
            RightAscension(moon_topocentric_equatorial_coordinates.ra()),
            Degrees(moon_topocentric_equatorial_coordinates.dec()),
            Degrees(moon_alt_azimuth_coordinates.az()),
            Degrees(moon_alt_azimuth_coordinates.alt()),
            Degrees(moon_ecliptical_coordinates.lat()),
            Degrees(moon_ecliptical_coordinates.lon())
        );

        let sun_topocentric_equatorial_coordinates: SphericalPosition<
            Topocentric,
            EquatorialTrueOfDate,
            AstronomicalUnit,
        > = Sun::get_apparent_topocentric_equ(start.to::<siderust::time::JD>(), bath);

        let sun_alt_azimuth_coordinates = equatorial_to_horizontal(&sun_topocentric_equatorial_coordinates, bath, start.to::<siderust::time::JD>())

        let sun_ecliptical_coordinates = transform
            .apply(sun_topocentric_equatorial_coordinates.to_cartesian())
            .to_spherical();

        info!(
            "Sun position: Ra/Dec={}/{}, Az/Alt={}/{}, Ecliptic Lat/Long={}/{}",
            RightAscension(sun_topocentric_equatorial_coordinates.ra()),
            Degrees(sun_topocentric_equatorial_coordinates.dec()),
            Degrees(sun_alt_azimuth_coordinates.az()),
            Degrees(sun_alt_azimuth_coordinates.alt()),
            Degrees(sun_ecliptical_coordinates.lat()),
            Degrees(sun_ecliptical_coordinates.lon())
        );

        let twilight_classification = twilight_classification(sun_alt_azimuth_coordinates.alt());

        let moon_phase = moon_phase_topocentric::<Vsop87Ephemeris>(now.into(), bath);

        if moon_alt_azimuth_coordinates.alt() > 0.0 * DEG {
            // Moon is currently up

            todo!()
        } else {
            let moonrise = Moon
                .above_threshold(
                    &bath,
                    Interval {
                        start,
                        end: start + 1.0 * DAY,
                    },
                    0.0 * DEG,
                    SearchOpts::default(),
                )
                .remove(0);

            let moonrise_time = moonrise
                .start
                .try_to_chrono()
                .expect("Should be able to convert time");
            // Moon will rise at some point
            todo!()
        }
    }
}

struct MoonClockRunner {
    timer: Timer,
}

impl Default for MoonClockRunner {
    fn default() -> Self {
        let gui = Gui::open();
        let channel = Arc::new(Mutex::new(State::SunsetSoon));
        let view_port = ViewPort::new(Context {
            channel: channel.clone(),
        });
        let view_port = gui.add_view_port(view_port, flipperzero::gui::GuiLayer::StatusBarRight);

        let timer = Timer::new(move || {
            let channel = channel.clone();
            *channel.lock() = State::get_now();
            view_port.update();
        });
        Self { timer }
    }
}

fn main(_args: Option<&CStr>) -> i32 {
    let record: UnsafeRecord<MoonClockRunner> = unsafe { UnsafeRecord::get_or_default(c"moon") };
    0
}

impl ViewPortCallbacks for Context {
    fn on_draw(&mut self, canvas: flipperzero::gui::canvas::CanvasView<'_>) {
        match *self.channel.lock() {
            State::SunsetSoon => todo!(),
            State::SunriseSoon => todo!(),
            State::MoonriseSoon => todo!(),
            State::MoonUp => todo!(),
            State::MoonsetSoon => todo!(),
        }
    }
}
