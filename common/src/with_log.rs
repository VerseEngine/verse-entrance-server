use log::{debug, error, info, trace, warn};
pub trait WithLog {
    fn if_err_trace(&self, msg: &str);
    fn if_err_debug(&self, msg: &str);
    fn if_err_info(&self, msg: &str);
    fn if_err_warn(&self, msg: &str);
    fn if_err_error(&self, msg: &str);
}
impl<T> WithLog for Result<T, anyhow::Error> {
    #[allow(dead_code)]
    fn if_err_warn(&self, msg: &str) {
        if let Err(e) = self {
            warn!("result failed: {} {:?}", msg, e);
        }
    }
    #[allow(dead_code)]
    fn if_err_error(&self, msg: &str) {
        if let Err(e) = self {
            error!("result failed: {} {:?}", msg, e);
        }
    }
    #[allow(dead_code)]
    fn if_err_info(&self, msg: &str) {
        if let Err(e) = self {
            info!("result failed: {} {:?}", msg, e);
        }
    }
    #[allow(dead_code)]
    fn if_err_debug(&self, msg: &str) {
        if let Err(e) = self {
            debug!("result failed: {} {:?}", msg, e);
        }
    }
    #[allow(dead_code)]
    fn if_err_trace(&self, msg: &str) {
        if let Err(e) = self {
            trace!("result failed: {} {:?}", msg, e);
        }
    }
}

#[cfg(not(target_family = "wasm"))]
impl<T> WithLog for std::result::Result<T, webrtc::Error> {
    #[allow(dead_code)]
    fn if_err_warn(&self, msg: &str) {
        if let Err(e) = self {
            warn!("result failed: {} {:?}", msg, e);
        }
    }
    #[allow(dead_code)]
    fn if_err_error(&self, msg: &str) {
        if let Err(e) = self {
            error!("result failed: {} {:?}", msg, e);
        }
    }
    #[allow(dead_code)]
    fn if_err_info(&self, msg: &str) {
        if let Err(e) = self {
            info!("result failed: {} {:?}", msg, e);
        }
    }
    #[allow(dead_code)]
    fn if_err_debug(&self, msg: &str) {
        if let Err(e) = self {
            debug!("result failed: {} {:?}", msg, e);
        }
    }
    #[allow(dead_code)]
    fn if_err_trace(&self, msg: &str) {
        if let Err(e) = self {
            trace!("result failed: {} {:?}", msg, e);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use logtest::Logger;

    #[test]
    fn test_with_log() {
        let mut logger = Logger::start();

        raise_error_fn(true, "").if_err_trace("errtrace");
        assert_eq!(logger.len(), 0);
        raise_error_fn(false, "raise").if_err_trace("errtrace");
        assert_eq!(logger.len(), 1);
        assert_eq!(
            logger.pop().unwrap().args(),
            "result failed: errtrace raise"
        );

        raise_error_fn(true, "").if_err_debug("errdebug");
        assert_eq!(logger.len(), 0);
        raise_error_fn(false, "raise").if_err_debug("errdebug");
        assert_eq!(logger.len(), 1);
        assert_eq!(
            logger.pop().unwrap().args(),
            "result failed: errdebug raise"
        );

        raise_error_fn(true, "").if_err_info("errinfo");
        assert_eq!(logger.len(), 0);
        raise_error_fn(false, "raise").if_err_info("errinfo");
        assert_eq!(logger.len(), 1);
        assert_eq!(logger.pop().unwrap().args(), "result failed: errinfo raise");

        raise_error_fn(true, "").if_err_warn("errwarn");
        assert_eq!(logger.len(), 0);
        raise_error_fn(false, "raise1").if_err_warn("errwarn");
        assert_eq!(logger.len(), 1);
        assert_eq!(
            logger.pop().unwrap().args(),
            "result failed: errwarn raise1"
        );

        raise_error_fn(true, "").if_err_error("errerror");
        assert_eq!(logger.len(), 0);
        raise_error_fn(false, "raise2").if_err_error("errerror");
        assert_eq!(logger.len(), 1);
        assert_eq!(
            logger.pop().unwrap().args(),
            "result failed: errerror raise2"
        );
    }

    fn raise_error_fn(is_success: bool, err: &str) -> Result<()> {
        if !is_success {
            Err(anyhow::anyhow!(err.to_string()))
        } else {
            Ok(())
        }
    }
}
