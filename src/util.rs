use chrono::Local;
use once_cell::sync::Lazy;
use std::ffi::OsStr;

pub fn get_output_name() -> String {
    static TIME: Lazy<String> = Lazy::new(|| {
        Local::now()
            .naive_local()
            .format("demclean-%Y-%m-%d-%H-%M-%S")
            .to_string()
    });

    TIME.clone()
}

pub fn is_demo(ext: &Option<&OsStr>) -> bool {
    ext.and_then(OsStr::to_str)
        .map_or(false, |str| str == "dem")
}
