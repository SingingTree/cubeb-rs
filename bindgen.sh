#! /usr/bin/env bash

# This script can be use used to generate cubeb-core/src/ffi.rs.
#
# To regenerate cubeb-core/src/ffi.rs, check out cubeb and create a
# build using cmake, then run this script:
#
# $ bindgen.sh ~/Mozilla/cubeb > cubeb-core/src/ffi.rs
#
# The first argument is where to find cubeb source tree.
# The second argument is where to find a build dir containing exports/cubeb_export.h

bindgen=${BINDGEN:-bindgen}
cubeb_dir=${1:-~/Mozilla/cubeb}
cubeb_build_dir=${2:-$cubeb_dir/build}

cubeb_h=$cubeb_dir/include/cubeb/cubeb.h
cubeb_exports=$cubeb_build_dir/exports

script_path=$(cd $(dirname $0); pwd -P)
test_h=$script_path/test.h

if [[ ! -f $cubeb_h ]]; then
    echo >& "Cubeb source not found at $cubeb_dir. $cubeb_h is missing."
    exit 1
fi

if [[ ! -f $cubeb_exports/cubeb_export.h ]]; then
    echo >&2 "Cubeb build not found at $cubeb_exports. $cubeb_exports/cubeb_export.h is missing."
    exit 1
fi

hash $bindgen 2>/dev/null || {
    cat >&2 <<EOF
bindgen is missing from PATH.

Please install with \`cargo install bindgen\` and ensure bindgen command
is added to PATH variable.
EOF
    exit 1
}

whitelist_type='--whitelist-type cubeb.*'
# _bindgen_ty_1 is the name for the anonymous enum containing
# CUBEB_OK, CUBEB_ERROR, etc.
whitelist_type+=' --whitelist-type _bindgen_ty_1'
const_enums='--constified-enum _bindgen_ty_1'
const_enums+=' --constified-enum cubeb_channel_layout'
const_enums+=' --constified-enum cubeb_device_fmt'
const_enums+=' --constified-enum cubeb_device_pref'
const_enums+=' --constified-enum cubeb_device_state'
const_enums+=' --constified-enum cubeb_device_type'
const_enums+=' --constified-enum cubeb_log_level'
const_enums+=' --constified-enum cubeb_sample_format'
const_enums+=' --constified-enum cubeb_state'
rawline='--raw-line #![allow(non_upper_case_globals)]'
rawline+=' --raw-line #![allow(non_camel_case_types)]'
rawline+=' --raw-line #![allow(non_snake_case)]'
# cubeb_device_info shouldn't autoderive Copy, Clone
disallow_copy='--disallow-copy cubeb_device_info'

$bindgen --ignore-functions --no-prepend-enum-name $whitelist_type $const_enums $disallow_copy $rawline $cubeb_h -- -I $cubeb_exports
