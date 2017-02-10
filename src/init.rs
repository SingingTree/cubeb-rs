use {Context, Error, Result};

#[cfg(feature = "use_pulse")]
use backend_pulse::init as pulse_init;

#[cfg(feature = "use_jack")]
use backend_jack::JackContext;

#[cfg(feature = "use_jack")]
fn jack_init(context_name: &str) -> Result<Box<Context>> {
    Ok(Box::new(JackContext::new(context_name)))
}

pub fn init(context_name: &str) -> Result<Box<Context>> {
    let mut init: Vec<fn(&str) -> Result<Box<Context>>> = Vec::new();
#[cfg(feature = "use_pulse")]
    init.push(pulse_init);
#[cfg(feature = "use_jack")]
    init.push(jack_init);

    for i in init {
        let ctx = i(context_name);
        if ctx.is_ok() {
            return ctx;
        }
    }

    Err(Error::Unclassified)
}
