mod lifetime;

use fxhash::FxHashMap;
use lazy_static::lazy_static;

use lifetime::RUSTC_VER_NUM;

lazy_static! {
    static ref RUF_LIFETIME: FxHashMap<&'static str, [u8; RUSTC_VER_NUM]> =
        lifetime::get_lifetime_raw();
}
