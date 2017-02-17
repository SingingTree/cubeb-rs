use {Device, Result};

pub trait Stream {
    /** Start playback.
    @param stream
    @retval CUBEB_OK
    @retval CUBEB_ERROR */
    fn start(&mut self) -> Result<()>;

    /** Stop playback.
    @param stream
    @retval CUBEB_OK
    @retval CUBEB_ERROR */
    fn stop(&mut self) -> Result<()>;

    /** Get the current stream playback position.
    @param stream
    @param position Playback position in frames.
    @retval CUBEB_OK
    @retval CUBEB_ERROR */
    fn get_position(&self) -> Result<u64>;

    /** Get the latency for this stream, in frames. This is the number of frames
    between the time cubeb acquires the data in the callback and the listener
    can hear the sound.
    @param stream
    @param latency Current approximate stream latency in frames.
    @retval CUBEB_OK
    @retval CUBEB_ERROR_NOT_SUPPORTED
    @retval CUBEB_ERROR */
    fn get_latency(&self) -> Result<u32>;

    /** Set the volume for a stream.
    @param stream the stream for which to adjust the volume.
    @param volume a float between 0.0 (muted) and 1.0 (maximum volume)
    @retval CUBEB_OK
    @retval CUBEB_ERROR_INVALID_PARAMETER volume is outside [0.0, 1.0] or
            stream is an invalid pointer
    @retval CUBEB_ERROR_NOT_SUPPORTED */
    fn set_volume(&mut self, volume: f32) -> Result<()>;

    /** If the stream is stereo, set the left/right panning. If the stream is mono,
    this has no effect.
    @param stream the stream for which to change the panning
    @param panning a number from -1.0 to 1.0. -1.0 means that the stream is
           fully mixed in the left channel, 1.0 means the stream is fully
           mixed in the right channel. 0.0 is equal power in the right and
           left channel (default).
    @retval CUBEB_OK
    @retval CUBEB_ERROR_INVALID_PARAMETER if stream is null or if panning is
            outside the [-1.0, 1.0] range.
    @retval CUBEB_ERROR_NOT_SUPPORTED
    @retval CUBEB_ERROR stream is not mono nor stereo */
    fn set_panning(&mut self, panning: f32) -> Result<()>;

    /** Get the current output device for this stream.
    @param stm the stream for which to query the current output device
    @param device a pointer in which the current output device will be stored.
    @retval CUBEB_OK in case of success
    @retval CUBEB_ERROR_INVALID_PARAMETER if either stm, device or count are
            invalid pointers
    @retval CUBEB_ERROR_NOT_SUPPORTED */
    fn get_current_device(&self) -> Result<Device>;

    /// Set a callback to be notified when the output device changes.
    ///
    /// # Arguments
    ///
    /// * `cb` - a function called whenever the device has
    /// changed. Passing NULL allow to unregister a function
    ///
    /// # Errors
    ///
    /// If the backend doesn't support notification of device change,
    /// this function returns `Error::NotSupported`.
    ///
    /// If `cb` is an invalid pointer, this function returns
    /// `Error::InvalidParameter`.
    fn set_device_changed_callback(&mut self, cb: &Fn(&mut Stream, *mut ::libc::c_void)) -> Result<()>;
}
