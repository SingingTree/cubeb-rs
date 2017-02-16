use cubeb_core as cubeb;

use stream::PulseStream;

use libc::{c_char,c_void};
use libpulse;
use libpulse_sys;
use std::ffi::{CStr,CString};
use std::default::Default;
use util::*;

const PA_RATE_MAX: i32 = 48000 * 8;

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

// For device enumeration
struct PulseDevListData<'a> {
    default_sink_name: CString,
    default_source_name: CString,
    devinfo: Vec<cubeb::DeviceInfo>,
    context: &'a PulseContext
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

        struct ServerInfoCBData<'a> {
            ctx: &'a PulseContext,
            default_sink_name: CString
        };

        fn sink_info_cb(_: &libpulse::Context,
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

        fn server_info_cb(_: &libpulse::Context,
                          info: &libpulse::ServerInfo,
                          u: *mut c_void) {
            let mut cd_data = cast_mut::<ServerInfoCBData>(u);
            cd_data.default_sink_name = unsafe { CStr::from_ptr(info.default_sink_name) }.to_owned();
            cd_data.ctx.mainloop.signal(false);
        }

        let mut ctx: Box<PulseContext> = Default::default();

        ctx.context_name = CString::new(context_name).unwrap();
        ctx.mainloop = libpulse::ThreadedMainloop::new();
        ctx.mainloop.start();

        if !ctx.context_init() {
            return Err(cubeb::Error::Unclassified);
        }

        {
            let mut server_info_data = ServerInfoCBData {
                ctx: &ctx,
                default_sink_name: Default::default()
            };

            ctx.mainloop.lock();

            if let Ok(o) = ctx.context.get_server_info(server_info_cb,
                                                       cast_void_ptr(&mut server_info_data)) {
                ctx.operation_wait(None, &o);
            }

            if let Ok(o) = ctx.context.get_sink_info_by_name(&server_info_data.default_sink_name,
                                                             sink_info_cb,
                                                             cast_void_ptr(ctx.as_ref())) {
                ctx.operation_wait(None, &o);
            }

            ctx.mainloop.unlock();
        }

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

    fn enumerate_devices(&self, devtype: cubeb::DeviceType) -> cubeb::Result<Vec<cubeb::DeviceInfo>> {

        fn server_info_cb(_: &libpulse::Context,
                          info: &libpulse::ServerInfo,
                          user_data: *mut c_void)
        {
            let list_data = cast_mut::<PulseDevListData>(user_data);

            list_data.default_sink_name = unsafe { CStr::from_ptr(info.default_sink_name) }.to_owned();
            list_data.default_source_name = unsafe { CStr::from_ptr(info.default_source_name) }.to_owned();

            list_data.context.mainloop.signal(false);
        }

        macro_rules! extract_devinfo {
            ($info: ident, $default_name: expr)  =>
            {{
                let mut devinfo: cubeb::DeviceInfo = Default::default();
                devinfo.device_id = unsafe { CStr::from_ptr($info.name) }.to_owned();
                devinfo.devid = devinfo.device_id.as_ptr() as _;
                devinfo.friendly_name = unsafe { CStr::from_ptr($info.description) }.to_owned();
                unsafe {
                    let prop = libpulse_sys::pa_proplist_gets($info.proplist, b"sysfs.path\0".as_ptr() as *const c_char);
                    if !prop.is_null() {
                        devinfo.group_id = Some(CStr::from_ptr(prop).to_owned());
                    }
                    let prop = libpulse_sys::pa_proplist_gets($info.proplist, b"device.vendor.name\0".as_ptr() as *const c_char);
                    if !prop.is_null() {
                        devinfo.vendor_name = Some(CStr::from_ptr(prop).to_owned());
                    }
                }

                devinfo.devtype = cubeb::DeviceType::Output;
                devinfo.state = {
                    if $info.active_port.is_null() {
                        cubeb::DeviceState::Disabled
                    } else {
                        let port = unsafe { *$info.active_port };
                        if cfg!(feature="pa_version_2") && port.available == libpulse_sys::PA_PORT_AVAILABLE_NO {
                            cubeb::DeviceState::Unplugged
                        } else {
                            cubeb::DeviceState::Enabled
                        }
                    }
                };

                devinfo.preferred = if devinfo.device_id == $default_name {
                    cubeb::DEVICE_PREF_ALL
                } else {
                    cubeb::DEVICE_PREF_NONE
                };

                devinfo.format = cubeb::DEVICE_FMT_ALL;
                devinfo.default_format = to_cubeb_format($info.sample_spec.format);
                devinfo.max_channels = $info.channel_map.channels as i32;
                devinfo.min_rate = 1;
                devinfo.max_rate = PA_RATE_MAX;
                devinfo.default_rate = $info.sample_spec.rate as i32;

                devinfo.latency_lo = 0;
                devinfo.latency_hi = 0;

                devinfo
            }}
        }

        macro_rules! info_cb {
            ($name: ident, $info_ty: ty, $devtype: expr, $default_name: ident) => {
                fn $name(_: &libpulse::Context, info: &$info_ty, eol: i32, user_data: *mut c_void){
                    if eol != 0 {
                        return;
                    }

                    debug_assert!(!user_data.is_null());

                    let mut list_data = cast_mut::<PulseDevListData>(user_data);

                    let mut devinfo: cubeb::DeviceInfo = Default::default();
                    devinfo.device_id = unsafe { CStr::from_ptr(info.name) }.to_owned();
                    devinfo.devid = devinfo.device_id.as_ptr() as _;
                    devinfo.friendly_name = unsafe { CStr::from_ptr(info.description) }.to_owned();
                    unsafe {
                        let prop = libpulse_sys::pa_proplist_gets(info.proplist, b"sysfs.path\0".as_ptr() as *const c_char);
                        if !prop.is_null() {
                            devinfo.group_id = Some(CStr::from_ptr(prop).to_owned());
                        }
                        let prop = libpulse_sys::pa_proplist_gets(info.proplist, b"device.vendor.name\0".as_ptr() as *const c_char);
                        if !prop.is_null() {
                            devinfo.vendor_name = Some(CStr::from_ptr(prop).to_owned());
                        }
                    }

                    devinfo.devtype = $devtype;
                    devinfo.state = {
                        if info.active_port.is_null() {
                            cubeb::DeviceState::Disabled
                        } else {
                            let port = unsafe { *info.active_port };
                            if cfg!(feature="pa_version_2") && port.available == libpulse_sys::PA_PORT_AVAILABLE_NO {
                                cubeb::DeviceState::Unplugged
                            } else {
                                cubeb::DeviceState::Enabled
                            }
                        }
                    };

                    devinfo.preferred = if devinfo.device_id == list_data.$default_name {
                        cubeb::DEVICE_PREF_ALL
                    } else {
                        cubeb::DEVICE_PREF_NONE
                    };

                    devinfo.format = cubeb::DEVICE_FMT_ALL;
                    devinfo.default_format = to_cubeb_format(info.sample_spec.format);
                    devinfo.max_channels = info.channel_map.channels as i32;
                    devinfo.min_rate = 1;
                    devinfo.max_rate = PA_RATE_MAX;
                    devinfo.default_rate = info.sample_spec.rate as i32;

                    devinfo.latency_lo = 0;
                    devinfo.latency_hi = 0;

                    list_data.devinfo.push(devinfo);
                    list_data.context.mainloop.signal(false);
                }
            }
        }

        info_cb!(sink_info_cb, libpulse::SinkInfo, cubeb::DEVICE_TYPE_OUTPUT, default_sink_name);
        info_cb!(source_info_cb, libpulse::SourceInfo, cubeb::DEVICE_TYPE_INPUT, default_source_name);

        let mut device_data = PulseDevListData::new(self);

        {
            self.mainloop.lock();

            let ud = &device_data as *const _ as *mut c_void;
            if let Ok(o) = self.context.get_server_info(server_info_cb, ud) {
                self.operation_wait(None, &o);
            }

            if (devtype & cubeb::DEVICE_TYPE_OUTPUT) != 0 {
                if let Ok(o) = self.context.get_sink_info_list(sink_info_cb, ud) {
                    self.operation_wait(None, &o);
                }
            }

            if (devtype & cubeb::DEVICE_TYPE_INPUT) != 0 {
                if let Ok(o) = self.context.get_source_info_list(source_info_cb, ud) {
                    self.operation_wait(None, &o);
                }
            }

            self.mainloop.unlock();
        }

        Ok(device_data.devinfo)
    }

    fn register_device_changed_callback(&mut self, cb: &Fn()) -> cubeb::Result<()> {
        Err(cubeb::Error::Unclassified)
    }
}

impl<'ctx> PulseDevListData<'ctx> {
    pub fn new(ctx: &'ctx PulseContext) -> Self {
        PulseDevListData {
            default_sink_name: Default::default(),
            default_source_name: Default::default(),
            devinfo: Default::default(),
            context: ctx
        }
    }
}

fn to_cubeb_format(format: libpulse_sys::pa_sample_format_t) -> cubeb::DeviceFormat
{
  match format {
    libpulse_sys::PA_SAMPLE_S16LE => cubeb::DEVICE_FMT_S16LE,
    libpulse_sys::PA_SAMPLE_S16BE => cubeb::DEVICE_FMT_S16BE,
    libpulse_sys::PA_SAMPLE_FLOAT32LE => cubeb::DEVICE_FMT_F32LE,
    libpulse_sys::PA_SAMPLE_FLOAT32BE => cubeb::DEVICE_FMT_F32BE,
    _ => { panic!("Invalid format"); }
  }
}
