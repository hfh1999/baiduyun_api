//! 百度网盘开放平台的 Rust 封装——读写、搜索、上传一库搞定,错误信息直透百度真实原因。
//!
//! **本库只提供官方 API 的封装,不提供任何作弊功能。**
//!
//! # 特性
//!
//! - **纯同步 IO、零 async 运行时**:普通 `fn main` 直接跑,不引入 tokio——CLI、脚本、轻量工具开箱即用
//! - **[YunFs](util::YunFs)(推荐)**:像操作本地文件夹一样操作网盘,支持相对路径(`pwd`/`chdir`/`ls`/`mkdir`/`rm`/`mv`/`cp`/`upload`)
//! - **完整读写**:用户信息、空间配额、文件列表、文件信息、下载链接、关键词搜索
//! - **写操作**:创建文件夹、删除、移动、复制、重命名、单步上传(≤2GB)
//! - **错误直透**:百度返回的 `errno` + `errmsg` 原样传递([ApiError])
//! - **零 panic 设计**:网络、解析、格式异常一律返回 `Result`
//!
//! # 快速开始
//!
//! ```no_run
//! use baiduyun_api::YunApi;
//!
//! let api = YunApi::new("你的access_token");
//! let user = api.get_user_info().unwrap();
//! println!("百度账号: {}", user.baidu_name);
//! ```
//!
//! access_token 的获取方式见 [获取 access_token](#获取-accesstoken)。
//!
//! # 使用示例
//!
//! ## 推荐:用 YunFs 像操作本地文件系统一样
//!
//! 日常网盘操作推荐用 [YunFs]:它维护一个"当前目录",支持相对路径(`..`、直接文件名),
//! 与本地文件夹的操作习惯一致,`ls` 自动翻页。
//!
//! ```no_run
//! use baiduyun_api::{util, YunApi};
//!
//! let api = YunApi::new("你的access_token");
//! let mut fs = util::YunFs::new(&api);
//! fs.chdir("学习资料/").unwrap();
//! fs.mkdir("新目录").unwrap();                    // 相对路径自动解析
//! fs.upload("./a.txt", "a.txt").unwrap();
//! for item in fs.ls().unwrap() {
//!     println!("{}", item.server_filename);
//! }
//! fs.download("a.txt", "./a.txt").unwrap();   // 下载到本地(已存在会被覆盖)
//! fs.rm("a.txt").unwrap();
//! ```
//!
//! ## 列出目录内容
//!
//! ```no_run
//! use baiduyun_api::YunApi;
//!
//! let api = YunApi::new("你的access_token");
//! let list = api.get_files_list("/apps", 0, 100).unwrap();
//! for file in list {
//!     println!("{}  {}B", file.server_filename, file.size);
//! }
//! ```
//!
//! ## 搜索文件(支持中文关键字,递归)
//!
//! ```no_run
//! use baiduyun_api::YunApi;
//!
//! let api = YunApi::new("你的access_token");
//! let results = api.search_with_key("唱戏机", "/", true, 1, 100, false).unwrap();
//! for item in results {
//!     println!("{} -> {}", item.server_filename, item.path);
//! }
//! ```
//!
//! ## 上传文件
//!
//! ```no_run
//! use baiduyun_api::{OnDup, YunApi};
//!
//! let api = YunApi::new("你的access_token");
//! // 注意: 上传路径必须位于 /apps/{你的应用名}/ 下(百度限制)
//! let result = api.upload("./photo.jpg", "/apps/myapp/photo.jpg", OnDup::Fail).unwrap();
//! println!("上传成功: {}", result.path);
//! ```
//!
//! # 获取 access_token
//!
//! 推荐使用授权工具(自动打开浏览器,粘贴地址栏 URL 即可自动提取写入 `.env`):
//!
//! ```text
//! cargo run --example authorize -- --app-key=你的APP_KEY
//! ```
//!
//! 或手动授权:浏览器访问
//! `https://openapi.baidu.com/oauth/2.0/authorize?response_type=token&client_id=你的APP_KEY&redirect_uri=oob&scope=netdisk`,
//! 授权后从地址栏 `...login_success#access_token=xxx...` 提取 token。
//!
//! token 有效期 30 天,持续使用不会过期。
//!
//! # 演示 CLI
//!
//! ```text
//! cargo run --example cli -- ls /
//! cargo run --example cli -- search 唱戏机
//! ```
//!
//! # API 稳定性
//!
//! 从 **0.3.0 开始 API 稳定**:之后只增加新接口,不会变动已有接口的签名和行为。

pub use error::ApiError;
pub use util::YunFs;
pub use yunapi::YunApi;

mod error;
mod models;
pub mod util;
mod yunapi;

pub use models::{
    DownloadOpts, FileId, FileInfo, FileInfoEx, FileInfoIter, FilePath, OnDup, QuotaInfo,
    SearchResult, Thumbs, UploadResult, UserInfo,
};

#[cfg(test)]
mod tests;
