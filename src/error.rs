use std::fmt::Display;
#[derive(Debug)]
/// 本api的专有错误类型
///
/// 可以填充和返回错误原因
///
/// 实现了 `std::error::Error` trait，可以与 Rust 标准错误处理机制良好集成
///
/// # Example
/// ```
/// use baiduyun_api::ApiError;
///
/// let error = ApiError::new(8989, "custom error message");
/// println!("{}", error); // 输出: Api innner error.: custom error message
/// ```
pub struct ApiError {
    errno_prompt: String,
    custom_prompt: String,
    errno_id: i64,
}
impl ApiError {
    /// 新建一个ApiError类型
    ///
    /// # Example
    ///```
    ///let myerror = ApiError::new("unknow error.");
    ///println!("{}",myerror.ret_prompt());
    ///```
    pub fn new(errno: i64, custom_info: &str) -> ApiError {
        let errno_str: String = match errno {
            2 => String::from("argument error,Please check your argument."),
            -6 => String::from("Authentication failed,Please check your access token."),
            31034 => String::from("Hit interface frequency control."),
            42000 => String::from("Your try is too often,Please wait for a moment."),
            42001 => String::from("Rand verification failed"),
            42999 => String::from("This Funtion have been revoked."),
            9100 => String::from("You have been banned:No.1"),
            9200 => String::from("You have been banned:No.2"),
            9300 => String::from("You have been banned:No.3"),
            9400 => String::from("You have been banned:No.4"),
            9500 => String::from("You have been banned:No.5"),
            8989 => String::from("Api innner error."),
            _ => String::from("UnKnow errono."),
        };
        ApiError {
            errno_prompt: errno_str,
            custom_prompt: String::from(custom_info),
            errno_id: errno,
        }
    }
    /// 返回api错误的原因
    pub fn ret_prompt(&self) -> String {
        self.errno_prompt.clone()
    }

    /// 返回底层错误码
    pub fn ret_errno(&self) -> i64 {
        self.errno_id
    }
}

impl From<&str> for ApiError {
    fn from(prompt: &str) -> Self {
        ApiError::new(8989, prompt)
    }
}

impl Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.custom_prompt.is_empty() {
            write!(f, "{}", self.errno_prompt)
        } else {
            write!(f, "{}: {}", self.errno_prompt, self.custom_prompt)
        }
    }
}

impl std::error::Error for ApiError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;

    #[test]
    fn test_api_error_display_with_custom_prompt() {
        let error = ApiError::new(8989, "custom error message");
        let display = format!("{}", error);
        assert!(display.contains("Api innner error."));
        assert!(display.contains("custom error message"));
    }

    #[test]
    fn test_api_error_display_without_custom_prompt() {
        let error = ApiError::new(8989, "");
        let display = format!("{}", error);
        assert_eq!(display, "Api innner error.");
    }

    #[test]
    fn test_api_error_from_str() {
        let error = ApiError::from("test error");
        assert_eq!(error.ret_errno(), 8989);
        let display = format!("{}", error);
        assert!(display.contains("test error"));
    }

    #[test]
    fn test_api_error_ret_prompt() {
        let error = ApiError::new(2, "test");
        assert_eq!(
            error.ret_prompt(),
            "argument error,Please check your argument."
        );
    }

    #[test]
    fn test_api_error_ret_errno() {
        let error = ApiError::new(-6, "test");
        assert_eq!(error.ret_errno(), -6);
    }

    #[test]
    fn test_api_error_source() {
        let error = ApiError::new(8989, "test");
        assert!(error.source().is_none());
    }

    #[test]
    fn test_api_error_known_error_codes() {
        let test_cases = vec![
            (2, "argument error,Please check your argument."),
            (-6, "Authentication failed,Please check your access token."),
            (31034, "Hit interface frequency control."),
            (42000, "Your try is too often,Please wait for a moment."),
            (42001, "Rand verification failed"),
            (42999, "This Funtion have been revoked."),
            (9100, "You have been banned:No.1"),
            (9200, "You have been banned:No.2"),
            (9300, "You have been banned:No.3"),
            (9400, "You have been banned:No.4"),
            (9500, "You have been banned:No.5"),
            (8989, "Api innner error."),
        ];

        for (errno, expected_prompt) in test_cases {
            let error = ApiError::new(errno, "test");
            assert_eq!(error.ret_prompt(), expected_prompt);
        }
    }

    #[test]
    fn test_api_error_unknown_error_code() {
        let error = ApiError::new(99999, "test");
        assert_eq!(error.ret_prompt(), "UnKnow errono.");
    }
}
