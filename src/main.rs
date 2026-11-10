#![no_std]
#![no_main]

extern crate alloc;
extern crate libc;

use core::alloc::GlobalAlloc;
use siderust::affn::frames::{EquatorialTrueOfDate, Horizontal};
use siderust::astro::eop::NullEop;
use siderust::astro::nutation::Iau2006;
use siderust::bodies::Moon;
use siderust::chrono::{self, DateTime, TimeZone, Utc};
use siderust::coordinates::centers::{Geodetic, Topocentric};
use siderust::coordinates::transform::context::DefaultEphemeris;
use siderust::event::horizontal::equatorial_to_horizontal_true_of_date_with_ctx;
use siderust::event::lunar::phase::moon_phase;
use siderust::qtty::{
    AstronomicalUnit, DEG, Degree, IlluminationFraction, Kilometer, M, Quantity, Radian,
};

struct MoonUp {
    illuminated_fraction: Quantity<IlluminationFraction>,
    bright_limb_position_angle: Quantity<Radian>,
    azimuth: Quantity<Degree>,
}

impl MoonUp {
    fn get_at_time(time: DateTime<Utc>) -> Self {
        let site = Geodetic::new(51.507222 * DEG, -0.1275 * DEG, M);

        let time: siderust::time::Time<siderust::time::TT, siderust::JD> = time.into();
        use siderust::affn::SphericalPosition;
        use siderust::coordinates::transform::AstroContext;

        let ctx: AstroContext<DefaultEphemeris, NullEop> = AstroContext::with_types();
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
                site,
                time,
                &ctx,
            )
        };

        let moon_alt_azimuth_coordinates: SphericalPosition<Topocentric, Horizontal, Kilometer> =
            equatorial_to_horizontal_true_of_date_with_ctx(
                &moon_topocentric_equatorial_coordinates,
                site,
                time,
                &ctx,
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
                site,
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

        MoonUp {
            illuminated_fraction,
            bright_limb_position_angle: position_angle,
            azimuth: moon_alt_azimuth_coordinates.alt(),
        }
    }
}

#[panic_handler]
fn panic_handler(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}

#[unsafe(no_mangle)]
pub extern "C" fn main(_argc: isize, _argv: *const *const u8) -> isize {
    let now = chrono::Utc
        .with_ymd_and_hms(2026, 11, 10, 20, 10, 0)
        .unwrap();

    let moon_up = MoonUp::get_at_time(now);
    unsafe {
        libc::printf(
            c"Moon is %s the horizon (%.2f°), angle of %.2f° and %.1f%% illuminated\n".as_ptr(),
            if moon_up.azimuth.value_ref().is_sign_positive() {
                c"above"
            } else {
                c"below"
            },
            moon_up.azimuth.value(),
            moon_up
                .bright_limb_position_angle
                .to_const::<Degree>()
                .value(),
            moon_up.illuminated_fraction.value() * 100.0,
        )
    };

    0
}

pub struct LibcAlloc;

#[global_allocator]
static ALLOCATOR: LibcAlloc = LibcAlloc;

unsafe impl GlobalAlloc for LibcAlloc {
    #[inline]
    unsafe fn alloc(&self, layout: core::alloc::Layout) -> *mut u8 {
        let align = layout.align().max(core::mem::size_of::<usize>());
        let size = layout.size();

        (unsafe { libc::memalign(align, size) }) as *mut u8
    }

    #[inline]
    unsafe fn dealloc(&self, ptr: *mut u8, _layout: core::alloc::Layout) {
        unsafe { libc::free(ptr as *mut core::ffi::c_void) };
    }
}
