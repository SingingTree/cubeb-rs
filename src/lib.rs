extern crate cubeb_core;

mod init;
mod validation;

/* Backends - configured via features in Cargo.toml */
#[cfg(feature = "use_alsa")]
mod backend_alsa;
#[cfg(feature = "use_audiotrack")]
mod backend_audiotrack;
#[cfg(feature = "use_audiounit")]
mod backend_audiounit;
#[cfg(feature = "use_jack")]
mod backend_jack;
#[cfg(feature = "use_kai")]
mod backend_kai;
#[cfg(feature = "use_opensl")]
mod backend_opensl;
#[cfg(feature = "use_pulse")]
extern crate backend_pulse;
#[cfg(feature = "use_sndio")]
mod backend_sndio;
#[cfg(feature = "use_wasapi")]
mod backend_wasapi;
#[cfg(feature = "use_winmm")]
mod backend_winmm;

pub use cubeb_core::*;
pub use init::init;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {}
}
