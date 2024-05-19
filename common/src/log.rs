#[macro_export]
macro_rules! logmsg {
    () => {
        &format!("{}:{}", file!(), line!())
    };
    ( $v:expr ) => {
        &format!("{} {}:{}", $v, file!(), line!())
    };
}
pub use logmsg;
