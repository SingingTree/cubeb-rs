/*
 * Copyright © 2011 Mozilla Foundation
 *
 * This program is made available under an ISC-style license.  See the
 * accompanying file LICENSE for details.
 */

/* libcubeb api/function test. Plays a simple tone. */
extern crate cubeb;
extern crate libc;

use libc::c_void;
use std::f32::consts;
use std::{ptr,slice};

const SAMPLE_FREQUENCY: i32 = 48000;
#[cfg(target_os = "windows")]
const STREAM_FORMAT: cubeb::SampleFormat = cubeb::SAMPLE_FLOAT32LE;
#[cfg(not(target_os = "windows"))]
const STREAM_FORMAT: cubeb::SampleFormat = cubeb::SAMPLE_S16LE;

/* store the phase of the generated waveform */
struct CbUserData {
  position: usize
}

fn delay(ms: u32)
{
    unsafe { libc::usleep(1000 * ms); }
}

fn sine_tone(phase: f32, freq: f32) -> f32 { f32::sin(2.0*consts::PI * phase * freq / SAMPLE_FREQUENCY as f32) }

#[cfg(target_os = "windows")]
fn blend3(t1: f32, t2: f32, t3: f32) -> f32 {
    (t1 + t2 + t3) / 3.0
}

#[cfg(not(target_os = "windows"))]
fn blend3(t1: f32, t2: f32, t3: f32) -> i16 {
    ((::std::i16::MAX as f32 / 3.0) * (t1 + t2 + t3)) as i16
}

fn data_cb_tone(user_ptr: *mut c_void,
                _inputbuffer: *const c_void,
                outputbuffer: *mut c_void,
                nframes: usize) -> cubeb::Result<usize>
{
    let u = unsafe { &mut  *(user_ptr as *mut CbUserData) };
    #[cfg(target_os = "windows")]
    let b = unsafe { slice::from_raw_parts_mut(outputbuffer as *mut f32, nframes as usize) };
    #[cfg(not(target_os = "windows"))]
    let mut b = unsafe { slice::from_raw_parts_mut(outputbuffer as *mut i16, nframes as usize) };

    if user_ptr.is_null() {
        return Err(cubeb::Error::Unclassified);
    }

    /* generate our test tone on the fly */
    for (i, f) in b.iter_mut().enumerate().take(nframes) {
        /* Australian dial tone */
        let t1 = sine_tone((i + u.position) as f32, 400.0);
        let t2 = sine_tone((i + u.position) as f32, 425.0);
        let t3 = sine_tone((i + u.position) as f32, 450.0);
        *f = blend3(t1, t2, t3);
    }
    /* remember our phase to avoid clicking on buffer transitions */
    /* we'll still click if position overflows */
    u.position += nframes;

    Ok(nframes)
}

fn state_cb_tone(user_ptr: *mut c_void, state: cubeb::State)
{
    if user_ptr.is_null() {
        return;
    }

    match state {
        cubeb::State::Started => println!("stream started"),
        cubeb::State::Stopped => println!("stream stopped"),
        cubeb::State::Drained => println!("stream drained"),
        _ => println!("unknown stream state {:?}", state)
    }
}

fn main()
{
    let r = cubeb::init("Cubeb tone example");
    let mut ctx = r.expect("Error initializing cubeb library");

    let params = cubeb::StreamParams::new(STREAM_FORMAT, SAMPLE_FREQUENCY, 1, cubeb::ChannelLayout::Mono);
    let user_data = CbUserData { position: 0 };

    let data_cb = &data_cb_tone;
    let state_cb = &state_cb_tone;
    let r = ctx.stream_init("Cubeb tone (mono)", ptr::null_mut(), None, ptr::null_mut(), Some(params),
                        4096, data_cb, &state_cb, &user_data as *const _ as *mut c_void);
    let mut stream = r.expect("Error initializing cubeb stream");

    stream.start().expect("Failed to start stream");
    delay(5000);
    stream.stop().expect("Failed to stop stream");

    debug_assert!(user_data.position > 0);
}
