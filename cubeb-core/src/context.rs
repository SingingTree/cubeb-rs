use libc::c_void;
use *;

pub trait Context {
    fn get_backend_id(&self) -> &str;
    fn get_max_channel_count(&self) -> Result<i32>;
    fn get_min_latency(&self, params: StreamParams) -> Result<i32>;
    fn get_preferred_sample_rate(&self) -> Result<u32>;
    fn get_preferred_channel_layout(&self) -> Result<ChannelLayout>;
    fn stream_init<'a>(&'a mut self,
                       stream_name: &str,
                       input_device: DeviceId,
                       input_stream_params: Option<StreamParams>,
                       output_device: DeviceId,
                       output_stream_params: Option<StreamParams>,
                       latency_frames: u32,
                       data_callback: &'a Fn(*mut c_void, *const c_void, *mut c_void, usize) -> Result<usize>,
                       state_callback: &'a Fn(*mut c_void, State),
                       user_ptr: *mut c_void)
                   -> Result<Box<Stream + 'a>>;

    /** Set a callback to be notified when the output device changes.
    @param stream the stream for which to set the callback.
    @param device_changed_callback a function called whenever the device has
           changed. Passing NULL allow to unregister a function
    @retval CUBEB_OK
    @retval CUBEB_ERROR_INVALID_PARAMETER if either stream or
            device_changed_callback are invalid pointers.
    @retval CUBEB_ERROR_NOT_SUPPORTED */
    fn register_device_changed_callback(&mut self, cb: &Fn()) -> Result<()>;
}
