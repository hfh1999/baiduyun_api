use std::fmt::Display;
#[derive(Debug)]
/// 本api的专有错误类型
///
/// 可以填充和返回错误原因
pub struct ApiError {
    prompt: String,
}
impl ApiError {
    /// 新建一个ApiError类型
    ///
    /// # Example
    ///```
    ///let myerror = ApiError::new("unknow error.");
    ///println!("{}",myerror.ret_prompt());
    ///```
    pub fn new(prompt_str: &str) -> ApiError {
        ApiError {
            prompt: String::from(prompt_str),
        }
    }
    /// 返回api错误的原因
    pub fn ret_prompt(&self) -> String {
        return self.prompt.clone();
    }
}
impl Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.prompt)
    }
}
