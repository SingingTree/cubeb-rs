use cubeb_core as cubeb;

use stream::PulseStream;

use libc::c_void;
use libpulse;
use libpulse_sys;
use std::ffi::{CStr,CString};
use std::default::Default;
use util::*;

pub struct DefaultInfo {
    pub sample_spec: libpulse::SampleSpec,
    pub channel_map: libpulse::ChannelMap,
    pub flags: libpulse::SinkFlags,
}

#[derive(Default)]
pub struct PulseContext {
    pub context: libpulse::Context,
    pub mainloop: libpulse::ThreadedMainloop,
    pub default_sink_info: Option<DefaultInfo>,
    pub context_name: CString,
    pub error: bool
}

fn context_notify_callback(_: &libpulse::Context, user_data: *mut c_void) {
    let ctx = cast_mut::<PulseContext>(user_data);
    ctx.mainloop.signal(false);
}

impl PulseContext {
    fn wait_until_context_ready(&mut self) -> bool {
        loop {
            let state = self.context.get_state();
            if !state.is_good() {
                return false;
            }
            if state == libpulse_sys::PA_CONTEXT_READY {
                break;
            }
            self.mainloop.wait();
        }
        true
    }

    fn context_destroy(&mut self) {

        let vp: *mut c_void = self as *mut _ as *mut c_void;

        self.mainloop.lock();
        let r = self.context.drain(context_notify_callback, vp);
        if let Ok(o) = r {
            self.operation_wait(None, &o);
        };

        self.context.clear_state_callback();
        self.context.disconnect();
        self.mainloop.unlock();
    }

    fn context_init(&mut self) -> bool {
        fn context_state_callback(c: &libpulse::Context, user_data: *mut c_void) {
            let ctx = cast_mut::<PulseContext>(user_data);
            if !c.get_state().is_good() {
                ctx.error = true;
            }
            ctx.mainloop.signal(false);
        }

        if !self.context.is_null() {
            debug_assert!(self.error);
            self.context_destroy();
        }

        self.context = libpulse::Context::new(&self.mainloop.get_api(), &self.context_name);

        if self.context.is_null() {
            false
        } else {
            let self_void_ptr = cast_void_ptr(self);
            self.context.set_state_callback(context_state_callback, self_void_ptr);

            self.mainloop.lock();
            self.context.connect_simple();
            if !self.wait_until_context_ready() {
                self.mainloop.unlock();
                self.context_destroy();
                return false;
            }
            self.mainloop.unlock();

            self.error = false;

            true
        }
    }

    pub fn init(context_name: &str) -> cubeb::Result<Box<cubeb::Context>> {

        fn sink_info_callback(_context: &libpulse::Context,
                              info: &libpulse::SinkInfo,
                              eol: i32,
                              u: *mut c_void) {
            let ctx = cast_mut::<PulseContext>(u);
            if eol == 0 {
                ctx.default_sink_info = Some(DefaultInfo{
                    sample_spec: info.sample_spec,
                    channel_map: info.channel_map,
                    flags: info.flags
                });
            }
            ctx.mainloop.signal(false);
        }

        fn server_info_callback(context: &libpulse::Context,
                                info: &libpulse::ServerInfo,
                                u: *mut c_void) {
            context.get_sink_info_by_name(unsafe{ CStr::from_ptr(info.default_sink_name) },
                                          sink_info_callback, u);
        }

        let mut ctx: Box<PulseContext> = Default::default();

        ctx.context_name = CString::new(context_name).unwrap();
        ctx.mainloop = libpulse::ThreadedMainloop::new();
        ctx.mainloop.start();

        if !ctx.context_init() {
            return Err(cubeb::Error::Unclassified);
        }

        let ctx_void_ptr = cast_void_ptr(ctx.as_ref());
        ctx.mainloop.lock();
        ctx.context.get_server_info(server_info_callback, ctx_void_ptr);
        ctx.mainloop.unlock();

        Ok(ctx)
    }

    fn destroy(&mut self) {
        if !self.context.is_null() {
            self.context_destroy();
        }

        if !self.mainloop.is_null() {
            self.mainloop.stop();
            self.mainloop = Default::default();
        }
    }

    pub fn operation_wait<'a, S:Into<Option<&'a libpulse::Stream>>>(&self, stream: S, o: &libpulse::Operation) -> bool {
        let s = stream.into();
        while o.get_state() == libpulse_sys::PA_OPERATION_RUNNING {
                self.mainloop.wait();
                if !self.context.get_state().is_good() {
                    return false;
                }

            if let Some(stm) = s {
                if !stm.get_state().is_good() {
                    return false;
                }
            }
        }

        true
    }
}

impl ::std::ops::Drop for PulseContext {
    fn drop(&mut self) {
        self.destroy();
    }
}

impl cubeb::Context for PulseContext {
    fn get_backend_id(&self) -> &str {
        "pulse"
    }

    fn get_max_channel_count(&self) -> cubeb::Result<i32> {
        self.mainloop.lock();
        while self.default_sink_info.is_none() {
            self.mainloop.wait();
        }
        self.mainloop.unlock();

        match self.default_sink_info.as_ref() {
            Some(info) => Ok(info.channel_map.channels as i32),
            None => Err(cubeb::Error::Unclassified)
        }
    }

    fn get_min_latency(&self, params: cubeb::StreamParams) -> cubeb::Result<i32> {
        // According to PulseAudio developers, this is a safe minimum.
        Ok(25 * params.rate / 1000)
    }

    fn get_preferred_sample_rate(&self) -> cubeb::Result<u32> {
        self.mainloop.lock();
        while self.default_sink_info.is_none() {
            self.mainloop.wait();
        }
        self.mainloop.unlock();

        match self.default_sink_info.as_ref() {
            Some(info) => Ok(info.sample_spec.rate),
            None => Err(cubeb::Error::Unclassified)
        }
    }

    fn get_preferred_channel_layout(&self) -> cubeb::Result<cubeb::ChannelLayout> {
        Err(cubeb::Error::NotSupported)
    }

    fn stream_init<'a>(&'a mut self,
                       stream_name: &str,
                       input_device: cubeb::DeviceId,
                       input_stream_params: Option<cubeb::StreamParams>,
                       output_device: cubeb::DeviceId,
                       output_stream_params: Option<cubeb::StreamParams>,
                       latency_frames: u32,
                       data_callback: &'a Fn(*mut c_void, *const c_void, *mut c_void, usize) -> cubeb::Result<usize>,
                       state_callback: &'a Fn(*mut c_void, cubeb::State),
                       user_ptr: *mut c_void) -> cubeb::Result<Box<cubeb::Stream + 'a>>
    {
        // If the connection failed for some reason, try to reconnect
        if self.error && !self.context_init() {
            return Err(cubeb::Error::Unclassified);
        }

        PulseStream::init(self,
                          stream_name,
                          input_device,
                          input_stream_params,
                          output_device,
                          output_stream_params,
                          latency_frames,
                          data_callback,
                          state_callback,
                          user_ptr)
    }

    fn register_device_changed_callback(&mut self, cb: &Fn()) -> cubeb::Result<()> {
        Err(cubeb::Error::Unclassified)
    }
}
