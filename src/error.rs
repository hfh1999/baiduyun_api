use std::fmt::Display;
#[derive(Debug)]
/// 本api的专有错误类型
///
/// 可以填充和返回错误原因
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
        return self.errno_prompt.clone();
    }

    /// 返回底层错误码
    pub fn ret_errno(&self) -> i64 {
        return self.errno_id;
    }
}
impl Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.errno_prompt)
    }
}
