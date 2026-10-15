//! 10-sector golden atlas geometry — single source for phi, sector, x.

pub const SECTOR_COUNT: u8 = 10;
pub const DEFAULT_TICKS_PER_SECTOR: u64 = 10;

const TWO_PI: f64 = 2.0 * core::f64::consts::PI;
const PI: f64 = core::f64::consts::PI;

#[inline]
fn fcos(x: f64) -> f64 {
    #[cfg(feature = "std")]
    {
        x.cos()
    }
    #[cfg(all(not(feature = "std"), feature = "baremetal"))]
    {
        libm::cos(x)
    }
    #[cfg(all(not(feature = "std"), not(feature = "baremetal")))]
    {
        x.cos()
    }
}

pub fn default_delta_phi(ticks_per_sector: u64) -> f64 {
    let r = ticks_per_sector.max(1) as f64;
    PI / (5.0 * r)
}

pub fn wrap_phi(phi: f64) -> f64 {
    let mut p = phi;
    while p <= -PI {
        p += TWO_PI;
    }
    while p > PI {
        p -= TWO_PI;
    }
    p
}

pub fn master_x(phi: f64) -> f64 {
    2.0 * fcos(phi / 5.0)
}

pub fn sector_base_phi(sector_id: u8) -> f64 {
    let s = (sector_id % SECTOR_COUNT) as f64;
    -PI + s * (TWO_PI / SECTOR_COUNT as f64)
}

#[inline]
fn ffloor(x: f64) -> f64 {
    #[cfg(feature = "std")]
    {
        x.floor()
    }
    #[cfg(all(not(feature = "std"), feature = "baremetal"))]
    {
        libm::floor(x)
    }
    #[cfg(all(not(feature = "std"), not(feature = "baremetal")))]
    {
        x.floor()
    }
}

pub fn sector_from_phi(phi: f64) -> u8 {
    let wrapped = wrap_phi(phi);
    let shifted = wrapped + PI;
    let idx = ffloor(shifted / (TWO_PI / SECTOR_COUNT as f64)) as i32;
    idx.clamp(0, SECTOR_COUNT as i32 - 1) as u8
}

pub fn epsilon_from_phi(phi: f64, sector_id: u8) -> f64 {
    wrap_phi(phi) - sector_base_phi(sector_id)
}

pub fn update_from_phi(phi: f64) -> (u8, f64, f64) {
    let wrapped = wrap_phi(phi);
    let sector_id = sector_from_phi(wrapped);
    let epsilon = epsilon_from_phi(wrapped, sector_id);
    let x = master_x(wrapped);
    (sector_id, epsilon, x)
}
