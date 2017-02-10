use {Error, Result, StreamParams};
use {SAMPLE_S16LE, SAMPLE_S16BE, SAMPLE_FLOAT32LE, SAMPLE_FLOAT32BE};

pub fn valid_frequency(hz: i32) -> bool { hz >= 1000 && hz <= 192000 }
pub fn valid_channel_count(channel: i32) -> bool { channel > 0 && channel <= 8 }

pub fn validate_stream_params(p: &StreamParams) -> Result<()> {
    if !valid_frequency(p.rate) {
        return Err(Error::InvalidFormat);
    }

    if !valid_channel_count(p.channels) {
        return Err(Error::InvalidFormat);
    }

    match p.format {
        SAMPLE_S16LE | SAMPLE_S16BE | SAMPLE_FLOAT32LE | SAMPLE_FLOAT32BE => {
            Ok(())
        }
        _ => {
            Err(Error::InvalidFormat)
        }
    }
}

pub fn validate_duplex_stream_params(input: &StreamParams, output: &StreamParams) -> Result<()> {
    let r = validate_stream_params(output);
    if r.is_err() {
        return r;
    }

    let r = validate_stream_params(input);
    if r.is_err() {
        return r;
    }

    // Rate and sample format must be the same for input and output, if
    // using a duplex stream.
    if input.rate != output.rate || input.format != output.format {
        return Err(Error::InvalidFormat);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use {ChannelLayout, Error, StreamParams};
    use SAMPLE_FLOAT32LE;

    #[test]
    fn valid_frequencies() {
        for hz in 1000..192001 {
            assert!(valid_frequency(hz));
        }
    }

    #[test]
    fn invalid_frequencies() {
        for hz in 0..1000 {
            assert!(!valid_frequency(hz));
        }
        for hz in 192001..193000 {
            assert!(!valid_frequency(hz));
        }
    }

    #[test]
    fn valid_channels() {
        for c in 1..9 {
            assert!(valid_channel_count(c));
        }
    }

    #[test]
    fn invalid_channels() {
        assert!(!valid_channel_count(0));
        assert!(!valid_channel_count(9));
    }

    fn sample_params_ok(p: &StreamParams) -> bool {
        match validate_stream_params(p) {
            Ok(_) => true,
            _ => false,
        }
    }

    #[test]
    fn valid_sample_params() {
        let p = StreamParams::new(SAMPLE_FLOAT32LE,
                                  44100,
                                  2,
                                  ChannelLayout::Stereo);

        assert!(sample_params_ok(&p));
    }

    fn sample_params_err(p: &StreamParams, e: Error) -> bool {
        match validate_stream_params(p) {
            Err(err) => err == e,
            _ => false,
        }
    }

    #[test]
    fn invalid_sample_params() {
        let zero_hz = StreamParams::new(SAMPLE_FLOAT32LE, 0, 2, ChannelLayout::Stereo);
        assert!(sample_params_err(&zero_hz, Error::InvalidFormat));
        let zero_channels = StreamParams::new(SAMPLE_FLOAT32LE,
                                              44100,
                                              0,
                                              ChannelLayout::Stereo);
        assert!(sample_params_err(&zero_channels, Error::InvalidFormat));
    }

    #[test]
    fn valid_duplex_sample_params() {
        let p = StreamParams::new(SAMPLE_FLOAT32LE,
                                  44100,
                                  2,
                                  ChannelLayout::Stereo);
        match validate_duplex_stream_params(&p, &p) {
            Ok(_) => {}
            Err(err) => {
                panic!(err);
            }
        }
    }

    #[test]
    fn invalid_duplex_sample_params() {
        let i = StreamParams::new(SAMPLE_FLOAT32LE,
                                  44100,
                                  2,
                                  ChannelLayout::Stereo);
        let o = StreamParams { rate: 22050, ..i };
        match validate_duplex_stream_params(&i, &o) {
            Ok(_) => {
                panic!();
            }
            Err(Error::InvalidFormat) => {}
            Err(err) => {
                panic!(err);
            }
        }
    }
}
