use cubeb_core as cubeb;

use context::PulseContext;
use libc::size_t;
use libc::{c_char, c_int, c_uint, c_void};
use libpulse;
use libpulse_sys::*;
use std::ffi::{CStr,CString};
use std::ptr;
use std::slice;
use util::*;

const PULSE_NO_GAIN: f32 = -1.0;
const PA_USEC_PER_MSEC: pa_usec_t = 1000;
const PA_USEC_PER_SEC: pa_usec_t = 1000000;
const PA_RATE_MAX: c_uint = 48000 * 8;

type CorkState = i32;
const UNCORK: CorkState = 0x0;
const CORK: CorkState = 0x1;
const NOTIFY: CorkState = 0x2;

type StreamType = i32;
const OUTPUT: StreamType = 0;
const INPUT: StreamType = 1;

fn to_pulse_format(format: cubeb::SampleFormat) -> pa_sample_format_t {
    match format {
        cubeb::SAMPLE_S16LE => PA_SAMPLE_S16LE,
        cubeb::SAMPLE_S16BE => PA_SAMPLE_S16BE,
        cubeb::SAMPLE_FLOAT32LE => PA_SAMPLE_FLOAT32LE,
        cubeb::SAMPLE_FLOAT32BE => PA_SAMPLE_FLOAT32BE,
        _ => PA_SAMPLE_INVALID,
    }
}

fn create_pa_stream(ctx: &libpulse::Context,
                    stream_params: cubeb::StreamParams,
                    stream_name: &str)
                    -> cubeb::Result<libpulse::Stream> {
    let fmt = to_pulse_format(stream_params.format);
    if fmt == PA_SAMPLE_INVALID {
        return Err(cubeb::Error::InvalidFormat);
    }

    let ss = pa_sample_spec {
        channels: stream_params.channels as u8,
        format: fmt,
        rate: stream_params.rate as u32,
    };

    let name = CString::new(stream_name).unwrap();
    let s = libpulse::Stream::new(ctx, &name, &ss, None);
    if s.is_null() {
        Err(cubeb::Error::Unclassified)
    } else {
        Ok(s)
    }
}

fn set_buffering_attribute(latency_frames: c_uint,
                           sample_spec: &libpulse::SampleSpec) -> libpulse::BufferAttr {
    let tlength = latency_frames * libpulse::frame_size(sample_spec) as u32;
    let minreq = tlength / 4;
    let battr = pa_buffer_attr {
        maxlength: u32::max_value(),
        prebuf: u32::max_value(),
        tlength: tlength,
        minreq: minreq,
        fragsize: minreq,
    };

    // LOG("Requested buffer attributes maxlength %u, tlength %u, "
    // "prebuf %u, minreq %u, fragsize %u",
    //   battr.maxlength, battr.tlength, battr.prebuf, battr.minreq, battr.fragsize);

    battr
}

// Pulse audio callbacks

/// Generic operation success callback
fn success_callback(_: &libpulse::Stream, success: i32, u: *mut c_void) {
    let stm = cast::<PulseStream>(u);
    debug_assert!(success != 0);
    stm.context.mainloop.signal(false);
}


pub struct PulseStream<'a> {
    context: &'a PulseContext,
    //streams: [libpulse::Stream; 2],
    output_stream: libpulse::Stream,
    input_stream: libpulse::Stream,
    data_callback: Box<FnMut(*mut c_void, *const c_void, *mut c_void, usize) -> cubeb::Result<usize> + 'a>,
    state_callback: Box<FnMut(*mut c_void, cubeb::State) + 'a>,
    user_ptr: *mut c_void,
    drain_timer: libpulse::TimeEvent,
    output_sample_spec: libpulse::SampleSpec,
    input_sample_spec: libpulse::SampleSpec,
    shutdown: bool,
    volume: f32,
    state: cubeb::State,
}

impl<'a> PulseStream<'a> {
    fn get_stream(&self, st: StreamType) -> &libpulse::Stream {
        if st == INPUT {
            &self.input_stream
        } else {
            &self.output_stream
        }
    }

    pub fn init(context: &'a mut PulseContext,
                stream_name: &str,
                input_device: cubeb::DeviceId,
                input_stream_params: Option<cubeb::StreamParams>,
                output_device: cubeb::DeviceId,
                output_stream_params: Option<cubeb::StreamParams>,
                latency_frames: u32,
                data_callback: &'a Fn(*mut c_void, *const c_void, *mut c_void, usize) -> cubeb::Result<usize>,
                state_callback: &'a Fn(*mut c_void, cubeb::State),
                user_ptr: *mut c_void)
                -> cubeb::Result<Box<cubeb::Stream + 'a>> {

        fn stream_state_callback(s: &libpulse::Stream, u: *mut c_void) {
            let mut stm = cast_mut::<PulseStream>(u);
            if !s.get_state().is_good() {
                stm.invoke_state_change_callback(cubeb::State::Error);
            }
            stm.context.mainloop.signal(false);
        }

        fn stream_write_callback(_s: &libpulse::Stream,
                                 nbytes: usize,
                                 u: *mut c_void) {
            //  LOGV("Output callback to be written buffer size %zd", nbytes);
            let mut stm = cast_mut::<PulseStream>(u);
            if stm.shutdown || stm.state != cubeb::State::Started {
                return;
            }

            if stm.input_stream.is_null() {
                // Output/playback only operation.
                // Write directly to output
                debug_assert!(!stm.output_stream.is_null());
                stm.trigger_user_callback(OUTPUT, ptr::null(), nbytes);
            }
        }

        fn read_from_input(s: &libpulse::Stream) -> libpulse::Result<(*const c_void, usize)> {
            try!(s.readable_size());
            s.peek()
        }

        fn stream_read_callback(s: &libpulse::Stream,
                                _nbytes: usize,
                                u: *mut c_void) {
            //  LOGV("Input callback buffer size %zd", nbytes);
            let stm = cast_mut::<PulseStream>(u);
            if stm.shutdown {
                return;
            }

//            let mut read_data: *const c_void = ptr::null();
//            let mut read_size: usize = 0;

            while let Ok((read_data, read_size)) = read_from_input(s) {
                /* read_data can be NULL in case of a hole. */
                if !read_data.is_null() {
                    let in_frame_size = libpulse::frame_size(&stm.input_sample_spec);
                    let read_frames = read_size / in_frame_size;

                    if !stm.output_stream.is_null() {
                        // input/capture + output/playback operation
                        let out_frame_size = libpulse::frame_size(&stm.output_sample_spec);
                        let write_size = read_frames * out_frame_size;
                        // Offer full duplex data for writing
                        stm.trigger_user_callback(OUTPUT, read_data, write_size);
                    } else {
                        // input/capture only operation. Call callback directly
                        /* TODO: this needs cleaning up. */
                        let got = match (stm.data_callback)(stm.user_ptr,
                                                            read_data,
                                                            ptr::null_mut(),
                                                            read_frames) {
                            Ok(x) => x,
                            Err(_) => 0
                        };

                        if got != read_frames {
                            s.cancel_write();
                            stm.shutdown = true;
                            break;
                        }
                    }
                }

                if read_size > 0 {
                    s.drop_record();
                }

                if stm.shutdown {
                    return;
                }
            }
        }

        let mut stm = Box::new(PulseStream {
            context: context,
            //streams: [Default::default(), Default::default()],
            output_stream: Default::default(),
            input_stream: Default::default(),
            data_callback: Box::new(data_callback),
            state_callback: Box::new(state_callback),
            user_ptr: user_ptr,
            drain_timer: Default::default(),
            output_sample_spec: Default::default(),
            input_sample_spec: Default::default(),
            shutdown: false,
            volume: PULSE_NO_GAIN,
            state: cubeb::State::Uninitialized,
        });

        let mut r = true;
        stm.context.mainloop.lock();

        // Set up output stream
        if let Some(osp) = output_stream_params {
            match create_pa_stream(&stm.context.context, osp, stream_name) {
                Err(e) => {
                    stm.context.mainloop.unlock();
                    stm.destroy();
                    return Err(e);
                },
                Ok(s) => {
                    stm.output_sample_spec = *s.get_sample_spec();

                    s.set_state_callback(stream_state_callback,
                                         stm.as_mut() as *mut _ as *mut c_void);
                    s.set_write_callback(stream_write_callback,
                                         stm.as_mut() as *mut _ as *mut c_void);

                    let battr = set_buffering_attribute(latency_frames, &stm.output_sample_spec);
                    let device_name = if output_device.is_null() {
                        None
                    } else {
                        unsafe { Some(CStr::from_ptr(output_device as *const c_char)) }
                    };
                    s.connect_playback(device_name,
                                       &battr,
                                       PA_STREAM_AUTO_TIMING_UPDATE |
                                       PA_STREAM_INTERPOLATE_TIMING |
                                       PA_STREAM_START_CORKED |
                                       PA_STREAM_ADJUST_LATENCY,
                                       None,
                                       None);
                    stm.output_stream = s;
                }
            }
        }

        // Set up input stream
        if let Some(isp) = input_stream_params {
            match create_pa_stream(&stm.context.context, isp, stream_name) {
                Err(e) => {
                    stm.context.mainloop.unlock();
                    stm.destroy();
                    return Err(e);
                },
                Ok(s) => {
                    stm.input_sample_spec = *s.get_sample_spec();

                    s.set_state_callback(stream_state_callback,
                                         stm.as_mut() as *mut _ as *mut c_void);
                    s.set_read_callback(stream_read_callback,
                                        stm.as_mut() as *mut _ as *mut c_void);

                    let battr = set_buffering_attribute(latency_frames, &stm.input_sample_spec);
                    let device_name = if input_device.is_null() {
                        None
                    } else {
                        unsafe { Some(CStr::from_ptr(output_device as *const c_char)) }
                    };

                    s.connect_record(device_name,
                                     &battr,
                                     PA_STREAM_AUTO_TIMING_UPDATE |
                                     PA_STREAM_INTERPOLATE_TIMING |
                                     PA_STREAM_START_CORKED |
                                     PA_STREAM_ADJUST_LATENCY);

                    stm.input_stream = s;
                }
            }
        }

        if stm.wait_until_ready() {
            /* force a timing update now, otherwise timing info does not become valid
            until some point after initialization has completed. */
            r = stm.update_timing_info();
        }

        stm.context.mainloop.unlock();

        if !r {
            stm.destroy();
            return Err(cubeb::Error::Unclassified);
        }

            /*
            if g_log_level != 0 {
                if output_stream_params.is_some() {
                    let att = pa_stream_get_buffer_attr(stm.output_stream.0);
                    LOG("Output buffer attributes maxlength %u, tlength %u, prebuf %u, minreq \
                         %u, fragsize %u",
                        (*att).maxlength,
                        (*att).tlength,
                        (*att).prebuf,
                        (*att).minreq,
                        (*att).fragsize);
                }

                if input_stream_params.is_some() {
                    let att = pa_stream_get_buffer_attr(stm.input_stream.0);
                    LOG("Input buffer attributes maxlength %u, tlength %u, prebuf %u, minreq %u, \
                         fragsize %u",
                        (*att).maxlength,
                        (*att).tlength,
                        (*att).prebuf,
                        (*att).minreq,
                        (*att).fragsize);
                }
            }
*/

        Ok(stm)
    }

    fn destroy(&mut self) {

        fn shutdown_stream(stm: &libpulse::Stream) {
            if !stm.is_null() {
                stm.clear_state_callback();
                stm.clear_write_callback();
                stm.disconnect();
            }
        }

        self.cork(CORK);

        self.context.mainloop.lock();
        if !self.drain_timer.is_null() {
            /* there's no pa_rttime_free, so use this instead. */
            let ma = self.context.mainloop.get_api();
            ma.time_free(&self.drain_timer);
        }

        shutdown_stream(&self.output_stream);
        shutdown_stream(&self.input_stream);

        self.context.mainloop.unlock();

        self.output_stream = Default::default();
        self.input_stream = Default::default();
    }

    fn cork_stream(&self, stm: &libpulse::Stream, state: CorkState) {
        if !stm.is_null() {
            if let Ok(o) = stm.cork(state & CORK,
                                    success_callback,
                                    self as *const _ as *mut c_void) {
                self.context.operation_wait(stm, &o);
            }
        }
    }

    fn cork(&mut self, state: CorkState) {
        self.context.mainloop.lock();
        self.cork_stream(&self.output_stream, state);
        self.cork_stream(&self.input_stream, state);
        self.context.mainloop.unlock();

        if (state & NOTIFY) != 0 {
            self.invoke_state_change_callback(match state & CORK {
                CORK => cubeb::State::Stopped,
                _ => cubeb::State::Started,
            });
        }
    }

    fn invoke_state_change_callback(&mut self, s: cubeb::State) {
        self.state = s;
        (self.state_callback)(self.user_ptr, s);
    }

    fn trigger_user_callback(&mut self,
                             st: StreamType,
                             input_data: *const c_void,
                             nbytes: usize) {

        fn stream_drain_callback(ma: &libpulse::MainloopApi,
                                 e: &libpulse::TimeEvent,
                                 _tv: &Struct_timeval,
                                 u: *mut c_void)
        {
            let stm = cast_mut::<PulseStream>(u);
            debug_assert!(stm.drain_timer == *e);
            stm.invoke_state_change_callback(cubeb::State::Drained);
            /* there's no pa_rttime_free, so use this instead. */
            ma.time_free(&stm.drain_timer);
            stm.drain_timer = Default::default();
            stm.context.mainloop.signal(false);
        }

        let frame_size = ::libpulse::frame_size(&self.output_sample_spec);
        debug_assert!(nbytes as size_t % frame_size == 0);

        let mut towrite = nbytes;
        let mut read_offset: size_t = 0;
        while towrite > 0 {
            match self.get_stream(st).begin_write(towrite) {
                Err(_) => { panic!(""); },
                Ok((buffer, size)) => {
                    debug_assert!(size > 0);
                    debug_assert!(size % frame_size == 0);

                    //LOGV("Trigger user callback with output buffer size=%zd, read_offset=%zd",
                    //size, read_offset);
                    let read_ptr = unsafe { (input_data as *const u8).offset(read_offset as isize) };
                    let got = match (self.data_callback)(self.user_ptr,
                                                         read_ptr as *mut c_void,
                                                         buffer,
                                                         size / frame_size) {
                        Ok(x) => x,
                        Err(_) => {
                            self.get_stream(st).cancel_write();
                            self.shutdown = true;
                            return;
                        }
                    };

                    // If more iterations move offset of read buffer
                    if !input_data.is_null() {
                        let in_frame_size = ::libpulse::frame_size(&self.input_sample_spec);
                        read_offset += (size / frame_size) * in_frame_size;
                    }

                    if self.volume != PULSE_NO_GAIN {
                        let samples = self.output_sample_spec.channels as size_t * size / frame_size;

                        if self.output_sample_spec.format == PA_SAMPLE_S16BE ||
                            self.output_sample_spec.format == PA_SAMPLE_S16LE {
                                let b = unsafe { slice::from_raw_parts_mut(buffer as *mut i16, samples) };
                                let vol = self.volume as i16;
                                for f in b.iter_mut().take(samples) {
                                    *f *= vol;
                                }
                            } else {
                                let b = unsafe { slice::from_raw_parts_mut(buffer as *mut f32, samples) };
                                for f in b.iter_mut().take(samples) {
                                    *f *= self.volume;
                                }
                            }
                    }

                    let r = self.get_stream(st).write(buffer, got * frame_size, 0, PA_SEEK_RELATIVE);
                    debug_assert!(r.is_ok());

                    if got < size / frame_size {
                        let latency = match self.get_stream(st).get_latency() {
                            Ok((l, _)) => l,
                            Err(e) => {
                                debug_assert!(e == libpulse::ErrorCode::from_error_code(PA_ERR_NODATA));
                                /* this needs a better guess. */
                                100 * PA_USEC_PER_MSEC
                            }
                        };

                        /* pa_stream_drain is useless, see PA bug# 866. this is a workaround. */
                        /* arbitrary safety margin: double the current latency. */
                        debug_assert!(!self.drain_timer.is_null());
                        let self_void_ptr = cast_void_ptr(self);
                        self.drain_timer = self.context.context.rttime_new(
                            libpulse::rtclock_now() + 2 * latency,
                            stream_drain_callback,
                            self_void_ptr);
                        self.shutdown = false;
                        return;
                    }

                    towrite -= size;
                }
            }
        }

        debug_assert!(towrite == 0);
    }

    fn update_timing_info(&mut self) -> bool {
        let self_void_ptr = to_void_ptr(self);

        if !self.output_stream.is_null() {
            if let Ok(o) = self.output_stream.update_timing_info(success_callback, self_void_ptr) {
                if !self.context.operation_wait(&self.output_stream, &o) {
                    return false;
                }
            }
        }

        if !self.input_stream.is_null() {
            if let Ok(o) = self.input_stream.update_timing_info(success_callback, self_void_ptr) {
                if !self.context.operation_wait(&self.input_stream, &o) {
                    return false;
                }
            }
        }

        true
    }

    fn wait_until_ready(&mut self) -> bool {
        fn wait_until_io_stream_ready(stm: &libpulse::Stream,
                                      mainloop: &libpulse::ThreadedMainloop)
                                      -> bool {
            loop {
                let state = stm.get_state() ;
                if !state.is_good() {
                    return false;
                }
                if state == PA_STREAM_READY {
                    break;
                }
                mainloop.wait();
            }

            true
        }

        if !self.output_stream.is_null()
            && !wait_until_io_stream_ready(&self.output_stream, &self.context.mainloop) {
                return false;
            }

        if !self.input_stream.is_null()
            && !wait_until_io_stream_ready(&self.input_stream, &self.context.mainloop) {
                return false;
            }

        true
    }
}

impl<'a> ::std::ops::Drop for PulseStream<'a> {
    fn drop(&mut self) {
        self.destroy();
    }
}

impl<'a> cubeb::Stream for PulseStream<'a> {
    fn start(&mut self) -> cubeb::Result<()> {
        fn defer_event_cb(_: &libpulse::MainloopApi, u: *mut c_void) {
            let mut stm = cast_mut::<PulseStream>(u);
            if stm.shutdown {
                return;
            }
            if let Ok(size) = stm.output_stream.writable_size() {
                stm.trigger_user_callback(OUTPUT, ptr::null_mut(), size);
            }
        }

        self.shutdown = false;
        self.cork(UNCORK | NOTIFY);

        let self_void_ptr = to_void_ptr(self);
        if !self.output_stream.is_null() && self.input_stream.is_null() {
            /* On output only case need to manually call user cb once in order to make
             * things roll. This is done via a defer event in order to execute it
             * from PA server thread. */
            self.context.mainloop.lock();
            self.context.mainloop.get_api().once(defer_event_cb, self_void_ptr);
            self.context.mainloop.unlock();
        }
        Ok(())
    }

    fn stop(&mut self) -> cubeb::Result<()> {
        self.context.mainloop.lock();
        self.shutdown = true;
        // If draining is taking place wait to finish
        while !self.drain_timer.is_null() {
            self.context.mainloop.wait();
        }
        self.context.mainloop.unlock();

        self.cork(CORK | NOTIFY);

        Ok(())
    }

    fn get_position(&self) -> cubeb::Result<u64> {
        let in_thread = self.context.mainloop.in_thread();

        if !in_thread { self.context.mainloop.lock(); }

        let r = if self.output_stream.is_null() {
            Err(cubeb::Error::Unclassified)
        } else {
            match self.output_stream.get_time() {
                Ok(r_usec) => {
                    let bytes = unsafe { pa_usec_to_bytes(r_usec, &self.output_sample_spec) };
                    Ok((bytes / libpulse::frame_size(&self.output_sample_spec)) as u64)
                },
                Err(_) => {  Err(cubeb::Error::Unclassified) }
            }
        };

        if !in_thread { self.context.mainloop.unlock(); }

        r
    }

    fn get_latency(&self) -> cubeb::Result<u32> {
        if self.output_stream.is_null() {
            Err(cubeb::Error::Unclassified)
        } else {
            match self.output_stream.get_latency() {
                Ok((r_usec, negative)) => {
                    debug_assert!(!negative);
                    let latency = (r_usec * self.output_sample_spec.rate as libpulse::USec / PA_USEC_PER_SEC) as u32;
                    Ok(latency)
                },
                Err(_) => Err(cubeb::Error::Unclassified)
            }
        }
    }

    fn set_volume(&mut self, volume: f32) -> cubeb::Result<()> {

        fn volume_success(_: &libpulse::Context, success: c_int, u: *mut c_void) {
            let ctx = cast_mut::<PulseContext>(u);
            debug_assert!(success != 0);
            ctx.mainloop.signal(false);
        }

        if self.output_stream.is_null() {
            Err(cubeb::Error::Unclassified)
        } else {
            self.context.mainloop.lock();
            while self.context.default_sink_info.is_none() {
                self.context.mainloop.wait();
            }

            let mut cvol: libpulse::CVolume = Default::default();

            /* if the pulse daemon is configured to use flat volumes,
             * apply our own gain instead of changing the input volume
             * on the sink. */
            let flags = {
                match self.context.default_sink_info.as_ref() {
                    Some(info) => info.flags,
                    None => 0
                }
            };

            if (flags & PA_SINK_FLAT_VOLUME) != 0 {
                self.volume = volume;
            } else {
                let ss = self.output_stream.get_sample_spec();
                let vol = unsafe { pa_sw_volume_from_linear(volume as f64) };
                unsafe { pa_cvolume_set(&mut cvol, (*ss).channels as u32, vol) };

                let index = self.output_stream.get_index();

                let self_void_ptr = cast_void_ptr(self);
                if let Ok(o) = self.context.context.set_sink_input_volume(index,
                                                                          &cvol,
                                                                          volume_success,
                                                                          self_void_ptr) {
                    self.context.operation_wait(&self.output_stream, &o);
                }
            }

            self.context.mainloop.unlock();

            Ok(())
        }
    }

    fn set_panning(&mut self, panning: f32) -> cubeb::Result<()> {

        #[repr(C)]
        #[derive(Copy, Clone)]
        struct sink_input_info_result<'a> {
            pub cvol: libpulse::CVolume,
            pub mainloop: &'a libpulse::ThreadedMainloop,
        }

        fn sink_input_info_cb(_c: &libpulse::Context,
                              info: &libpulse::SinkInputInfo,
                              eol: i32,
                              u: *mut c_void) {
            let r = cast_mut::<sink_input_info_result>(u);
            if eol == 0 {
                r.cvol = info.volume;
            }
            r.mainloop.signal(false);
        }

        fn volume_success(_: &libpulse::Context, success: c_int, u: *mut c_void) {
            let ctx = cast_mut::<PulseContext>(u);
            debug_assert!(success != 0);
            ctx.mainloop.signal(false);
        }

        if self.output_stream.is_null() {
            Err(cubeb::Error::Unclassified)
        } else {
            self.context.mainloop.lock();

            let map = self.output_stream.get_channel_map();
            if unsafe { pa_channel_map_can_balance(map) } == 0 {
                return Err(cubeb::Error::Unclassified);
            }

            let index = self.output_stream.get_index();

            let mut r = sink_input_info_result {
                cvol: Default::default(),
                mainloop: &self.context.mainloop,
            };

            if let Ok(o) = self.context.context.get_sink_input_info(index,
                                                                    sink_input_info_cb,
                                                                    &mut r as *mut _ as *mut c_void)
            {
                self.context.operation_wait(&self.output_stream, &o);
            }

            unsafe { pa_cvolume_set_balance(&mut r.cvol, map, panning); }

            if let Ok(o) = self.context.context.set_sink_input_volume(index,
                                                                      &r.cvol,
                                                                      volume_success,
                                                                      self.context as *const _ as *mut c_void) {
                self.context.operation_wait(&self.output_stream, &o);
            }

            self.context.mainloop.unlock();

            Ok(())
        }
    }

    fn get_current_device(&self) -> cubeb::Result<cubeb::Device> {
        Ok(cubeb::Device {
            output_name: "".to_string(),
            input_name: "".to_string(),
        })
    }
}


fn to_void_ptr(s: &PulseStream) -> *mut c_void {
    s as *const _ as *mut c_void
}
