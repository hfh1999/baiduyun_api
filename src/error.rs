//! 错误模型设计契约
//!
//! # 两值域不变式
//!
//! `errno_id`(经 [ApiError::ret_errno] 读取)只可能有两个值域:
//!
//! 1. **百度透传码**:来自响应 body 的 `errno`(xpan 接口)或 `error_code`(pcs 接口)——
//!    两字段是同一语义体系(百度错误码全局一致),由 `parse_response` 统一识别后透传;
//!    调用方查百度错误码文档分诊(如 `-6` 换 token、`31034` 降频)。
//! 2. **内部错误码 `8989`**:网络失败/解析失败/参数校验/本地 IO 等库侧错误,
//!    具体原因在 [ApiError::ret_prompt] / Display 的文案中("场景: 底层原因")。
//!
//! # 统一包装端口(全库错误只从两个口进入)
//!
//! - 透传口:`parse_response`(HTTP 状态 + body → 百度码透传)
//! - 内部口:[ContextExt::context](ContextExt)(任意底层错误 + 场景 → 8989)
//!
//! HTTP 状态码既不是值域也不透传——它只决定走成功/错误解析路径,
//! body 解析不出业务码时作为文案线索出现在 8989 错误中。
//!
//! # 诊断规则(使用方)
//!
//! ```text
//! ret_errno() != 8989 → 百度返回的码,查百度错误码表
//! ret_errno() == 8989 → 库内部错误,看文案(含接口身份/场景/底层原因)
//! ```
//!
//! # 无堆栈决策
//!
//! [ApiError] 不携带调用堆栈(符合"库错误不带栈"的生态惯例);
//! 需要 backtrace 的调用方自行用 anyhow/eyre 包装。

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
/// println!("{}", error); // 输出: Api inner error.: custom error message
/// ```
pub struct ApiError {
    errno_prompt: String,
    custom_prompt: String,
    errno_id: i64,
}
impl ApiError {
    // ===== 已知错误码常量(百度 xpan errno / pcs error_code 同一语义体系) =====
    /// 参数错误
    pub const E_ARG: i64 = 2;
    /// 认证失败(无效 access_token)
    pub const E_AUTH: i64 = -6;
    /// 文件名/路径非法
    pub const E_INVALID_NAME: i64 = -7;
    /// 路径已存在(mkdir 等 create 接口)
    pub const E_ALREADY_EXISTS: i64 = -8;
    /// 文件/目录不存在
    pub const E_NOT_FOUND: i64 = -9;
    /// 接口频控(应降频/退避)
    pub const E_FREQ: i64 = 31034;
    /// user not exists(下载 dlink 403 等)
    pub const E_USER_NOT_FOUND: i64 = 31045;
    /// 文件已存在(upload 冲突 ondup=fail)
    pub const E_FILE_EXISTS: i64 = 31061;
    /// 路径无权限(上传路径须位于 /apps/{应用名}/ 下)
    pub const E_PATH_UNAUTHORIZED: i64 = 31064;
    /// 风控拦截(如 UA 不匹配触发 hitcode)
    pub const E_RISK_CONTROL: i64 = 31326;
    /// 库内部错误(两值域不变式的内部侧,见模块文档)
    pub const E_INTERNAL: i64 = 8989;

    /// 新建一个ApiError类型
    ///
    /// # Example
    ///```
    ///use baiduyun_api::ApiError;
    ///let myerror = ApiError::new(8989, "unknow error.");
    ///println!("{}",myerror.ret_prompt());
    ///```
    pub fn new(errno: i64, custom_info: &str) -> ApiError {
        // 文案映射:已知码给友好说明;未知码显示码本身(保持可诊断,替代无意义占位)
        let errno_str: String = match errno {
            Self::E_ARG => String::from("argument error, please check your argument."),
            Self::E_AUTH => String::from("authentication failed, please check your access token."),
            Self::E_INVALID_NAME => String::from("invalid filename or path."),
            Self::E_ALREADY_EXISTS => String::from("path already exists."),
            Self::E_NOT_FOUND => String::from("file or directory not found."),
            Self::E_FREQ => String::from("hit interface frequency control, please slow down."),
            Self::E_USER_NOT_FOUND => String::from("user not found or no permission."),
            Self::E_FILE_EXISTS => String::from("file already exists."),
            Self::E_PATH_UNAUTHORIZED => {
                String::from("path not authorized (upload requires /apps/{app}/).")
            }
            Self::E_RISK_CONTROL => String::from("request blocked by risk control."),
            Self::E_INTERNAL => String::from("Api inner error."),
            // 以下为百度文档化码,仅做文案映射(库逻辑不主动产生)
            42000 => String::from("Your try is too often,Please wait for a moment."),
            42001 => String::from("Rand verification failed"),
            42999 => String::from("This Funtion have been revoked."),
            9100 => String::from("You have been banned:No.1"),
            9200 => String::from("You have been banned:No.2"),
            9300 => String::from("You have been banned:No.3"),
            9400 => String::from("You have been banned:No.4"),
            9500 => String::from("You have been banned:No.5"),
            _ => format!("errno {errno}"), // 未知码:显示码本身,提示查码表
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

/// 给 [Result] 添加上下文标注的内部扩展 trait(anyhow 形态)
///
/// 统一包装逻辑的"内部口":任何底层错误(ureq/io/...)+ 场景 → [ApiError](errno=8989),
/// 文案为 `"场景: 底层原因"`。仅库内部使用(pub(crate)),见模块文档"统一包装端口"。
pub(crate) trait ContextExt<T> {
    /// 失败时包装为内部 ApiError;成功时原样通过
    fn context(self, ctx: impl std::fmt::Display) -> Result<T, ApiError>;
}

impl<T, E: std::fmt::Display> ContextExt<T> for Result<T, E> {
    fn context(self, ctx: impl std::fmt::Display) -> Result<T, ApiError> {
        self.map_err(|e| ApiError::from(format!("{ctx}: {e}").as_str()))
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
        assert!(display.contains("Api inner error."));
        assert!(display.contains("custom error message"));
    }

    #[test]
    fn test_api_error_display_without_custom_prompt() {
        let error = ApiError::new(8989, "");
        let display = format!("{}", error);
        assert_eq!(display, "Api inner error.");
    }

    #[test]
    fn test_api_error_from_str() {
        let error = ApiError::from("test error");
        assert_eq!(error.ret_errno(), ApiError::E_INTERNAL);
        let display = format!("{}", error);
        assert!(display.contains("test error"));
    }

    #[test]
    fn test_api_error_ret_prompt() {
        let error = ApiError::new(ApiError::E_ARG, "test");
        assert_eq!(
            error.ret_prompt(),
            "argument error, please check your argument."
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
        // 映射表:常量定义的码应有对应友好文案
        let test_cases = vec![
            (
                ApiError::E_ARG,
                "argument error, please check your argument.",
            ),
            (
                ApiError::E_AUTH,
                "authentication failed, please check your access token.",
            ),
            (ApiError::E_INVALID_NAME, "invalid filename or path."),
            (ApiError::E_ALREADY_EXISTS, "path already exists."),
            (ApiError::E_NOT_FOUND, "file or directory not found."),
            (
                ApiError::E_FREQ,
                "hit interface frequency control, please slow down.",
            ),
            (
                ApiError::E_USER_NOT_FOUND,
                "user not found or no permission.",
            ),
            (ApiError::E_FILE_EXISTS, "file already exists."),
            (
                ApiError::E_PATH_UNAUTHORIZED,
                "path not authorized (upload requires /apps/{app}/).",
            ),
            (ApiError::E_RISK_CONTROL, "request blocked by risk control."),
            (ApiError::E_INTERNAL, "Api inner error."),
        ];
        for (errno, expected_prompt) in test_cases {
            let error = ApiError::new(errno, "test");
            assert_eq!(
                error.ret_prompt(),
                expected_prompt,
                "errno={errno} 文案应匹配"
            );
        }
    }

    #[test]
    fn test_api_error_unknown_error_code() {
        // 未知码:显示码本身(可诊断),替代无意义占位
        let error = ApiError::new(99999, "test");
        assert_eq!(error.ret_prompt(), "errno 99999");
        let display = format!("{}", error);
        assert!(display.contains("errno 99999"));
        assert!(display.contains("test"));
    }

    #[test]
    fn test_context_ext_wraps_internal_error() {
        // 内部口:底层错误 + 场景 -> 8989,文案 "场景: 底层原因"
        use crate::error::ContextExt;
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "no such file");
        let result: Result<(), std::io::Error> = Err(io_err);
        let error = result.context("open local file").unwrap_err();
        assert_eq!(error.ret_errno(), ApiError::E_INTERNAL);
        let display = format!("{}", error);
        assert!(
            display.contains("open local file: no such file"),
            "实际: {display}"
        );
    }

    #[test]
    fn test_context_ext_passthrough_ok() {
        use crate::error::ContextExt;
        let result: Result<i32, std::io::Error> = Ok(42);
        assert_eq!(result.context("should not matter").unwrap(), 42);
    }
}
