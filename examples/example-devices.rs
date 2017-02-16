/*
 * Copyright © 2015 Haakon Sporsheim <haakon.sporsheim@telenordigital.com>
 *
 * This program is made available under an ISC-style license.  See the
 * accompanying file LICENSE for details.
 */

/* libcubeb enumerate device test/example.
 * Prints out a list of devices enumerated. */
extern crate cubeb;

use std::io;
use std::io::Write;

fn print_device_info<W: io::Write>(w: &mut W, info: &cubeb::DeviceInfo)
{
    let devtype = match info.devtype {
        cubeb::DEVICE_TYPE_INPUT => "input",
        cubeb::DEVICE_TYPE_OUTPUT => "output",
        _ => "unknown?"

    };

    let devstate = match info.state {
        cubeb::DeviceState::Disabled => "disabled",
        cubeb::DeviceState::Unplugged => "unplugged",
        cubeb::DeviceState::Enabled => "enabled",
    };

    let devdeffmt = match info.default_format {
        cubeb::DEVICE_FMT_S16LE => "S16LE",
        cubeb::DEVICE_FMT_S16BE => "S16BE",
        cubeb::DEVICE_FMT_F32LE => "F32LE",
        cubeb::DEVICE_FMT_F32BE => "F32BE",
        _ => "unknown?"
    };

    let mut devfmts = if (info.format & cubeb::DEVICE_FMT_S16LE) != 0 {
        format!("S16LE")
    } else {
        format!("")
    };
    if (info.format & cubeb::DEVICE_FMT_S16BE) != 0 {
        devfmts = format!("{} S16BE", devfmts);
    }
    if (info.format & cubeb::DEVICE_FMT_F32LE) != 0 {
        devfmts = format!("{} F32LE", devfmts);
    }
    if (info.format & cubeb::DEVICE_FMT_F32BE) != 0 {
        devfmts = format!("{} F32BE", devfmts);
    }

    let mut pref = if (info.preferred & cubeb::DEVICE_PREF_MULTIMEDIA) != 0 {
        format!("MULTIMEDIA")
    } else {
        format!("")
    };
    if (info.preferred & cubeb::DEVICE_PREF_VOICE) != 0 {
        pref = format!("{} VOICE", pref);
    }
    if (info.preferred & cubeb::DEVICE_PREF_NOTIFICATION) != 0 {
        pref = format!("{} NOTIFICATION", pref);
    }

    writeln!(w, "dev: {:?}{}", info.device_id, if info.preferred == cubeb::DEVICE_PREF_ALL {
        " (PREFERRED)"
    } else {
        &pref
    }).expect("Failed to write");
    writeln!(w, "\tName:    {:?}", info.friendly_name).expect("Failed to write");
    if let Some(group_id) = info.group_id.as_ref() {
        writeln!(w, "\tGroup:   {:?}", group_id).expect("Failed to write");
    }
    if let Some(vendor_name) = info.vendor_name.as_ref() {
        writeln!(w, "\tVendor:  {:?}", vendor_name).expect("Failed to write");
    }
    writeln!(w, "\tType:    {}", devtype).expect("Failed to write");
    writeln!(w, "\tState:   {}", devstate).expect("Failed to write");
    writeln!(w, "\tCh:      {}", info.max_channels).expect("Failed to write");
    writeln!(w, "\tFormat:  {} (0x{:x}) (default: {})", devfmts, info.format, devdeffmt).expect("Failed to write");
    writeln!(w, "\tRate:    {} - {} (default: {})", info.min_rate, info.max_rate, info.default_rate).expect("Failed to write");
    writeln!(w, "\tLatency: lo {} frames, hi {} frames\n", info.latency_lo, info.latency_hi).expect("Failed to write");
}

fn main()
{
    let r = cubeb::init("Cubeb audio test");
    let ctx = r.expect("Error initializing cubeb library");

    println!("Enumerating input devices for backend {}", ctx.get_backend_id());

    let mut stderr = io::stderr();
    match ctx.enumerate_devices(cubeb::DEVICE_TYPE_INPUT) {
        Err(cubeb::Error::NotSupported) => {
            writeln!(stderr, "Device enumeration not supported for this backed.").expect("Failed to write");
            return;
        }
        Err(e) => {
            writeln!(stderr, "Error enumerating devices {}", e).expect("Failed to write");
            return;
        }
        Ok(iter) => {
            writeln!(stderr, "Found input devices").expect("Failed to write");
            for info in iter {
                print_device_info(&mut stderr, &info);
            }
        }
    }

    println!("\nEnumerating output devices for backend {}", ctx.get_backend_id());

    match ctx.enumerate_devices(cubeb::DEVICE_TYPE_OUTPUT) {
        Err(cubeb::Error::NotSupported) => {
            writeln!(stderr, "Device enumeration not supported for this backed.").expect("Failed to write");
            return;
        }
        Err(e) => {
            writeln!(stderr, "Error enumerating devices {}", e).expect("Failed to write");
            return;
        }
        Ok(iter) => {
            writeln!(stderr, "Found output devices").expect("Failed to write");
            for info in iter {
                print_device_info(&mut stderr, &info);
            }
        }
    }
}
