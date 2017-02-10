extern crate libc;

mod context;
mod stream;

pub type Result<T> = ::std::result::Result<T, Error>;

pub use context::Context;
pub use stream::Stream;


/** Sample format enumeration. */
pub type SampleFormat = i32;
/**< Little endian 16-bit signed PCM. */
pub const SAMPLE_S16LE: SampleFormat = 0;
/**< Big endian 16-bit signed PCM. */
pub const SAMPLE_S16BE: SampleFormat = 1;
/**< Little endian 32-bit IEEE floating point PCM. */
pub const SAMPLE_FLOAT32LE: SampleFormat = 2;
/**< Big endian 32-bit IEEE floating point PCM. */
pub const SAMPLE_FLOAT32BE: SampleFormat = 3;
/**< Native endian 16-bit signed PCM. */
#[cfg(target_endian = "big")]
pub const SAMPLE_S16NE: SampleFormat = SAMPLE_S16BE;
/**< Native endian 32-bit IEEE floating point PCM. */
#[cfg(target_endian = "big")]
pub const SAMPLE_FLOAT32NE: SampleFormat = SAMPLE_FLOAT32BE;
/**< Native endian 16-bit signed PCM. */
#[cfg(target_endian = "little")]
pub const SAMPLE_S16NE: SampleFormat = SAMPLE_S16LE;
/**< Native endian 32-bit IEEE floating point PCM. */
#[cfg(target_endian = "little")]
pub const SAMPLE_FLOAT32NE: SampleFormat = SAMPLE_FLOAT32LE;

/**
 * This maps to the underlying stream types on supported platforms, e.g.
 * Android.
 */
#[cfg(target_os = "android")]
pub enum cubeb_stream_type {
    VoiceCall = 0,
    System = 1,
    Ring = 2,
    Music = 3,
    Alarm = 4,
    Notification = 5,
    BluetoothSco = 6,
    SystemEnforced = 7,
    Dtmf = 8,
    Tts = 9,
    Fm = 10,
}

/// An opaque handle used to refer to a particular input or output device across calls.
//typedef void const * cubeb_devid;
pub type DeviceId = *mut ::libc::c_void;

/// Level (verbosity) of logging for a particular cubeb context.
pub enum LogLevel {
    /// Logging disabled
    LogDisabled,
    /// Logging lifetime operation (creation/destruction).
    LogNormal,
    /// Verbose logging of callbacks, can have performance implications.
    LogVerbose,
}

/** SMPTE channel layout (also known as wave order)
 * DUAL-MONO      L   R
 * DUAL-MONO-LFE  L   R   LFE
 * MONO           M
 * MONO-LFE       M   LFE
 * STEREO         L   R
 * STEREO-LFE     L   R   LFE
 * 3F             L   R   C
 * 3F-LFE         L   R   C    LFE
 * 2F1            L   R   S
 * 2F1-LFE        L   R   LFE  S
 * 3F1            L   R   C    S
 * 3F1-LFE        L   R   C    LFE S
 * 2F2            L   R   LS   RS
 * 2F2-LFE        L   R   LFE  LS   RS
 * 3F2            L   R   C    LS   RS
 * 3F2-LFE        L   R   C    LFE  LS   RS
 * 3F3R-LFE       L   R   C    LFE  RC   LS   RS
 * 3F4-LFE        L   R   C    LFE  RLS  RRS  LS   RS
 *
 * The abbreviation of channel name is defined in following table:
 * Abbr  Channel name
 * ---------------------------
 * M     Mono
 * L     Left
 * R     Right
 * C     Center
 * LS    Left Surround
 * RS    Right Surround
 * RLS   Rear Left Surround
 * RC    Rear Center
 * RRS   Rear Right Surround
 * LFE   Low Frequency Effects
 */

#[derive(Copy, Clone, Debug)]
pub enum ChannelLayout {
    /// Indicate the speaker's layout is undefined.
    Undefined,
    DualMono,
    DualMonoLfe,
    Mono,
    MonoLfe,
    Stereo,
    StereoLfe,
    _3F,
    _3FLfe,
    _2F1,
    _2F1Lfe,
    _3F1,
    _3F1Lfe,
    _2F2,
    _2F2Lfe,
    _3F2,
    _3F2Lfe,
    _3F3RLfe,
    _3F4Lfe,
}

/** Stream format initialization parameters. */
#[derive(Copy, Clone, Debug)]
pub struct StreamParams {
    /// Requested sample format.  One of #cubeb_sample_format.
    pub format: SampleFormat,
    /// Requested sample rate.  Valid range is [1000, 192000].
    pub rate: i32,
    /// Requested channel count.  Valid range is [1, 8].
    pub channels: i32,
    /// Requested channel layout. This must be consistent with the provided channels.
    pub layout: ChannelLayout,
    /// Used to map Android audio stream types
    #[cfg(target_os = "android")]
    pub stream_type: StreamType,
}

impl StreamParams {
  pub fn new(format: SampleFormat, rate: i32, channels: i32, layout: ChannelLayout) -> StreamParams {
    StreamParams {
      format: format, rate: rate, channels: channels, layout: layout
    }
  }
}

/// Audio device description
pub struct Device {
    /// The name of the output device
    pub output_name: String,
    /// The name of the input device
    pub input_name: String,
}

/// Stream states signaled via state_callback.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum State {
    Uninitialized = -1,
    /// Stream started.
    Started,
    /// Stream stopped.
    Stopped,
    /// Stream drained.
    Drained,
    /// Stream disabled due to error.
    Error,
}

/// Result code enumeration.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Error {
    /// Result code enumeration.
    Unclassified,
    /// Unsupported #cubeb_stream_params requested.
    InvalidFormat,
    /// Invalid parameter specified.
    InvalidParameter,
    /// Optional function not implemented in current backend.
    NotSupported,
    /// Device specified by #cubeb_devid not available.
    DeviceUnavailable,
}

/// Whether a particular device is an input device (e.g. a microphone), or an
/// output device (e.g. headphones).
pub enum DeviceType {
    Unknown,
    Input,
    Output,
}

/// The state of a device.
pub enum DeviceState {
    /// The device has been disabled at the system level.
    Disabled,
    /// The device is enabled, but nothing is plugged into it.
    Unplugged,
    /// The device is enabled.
    Enabled,
}

/// Architecture specific sample type.
pub type DeviceFormat = i32;
/// 16-bit integers, Little Endian.
pub const DEVICE_FMT_S16LE: DeviceFormat = 0x0010;
/// 16-bit integers, Big Endian.
pub const DEVICE_FMT_S16BE: DeviceFormat = 0x0020;
/// 32-bit floating point, Little Endian.
pub const DEVICE_FMT_F32LE: DeviceFormat = 0x1000;
/// 32-bit floating point, Big Endian.
pub const DEVICE_FMT_F32BE: DeviceFormat = 0x2000;


/// 16-bit integers, native endianess, when on a Big Endian environment.
#[cfg(target_endian = "big")]
pub const DEVICE_FMT_S16NE: DeviceFormat = DEVICE_FMT_S16BE;
/// 32-bit floating points, native endianess, when on a Big Endian environment.
#[cfg(target_endian = "big")]
pub const DEVICE_FMT_F32NE: DeviceFormat = DEVICE_FMT_F32BE;
/// 16-bit integers, native endianess, when on a Little Endian environment.
#[cfg(target_endian = "little")]
pub const DEVICE_FMT_S16NE: DeviceFormat = DEVICE_FMT_S16LE;
/// 32-bit floating points, native endianess, when on a Little Endian environment.
#[cfg(target_endian = "little")]
pub const DEVICE_FMT_F32NE: DeviceFormat = DEVICE_FMT_F32LE;

/// All the 16-bit integers types.
pub const DEVICE_FMT_S16_MASK: DeviceFormat = (DEVICE_FMT_S16LE | DEVICE_FMT_S16BE);
/// All the 32-bit floating points types.
pub const DEVICE_FMT_F32_MASK: DeviceFormat = (DEVICE_FMT_F32LE | DEVICE_FMT_F32BE);
/// All the device formats types.
pub const DEVICE_FMT_ALL: DeviceFormat = (DEVICE_FMT_S16_MASK | DEVICE_FMT_F32_MASK);

/// Channel type for a `cubeb_stream`. Depending on the backend and platform
/// used, this can control inter-stream interruption, ducking, and volume
/// control.
pub type DevicePref = i32;
pub const DEVICE_PREF_NONE: DevicePref = 0x00;
pub const DEVICE_PREF_MULTIMEDIA: DevicePref = 0x01;
pub const DEVICE_PREF_VOICE: DevicePref = 0x02;
pub const DEVICE_PREF_NOTIFICATION: DevicePref = 0x04;
pub const DEVICE_PREF_ALL: DevicePref = 0x0F;

/// This structure holds the characteristics
/// of an input or output audio device. It can be obtained using
/// `cubeb_enumerate_devices`, and must be destroyed using
/// `cubeb_device_info_destroy`.
pub struct DeviceInfo {
    /// Device identifier handle.
    pub devid: DeviceId,

    /// Device identifier which might be presented in a UI.
    pub device_id: String,
    /// Friendly device name which might be presented in a UI.
    pub friendly_name: String,
    /// Two devices have the same group identifier if they belong to the same
    /// physical device; for example a headset and microphone.
    pub group_id: String,
    /// Optional vendor name, may be NULL.
    pub vendor_name: Option<String>,

    /// Type of device (Input/Output).
    pub dev_type: DeviceType,
    /// State of device disabled/enabled/unplugged.
    pub state: DeviceState,
    /// Preferred device.
    pub preferred: DevicePref,

    /// Sample format supported.
    pub format: DeviceFormat,
    /// The default sample format for this device.
    pub deafult_format: DeviceFormat,
    /// Channels.
    pub max_channels: i32,
    /// Default/Preferred sample rate.
    pub default_rate: i32,
    /// Maximum sample rate supported.
    pub max_rate: i32,
    /// Minimum sample rate supported.
    pub min_rate: i32,

    /// Lowest possible latency in frames.
    pub latency_lo: i32,
    /// Highest possible latency in frames.
    pub latency_hi: i32,
}

/** User supplied data callback.
    - Calling other cubeb functions from this callback is unsafe.
    - The code in the callback should be non-blocking.
    - Returning less than the number of frames this callback asks for or
      provides puts the stream in drain mode. This callback will not be called
      again, and the state callback will be called with CUBEB_STATE_DRAINED when
      all the frames have been output.
    @param stream The stream for which this callback fired.
    @param user_ptr The pointer passed to cubeb_stream_init.
    @param input_buffer A pointer containing the input data, or nullptr
                        if this is an output-only stream.
    @param output_buffer A pointer to a buffer to be filled with audio samples,
                         or nullptr if this is an input-only stream.
    @param nframes The number of frames of the two buffer.
    @retval Number of frames written to the output buffer. If this number is
            less than nframes, then the stream will start to drain.
    @retval CUBEB_ERROR on error, in which case the data callback will stop
            and the stream will enter a shutdown state. */
/*
typedef long (* cubeb_data_callback)(cubeb_stream * stream,
                                     void * user_ptr,
                                     void const * input_buffer,
                                     void * output_buffer,
                                     long nframes);
 */
//pub type DataCB = FnMut(*mut c_void, *const c_void, *mut c_void, usize) -> Result<usize>;

/** User supplied state callback.
    @param stream The stream for this this callback fired.
    @param user_ptr The pointer passed to cubeb_stream_init.
    @param state The new state of the stream. */
/*
typedef void (* cubeb_state_callback)(cubeb_stream * stream,
                                      void * user_ptr,
                                      cubeb_state state);
 */
//pub type StateCB = FnMut(*mut c_void, State);

/**
 * User supplied callback called when the underlying device changed.
 * @param user The pointer passed to cubeb_stream_init. */
/*
typedef void (* cubeb_device_changed_callback)(void * user_ptr);
*/
/**
 * User supplied callback called when the underlying device collection changed.
 * @param context A pointer to the cubeb context.
 * @param user_ptr The pointer passed to cubeb_stream_init. */
/*
typedef void (* cubeb_device_collection_changed_callback)(cubeb * context,
                                                          void * user_ptr);
*/
/** User supplied callback called when a message needs logging. */
// typedef void (* cubeb_log_callback)(char const * fmt, ...);
/** Set a callback to be called with a message.
    @param log_level CUBEB_LOG_VERBOSE, CUBEB_LOG_NORMAL.
    @param log_callback A function called with a message when there is
                        something to log. Pass NULL to unregister.
    @retval CUBEB_OK in case of success.
    @retval CUBEB_ERROR_INVALID_PARAMETER if either context or log_callback are
                                          invalid pointers, or if level is not
                                          in cubeb_log_level. */
/*pub fn set_log_callback<CB>(log_level: LogLevel, log_cb: CB) -> Result<()>
  where CB: Fn();
*/

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {}
}
