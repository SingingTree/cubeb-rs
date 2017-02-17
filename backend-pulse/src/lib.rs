extern crate cubeb_core;
extern crate libc;
extern crate libpulse_sys;
extern crate libpulse;
extern crate semver;

mod context;
mod stream;
mod util;

use cubeb_core as cubeb;
use context::PulseContext;

pub fn init(context_name: &str) -> cubeb::Result<Box<cubeb::Context>>
{
     PulseContext::init(context_name)
}
