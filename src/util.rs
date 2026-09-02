//!提供实用工具
//!
//!一些方便开发的实用设施
//!包括:
//!- 单线程及多线程下载设施
//!- 单位转换之设施
//!- 目录结构之设施: [YunFs],提供云端文件系统的抽象
//!
//!所有错误处理统一使用 [ApiError]

use super::ApiError;
use super::FileInfo;
use super::FileInfoIter;
use super::OnDup;
use super::YunApi;
use std::path::{Component, Path, PathBuf};

/// 提供方便的容量大小转换
///
///返回是一个元组,从左往右依次是转换为KB,MB,GB的值,用浮点数表示
pub fn human_quota(in_quta: i64) -> (f64, f64, f64) {
    let tmp_quota = in_quta as f64;
    let k = 1024_f64;
    let m = (1024 * 1024) as f64;
    let g = (1024 * 1024 * 1024) as f64;
    (tmp_quota / k, tmp_quota / m, tmp_quota / g)
}

/// 若输入一个有效的vip类型数字,返回一个相应的中文描述字符串
///
/// 会员类型，0普通用户、1普通会员、2超级会员
pub fn get_vip_type_str(vip_type: i64) -> Result<String, ApiError> {
    match vip_type {
        0 => Ok(String::from("普通用户")),
        1 => Ok(String::from("普通会员")),
        2 => Ok(String::from("超级会员")),
        _ => Err(ApiError::from("Not Support vip_type.")),
    }
}

///提供方便的云目录结构,方便进行各种目录切换操作
///
///
///YunFs是Api的高级抽象,需要创建一个YunFs结构体才能使用
///
///这个云目录模型模拟进行目录浏览，让我们浏览云端文件系统如同浏览本地文件系统一样
///
///提供以下几种操作:
///- 返回当前路径[pwd()](YunFs::pwd())
///- 切换路径[chdir()](YunFs::chdir())
///- 列出当前目录的所有文件[ls()](YunFs::ls())
///- 创建目录[mkdir()](YunFs::mkdir())
///- 删除文件/目录[rm()](YunFs::rm())
///- 移动[mv()](YunFs::mv())
///- 复制[cp()](YunFs::cp())
///- 上传[upload()](YunFs::upload())
///
///所有操作返回 [ApiError] 类型的错误
pub struct YunFs<'a> {
    api: &'a YunApi,
    current_path: PathBuf,
}
impl<'a> YunFs<'a> {
    ///创建一个YunFs结构体
    pub fn new(api_ref: &'a YunApi) -> YunFs<'a> {
        YunFs {
            api: api_ref,
            current_path: PathBuf::from("/"), // 总是以绝对路径的形式
        }
    }

    ///返回当前的目录(本地缓存状态,不发网络请求,永不失败)
    ///
    ///注意:不校验云端目录是否仍然存在;目录被外部删除时,
    ///后续 [ls](YunFs::ls) 等操作才会报错(与本地 shell 语义一致)
    ///
    pub fn pwd(&self) -> String {
        self.current_path.to_str().unwrap().into()
    }
    fn check_dir_fmt(dir_str: &str) -> Result<(), ApiError> {
        // 网盘路径是 Unix 风格: 反斜杠(Windows 分隔符)一律拒绝
        if dir_str.contains('\\') {
            return Err(ApiError::from(
                "path resolve Error: `\\` not the accepted char.",
            ));
        }
        // 连续 // 会被 components 折叠,需显式拒绝
        if dir_str.contains("//") {
            return Err(ApiError::from(
                "path resolve Error: `/` not the correct position.",
            ));
        }
        for comp in Path::new(dir_str).components() {
            match comp {
                Component::Normal(seg) => {
                    // 段以 . 开头但非 . 或 ..(.a、..a 非法;那两种是 CurDir/ParentDir)
                    if seg.to_str().unwrap_or_default().starts_with('.') {
                        return Err(ApiError::from(
                            "path resolve Error: `.` or `..` can not be here.",
                        ));
                    }
                }
                Component::CurDir | Component::ParentDir | Component::RootDir => {}
                Component::Prefix(_) => {
                    // Windows 盘符前缀(如 C:\)
                    return Err(ApiError::from(
                        "path resolve Error: windows prefix not accepted.",
                    ));
                }
            }
        }
        Ok(())
    }
    fn resolve_path(&self, dir_str: &str) -> Result<String, ApiError> {
        Self::check_dir_fmt(dir_str)?;

        // 段表初始化: 绝对路径从根(空表)开始,相对路径从当前路径的段开始
        let mut segments: Vec<String> =
            if Path::new(dir_str).components().next() == Some(Component::RootDir) {
                Vec::new()
            } else {
                self.current_path
                    .to_str()
                    .unwrap()
                    .split('/')
                    .filter(|s| !s.is_empty())
                    .map(|s| s.to_string())
                    .collect()
            };

        // 按 components 语义规约: Normal 入表,ParentDir 出表(到根自然停止)
        for comp in Path::new(dir_str).components() {
            match comp {
                Component::RootDir | Component::CurDir => {}
                Component::ParentDir => {
                    segments.pop();
                }
                Component::Normal(seg) => segments.push(seg.to_str().unwrap_or_default().to_string()),
                Component::Prefix(_) => {
                    return Err(ApiError::from(
                        "path resolve Error: windows prefix not accepted.",
                    ));
                }
            }
        }

        // 空表 = 根目录;否则补上前导 / 构成绝对路径
        Ok(format!("/{}", segments.join("/")))
    }
    ///切换当前目录
    ///
    ///切换时在线确认目标目录存在,不存在则操作失败
    ///
    ///输入的格式有许多种如:
    ///- ".[/]"
    ///- "..[/]"
    ///- "./dir1/dir2[/]"
    ///- "../dir1/dir2[/]"
    ///- "/dir1/dir2/dir3[/]"
    ///- "dir1/dir2/dir3[/]"
    pub fn chdir(&mut self, dir_str: &str) -> Result<(), ApiError> {
        //每一次目录变动都需要进行一次在线检查,检查失败则操作失败
        let resolved_result = self.resolve_path(dir_str);
        let dir_resolved = resolved_result?;
        //debug;;; println!("resolved:path {}",dir_resolved);
        if self.api.get_files_list(&dir_resolved, 0, 0).is_ok() {
            //将本地表示也改变为目录切换后的版本
            self.current_path = PathBuf::from(dir_resolved);
            Ok(())
        } else {
            Err(ApiError::from("Error:chdir():the directory may not exist."))
        }
    }

    ///列出当前目录的所有文件
    ///
    ///这个函数一次网络请求最多得到1000个文件,如果超过1000则需要发起多次网络请求,速度就会变慢.
    pub fn ls(&self) -> Result<FileInfoIter, ApiError> {
        //将所有的文件都列出来
        let list_len = 1000;
        let mut ret_vec: Vec<FileInfo> = Vec::new();
        let mut start = 0;
        loop {
            let tmp_list =
                match self
                    .api
                    .get_files_list(self.current_path.to_str().unwrap(), start, list_len)
                {
                    Ok(list) => list,
                    Err(error) => {
                        return Err(error);
                    }
                };
            let mut tmp_vec: Vec<FileInfo> = tmp_list.collect();
            let len = tmp_vec.len();
            ret_vec.append(&mut tmp_vec);
            if len < list_len as usize {
                break;
            }
            start += list_len;
        }
        Ok(FileInfoIter::new(ret_vec))
    }

    ///创建目录(支持相对/绝对路径,与 [chdir](YunFs::chdir) 相同的路径解析规则)
    ///
    ///路径已存在时返回错误(errno=-8)
    pub fn mkdir(&mut self, dir_str: &str) -> Result<(), ApiError> {
        let resolved = self.resolve_path(dir_str)?;
        self.api.mkdir(&resolved)
    }

    ///删除文件/目录(支持相对/绝对路径)
    ///
    ///注意:百度对不存在的文件静默成功
    pub fn rm(&mut self, path: &str) -> Result<(), ApiError> {
        let resolved = self.resolve_path(path)?;
        self.api.remove(&[resolved])
    }

    ///移动文件/目录到目标目录(支持相对/绝对路径)
    pub fn mv(&mut self, from: &str, to_dir: &str) -> Result<(), ApiError> {
        let from_resolved = self.resolve_path(from)?;
        let to_resolved = self.resolve_path(to_dir)?;
        self.api.mv(from_resolved, &to_resolved)
    }

    ///复制文件/目录到目标目录(支持相对/绝对路径)
    pub fn cp(&mut self, from: &str, to_dir: &str) -> Result<(), ApiError> {
        let from_resolved = self.resolve_path(from)?;
        let to_resolved = self.resolve_path(to_dir)?;
        self.api.cp(from_resolved, &to_resolved)
    }

    ///上传本地文件到当前目录(与 [download] 对称)
    ///
    ///- `local_path` 本地文件路径
    ///- `file_name` 上传后的文件名(可含子目录,经路径解析)
    ///
    ///注意:上传接口要求目标位于 `/apps/{自己的应用名}/` 下
    pub fn upload(&mut self, local_path: &str, file_name: &str) -> Result<(), ApiError> {
        let remote = self.resolve_path(file_name)?;
        self.api.upload(local_path, &remote, OnDup::Fail).map(|_| ())
    }
}

use std::fs::OpenOptions;
use std::io::Write;

///下载文件到指定的位置
///
///其中参数url,是你获取的下载链接,access_token是用户token,dst下载下来的文件在文件系统中的位置
///block_size用于分段下载，若值为0则不进行分段,若值不为0则以MB为单位进行分段
///如果is_debug:设为true则会有简单的调试信息类似下面这样:
///
///```text
///recieve data total 20 MB
///recieve data total 40 MB
///recieve data total 60 MB
///recieve data total 80 MB
///recieve data total 100 MB
///recieve data total 120 MB
///recieve data total 140 MB
///recieve data total 160 MB
///recieve data total 161 MB
///finish download.
///```
///# 已废弃
///
/// 请使用 [crate::YunApi::download](crate::YunApi::download)(token 内部持有,流式落盘,零 panic)
/// 或 [YunFs::download](YunFs::download)(YunFs 内直接按文件名下载)。
/// 本函数保留仅为 0.3.x 兼容,存在 panic 风险与追加写入问题。
#[deprecated(note = "请使用 YunApi::download / YunFs::download(见 docs/refactor-download.md)")]
pub fn download(url: &str, dst: &str, block_size: i32, access_token: &str, is_debug: bool) {
    let mut has_downloaded: i64 = 0;
    let size: i32 = 1024 * 1024 * block_size; //每个range1MB大小,100MB
    let config = ureq::Agent::config_builder()
        .http_status_as_error(false)
        .build();
    let downloader: ureq::Agent = config.into();
    let download_url = format!("{}&access_token={}", url, access_token);
    let mut file_to_store = OpenOptions::new()
        .append(true)
        .create(true)
        .open(dst)
        .unwrap();
    if size == 0 {
        let mut response = downloader
            .get(&download_url)
            .header("User-Agent", "pan.baidu.com")
            .call()
            .unwrap();
        file_to_store
            .write_all(&response.body_mut().read_to_vec().unwrap())
            .unwrap();
        return;
    }
    let mut range_head = 0;
    let mut range = format!("bytes={}-{}", range_head, range_head + size - 1);
    loop {
        let mut response = downloader
            .get(&download_url)
            .header("User-Agent", "pan.baidu.com")
            .header("Range", &range)
            .call()
            .unwrap();
        let len_rev = response
            .headers()
            .get("content-length")
            .unwrap()
            .to_str()
            .unwrap()
            .parse::<i32>()
            .unwrap();
        if is_debug {
            has_downloaded += human_quota(len_rev as i64).1 as i64;
            println!("recieve data total {} MB", has_downloaded);
        }
        file_to_store
            .write_all(&response.body_mut().read_to_vec().unwrap())
            .unwrap();
        //println!("{}",content_range);
        //不再需要再请求了
        if len_rev < size {
            if is_debug {
                println!("finish download.");
            }
            break;
        } else {
            //需要请求下一段

            range_head += size;
            range = format!("bytes={}-{}", range_head, range_head + size - 1);
            //println!("{} ====> {}",range,len_rev);
            //println!("contine get next!");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_human_quota() {
        let (kb, mb, gb) = human_quota(1024);
        assert_eq!(kb, 1.0);
        assert_eq!(mb, 0.0009765625);
        assert_eq!(gb, 9.5367431640625e-7);
    }

    #[test]
    fn test_human_quota_large() {
        let (kb, mb, gb) = human_quota(1024 * 1024 * 1024);
        assert_eq!(kb, 1048576.0);
        assert_eq!(mb, 1024.0);
        assert_eq!(gb, 1.0);
    }

    #[test]
    fn test_get_vip_type_str_normal_user() {
        let result = get_vip_type_str(0);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "普通用户");
    }

    #[test]
    fn test_get_vip_type_str_normal_member() {
        let result = get_vip_type_str(1);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "普通会员");
    }

    #[test]
    fn test_get_vip_type_str_super_member() {
        let result = get_vip_type_str(2);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "超级会员");
    }

    #[test]
    fn test_get_vip_type_str_invalid() {
        let result = get_vip_type_str(999);
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert_eq!(error.ret_errno(), 8989);
        let display = format!("{}", error);
        assert!(display.contains("Not Support vip_type."));
    }

    /// 构造一个 current_path 为指定路径的 YunFs(测试辅助,不涉及网络)
    fn make_fs_at<'a>(api: &'a YunApi, path: &str) -> YunFs<'a> {
        let mut fs = YunFs::new(api);
        fs.current_path = PathBuf::from(path);
        fs
    }

    #[test]
    fn test_yunfs_resolve_current_and_parent() {
        let api = YunApi::new("test_token");
        let fs = make_fs_at(&api, "/apps/bypy");
        assert_eq!(fs.resolve_path(".").unwrap(), "/apps/bypy");
        assert_eq!(fs.resolve_path("./").unwrap(), "/apps/bypy");
        assert_eq!(fs.resolve_path("..").unwrap(), "/apps");
        assert_eq!(fs.resolve_path("../").unwrap(), "/apps");
        assert_eq!(fs.resolve_path("../..").unwrap(), "/");
    }

    #[test]
    fn test_yunfs_resolve_parent_beyond_root() {
        // 越界父目录应停在根,不产生 /.. 之类
        let api = YunApi::new("test_token");
        let fs = make_fs_at(&api, "/apps/bypy");
        assert_eq!(fs.resolve_path("../../..").unwrap(), "/");
        let fs_root = make_fs_at(&api, "/");
        assert_eq!(fs_root.resolve_path("..").unwrap(), "/");
        assert_eq!(fs_root.resolve_path("../..").unwrap(), "/");
    }

    #[test]
    fn test_yunfs_resolve_relative() {
        let api = YunApi::new("test_token");
        let fs = make_fs_at(&api, "/apps/bypy");
        assert_eq!(fs.resolve_path("./dir1/dir2").unwrap(), "/apps/bypy/dir1/dir2");
        assert_eq!(fs.resolve_path("../dir1").unwrap(), "/apps/dir1");
        assert_eq!(fs.resolve_path("dir1/dir2").unwrap(), "/apps/bypy/dir1/dir2");
        assert_eq!(fs.resolve_path("dir1/").unwrap(), "/apps/bypy/dir1");
        assert_eq!(fs.resolve_path("src.txt").unwrap(), "/apps/bypy/src.txt");
        assert_eq!(fs.resolve_path("a/../b").unwrap(), "/apps/bypy/b");
    }

    #[test]
    fn test_yunfs_resolve_absolute() {
        let api = YunApi::new("test_token");
        let fs = make_fs_at(&api, "/apps/bypy");
        assert_eq!(fs.resolve_path("/dir1/dir2").unwrap(), "/dir1/dir2");
        assert_eq!(fs.resolve_path("/").unwrap(), "/");
        let fs_root = make_fs_at(&api, "/");
        assert_eq!(fs_root.resolve_path("dir1").unwrap(), "/dir1");
        assert_eq!(fs_root.resolve_path("../dir1").unwrap(), "/dir1");
    }

    #[test]
    fn test_yunfs_check_dir_fmt_reject_windows_prefix() {
        // Windows 盘符前缀(C:)被拒绝(网盘路径是 Unix 风格)
        #[cfg(windows)]
        {
            let result = YunFs::check_dir_fmt("C:/dir1");
            assert!(result.is_err());
        }
        // 非 Windows 平台无 Prefix 概念,C:/dir1 是普通段,跳过断言
    }

    #[test]
    fn test_yunfs_resolve_parent_and_dir_mixed() {
        let api = YunApi::new("test_token");
        let fs = make_fs_at(&api, "/apps/bypy");
        assert_eq!(fs.resolve_path("../../dir1").unwrap(), "/dir1");
        assert_eq!(fs.resolve_path("../../a.b/dir2").unwrap(), "/a.b/dir2");
    }

    #[test]
    fn test_yunfs_resolve_chinese_path() {
        // 网盘场景中文路径是核心用例(如 chdir("学习资料/"))
        let api = YunApi::new("test_token");
        let fs = make_fs_at(&api, "/apps/bypy");
        assert_eq!(fs.resolve_path("学习资料").unwrap(), "/apps/bypy/学习资料");
        assert_eq!(
            fs.resolve_path("学习资料/唱戏机").unwrap(),
            "/apps/bypy/学习资料/唱戏机"
        );
        assert_eq!(fs.resolve_path("../学习资料").unwrap(), "/apps/学习资料");
    }

    #[test]
    fn test_yunfs_resolve_invalid() {
        let api = YunApi::new("test_token");
        let fs = make_fs_at(&api, "/apps/bypy");
        assert!(fs.resolve_path("..a").is_err());
        assert!(fs.resolve_path("dir1\\dir2").is_err());
        assert!(fs.resolve_path("//dir1").is_err());
        assert!(fs.resolve_path(".a").is_err());
    }

    #[test]
    fn test_yunfs_pwd_is_pure_local() {
        // pwd 是本地状态查询: 不依赖网络、永不失败,直接返回缓存路径
        let api = YunApi::new("test_token");
        let fs = YunFs::new(&api);
        assert_eq!(fs.pwd(), "/");
    }

    #[test]
    fn test_yunfs_check_dir_fmt_valid_absolute() {
        let result = YunFs::check_dir_fmt("/dir1/dir2");
        assert!(result.is_ok());
    }

    #[test]
    fn test_yunfs_check_dir_fmt_valid_relative() {
        let result = YunFs::check_dir_fmt("./dir1/dir2");
        assert!(result.is_ok());
    }

    #[test]
    fn test_yunfs_check_dir_fmt_valid_parent() {
        let result = YunFs::check_dir_fmt("../dir1");
        assert!(result.is_ok());
    }

    #[test]
    fn test_yunfs_check_dir_fmt_current() {
        let result = YunFs::check_dir_fmt(".");
        assert!(result.is_ok());
    }

    #[test]
    fn test_yunfs_check_dir_fmt_parent_dir() {
        let result = YunFs::check_dir_fmt("..");
        assert!(result.is_ok());
    }

    #[test]
    fn test_yunfs_check_dir_fmt_invalid_backslash() {
        let result = YunFs::check_dir_fmt("dir1\\dir2");
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert_eq!(error.ret_errno(), 8989);
        let display = format!("{}", error);
        assert!(display.contains("path resolve Error"));
        assert!(display.contains("\\"));
    }

    #[test]
    fn test_yunfs_check_dir_fmt_invalid_double_slash() {
        let result = YunFs::check_dir_fmt("//dir1");
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert_eq!(error.ret_errno(), 8989);
        let display = format!("{}", error);
        assert!(display.contains("path resolve Error"));
        assert!(display.contains("/"));
    }

    #[test]
    fn test_yunfs_check_dir_fmt_dot_in_filename() {
        // 段中间的 `.` 是普通字符(如 src.txt、a.b.c.txt),应合法
        let result = YunFs::check_dir_fmt("dir1./dir2");
        assert!(result.is_ok());
        let result = YunFs::check_dir_fmt("src.txt");
        assert!(result.is_ok());
        let result = YunFs::check_dir_fmt("a.b.c.txt");
        assert!(result.is_ok());
        let result = YunFs::check_dir_fmt("/dir/sub/file.v1.2.zip");
        assert!(result.is_ok());
    }

    #[test]
    fn test_yunfs_check_dir_fmt_invalid_dot_start_mid() {
        // 段开头的 `.` 只允许 . 和 ..;`..a` 这类应非法
        let result = YunFs::check_dir_fmt("..a");
        assert!(result.is_err());
    }

    #[test]
    fn test_yunfs_check_dir_fmt_trailing_slash() {
        let result = YunFs::check_dir_fmt("/dir1/");
        assert!(result.is_ok());
    }

    #[test]
    fn test_yunfs_check_dir_fmt_simple() {
        let result = YunFs::check_dir_fmt("dir1");
        assert!(result.is_ok());
    }
}
