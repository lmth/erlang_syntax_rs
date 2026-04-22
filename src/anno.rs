/// Source location annotation. Zero-size when `locations` feature is disabled.
#[cfg(feature = "locations")]
pub type Anno = (u32, u32);

#[cfg(not(feature = "locations"))]
pub type Anno = ();
