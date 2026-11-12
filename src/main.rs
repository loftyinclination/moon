#![no_std]
#![no_main]

extern crate alloc;
extern crate libc;

use core::alloc::GlobalAlloc;
use siderust::bodies::Moon;
use siderust::chrono::{self, DateTime, TimeZone, Utc};
use siderust::coordinates::centers::{Geodetic, Topocentric};
use siderust::ephemeris::Vsop87Ephemeris;
use siderust::moon_phase_topocentric;
use siderust::qtty::{DEG, Degree, IlluminationFraction, Kilometer, M, Quantity, Radian};

struct MoonUp {
    illuminated_fraction: Quantity<IlluminationFraction>,
    bright_limb_position_angle: Quantity<Radian>,
    azimuth: Quantity<Degree>,
}

impl MoonUp {
    fn get_at_time(time: DateTime<Utc>) -> Self {
        let site = Geodetic::new(51.507222 * DEG, -0.1275 * DEG, M);

        let time: siderust::time::Time<siderust::time::TT, siderust::JD> = time.into();

        let moon_alt_azimuth_coordinates = Moon::get_horizontal::<Kilometer>(time, site);

        let siderust::MoonPhaseGeometry {
            illuminated_fraction,
            ..
        } = moon_phase_topocentric::<Vsop87Ephemeris>(time, site);

        let position_angle = Moon::bright_limb_position_angle::<Topocentric>((time, site))
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
