//!提供实用工具
//!
//!一些方便开发的实用设施
//!包括:
//!- 单线程及多线程下载设施
//!- 单位转换之设施
//!- 目录结构之设施: [YunFs](YunFs) ,提供云端文件系统的抽象.

use super::ApiError;
use super::FileInfo;
use super::FileInfoIter;
use super::YunApi;
use std::path::Path;
use std::path::PathBuf;

use std::fmt::Display;
#[derive(Debug)]
pub struct UtilError {
    prompt: String,
}
impl UtilError {
    fn new(prompt: &str) -> UtilError {
        UtilError {
            prompt: String::from(prompt),
        }
    }
}

impl Display for UtilError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.prompt)
    }
}
/// 提供方便的容量大小转换
///
///返回是一个元组,从左往右依次是转换为KB,MB,GB的值,用浮点数表示
pub fn human_quota(in_quta: i64) -> (f64, f64, f64) {
    let tmp_quota = in_quta as f64;
    let k = 1024 as f64;
    let m = (1024 * 1024) as f64;
    let g = (1024 * 1024 * 1024) as f64;
    (tmp_quota / k, tmp_quota / m, tmp_quota / g)
}

/// 分片上传的md5计算器
///
/// 返回值是一个vec of string
pub fn calculate_file_md5_vec<P: AsRef<Path>>(path: P) -> Vec<String> {
    use std::fs::File;
    use std::io::Read;
    let mut f = File::open(path).unwrap();
    const SIZE: usize = 1024 * 1024 * 4;
    let mut buffer = vec![0; SIZE];
    let mut result: Vec<String> = vec![];
    loop {
        if let Ok(size) = f.read(&mut buffer[0..SIZE]) {
            let slice_md5 = md5::compute(&buffer)
                .into_iter()
                .map(|x| format!("{:02x}", x))
                .collect::<String>();
            result.push(slice_md5);
            if size < SIZE {
                break;
            }
        } else {
            panic!()
        }
    }
    return result;
}

/// 若输入一个有效的vip类型数字,返回一个相应的中文描述字符串
///
/// 会员类型，0普通用户、1普通会员、2超级会员
pub fn get_vip_type_str(vip_type: i64) -> Result<String, UtilError> {
    match vip_type {
        0 => Ok(String::from("普通用户")),
        1 => Ok(String::from("普通会员")),
        2 => Ok(String::from("超级会员")),
        _ => Err(UtilError::new("Not Support vip_type.")),
    }
}

///提供方便的云目录结构,方便进行各种目录切换操作
///
///
///YunFs是Api的高级抽象,需要创建一个YunFs结构体才能使用
///
///这个云目录模型模拟进行目录浏览，让我们浏览云端文件系统如同浏览本地文件系统一样
///
///暂时提供以下几种操作:
///- 返回当前路径[pwd()](YunFs::pwd())
///- 切换路径[chdir()](YunFs::chdir())
///- 列出指定路径的所有文件list
pub struct YunFs<'a> {
    api: &'a YunApi,
    current_path: PathBuf,
}
impl<'a> YunFs<'a> {
    ///创建一个YunFs结构体
    pub fn new(api_ref: &YunApi) -> YunFs {
        YunFs {
            api: api_ref,
            current_path: PathBuf::from("/"), // 总是以绝对路径的形式
        }
    }

    ///返回当前的目录
    ///
    ///注意:每次都检查当前目录是否存在
    ///
    pub fn pwd(&self) -> Result<String, UtilError> {
        let path_string: String = self.current_path.to_str().unwrap().into();
        if let Ok(_) = self.api.get_files_list(&path_string, 0, 0) {
            Ok(path_string.clone())
        } else {
            Err(UtilError::new(
                "Error when pwd():the directory may not exist.",
            ))
        }
    }
    fn check_dir_fmt(dir_str: &str) -> Result<(), UtilError> {
        // 以下这几种才是正确的目录形式
        // .[/]
        // ..[/]
        // ./dir1/dir2[/]
        // ../dir1/dir2[/]
        // /dir1/dir2/dir3[/]
        // dir1/dir2/dir3[/]

        /*下面进行路径检查,使用状态机*/
        let states = (0, 1, 2, 3, 4); //0-开始,1-开头遇到`.`,2-开头遇到`..`,3-遇到`/`,4-遇到其他字符
        let mut c_state = states.0;
        for item in dir_str.chars() {
            if item == '.' {
                if c_state == states.0 {
                    //开头是.
                    c_state = states.1;
                    continue;
                } else if c_state == states.1 {
                    //开头是..
                    c_state = states.2;
                    continue;
                } else {
                    //error.
                    return Err(UtilError::new(
                        "path resolve Error: `.` not the correct position.",
                    ));
                }
            }
            if item == '/' {
                if c_state != states.3 {
                    c_state = states.3;
                    continue;
                } else {
                    //error.
                    return Err(UtilError::new(
                        "path resolve Error: `/` not the correct position.",
                    ));
                }
            } else {
                //剩下的应该都是普通字符,特殊字符则报错
                match item {
                    '\\' => {
                        return Err(UtilError::new(
                            "path resolve Error: `\\` not the accepted char.",
                        ))
                    }
                    _ => {}
                }
                if c_state == 1 || c_state == 2 {
                    return Err(UtilError::new("`.` or `..`can not be here."));
                }
                c_state = states.4;
                continue;
            }
        }
        return Ok(());
    }
    fn resolve_path(&self, dir_str: &str) -> Result<String, UtilError> {
        //先检查是否符合路径规范
        match Self::check_dir_fmt(dir_str) {
            Ok(_) => {}
            Err(error) => {
                return Err(error);
            }
        }

        let mut tmp_dir = String::from(dir_str);
        if dir_str == "." || dir_str == "./" {
            return Ok(self.current_path.to_str().unwrap().into());
        }

        if dir_str == ".." || dir_str == "../" {
            if self.current_path.to_str().unwrap() == "/" {
                //特殊情况,已经是系统的根了就不应该再往上找了.
                return Ok("/".into());
            } else {
                let mut tmp_path = PathBuf::from(&self.current_path);
                tmp_path.pop();
                return Ok(tmp_path.to_str().unwrap().into());
            }
        }

        //去除可能有的最后的`/`
        if tmp_dir.len() != 1 && tmp_dir.ends_with("/") {
            tmp_dir.pop();
        }

        // ../ 开头,父目录查找
        if tmp_dir.starts_with("../") {
            tmp_dir.remove(0);
            tmp_dir.remove(0);
            tmp_dir.remove(0);

            if self.current_path.to_str().unwrap() == "/" {
                //特殊情况,已经是系统的根了就不应该再往上找了.
                let ret_string = format!("/{}", tmp_dir);
                return Ok(ret_string);
            } else {
                let mut tmp_path = PathBuf::from(&self.current_path);
                tmp_path.pop();
                return Ok(format!("{}/{}", tmp_path.to_str().unwrap(), tmp_dir));
            }
        }
        // 绝对目录查找
        if tmp_dir.starts_with("/") {
            return Ok(tmp_dir);
        }

        /*剩下的都是相对目录查找*/
        // ./开头,相对目录查找
        if tmp_dir.starts_with("./") {
            tmp_dir.remove(0);
            tmp_dir.remove(0);
        }
        if self.current_path.to_str().unwrap() == "/" {
            //要是不处理这种特殊情况会出现解析出来为 //dir1 的情况
            return Ok(format!("/{}", tmp_dir));
        }
        let ret_string = format!("{}/{}", self.current_path.to_str().unwrap(), tmp_dir);
        return Ok(ret_string);
    }
    ///切换当前目录
    ///
    ///该函数自动定为在线,即总是在切换目录时确认路径是否存在.
    ///
    ///输入的格式有许多种如:
    ///- ".[/]"
    ///- "..[/]"
    ///- "./dir1/dir2[/]"
    ///- "../dir1/dir2[/]"
    ///- "/dir1/dir2/dir3[/]"
    ///- "dir1/dir2/dir3[/]"
    pub fn chdir(&mut self, dir_str: &str) -> Result<(), UtilError> {
        //每一次目录变动都需要进行一次在线检查,检查失败则操作失败
        let resolved_result = self.resolve_path(dir_str);
        let dir_resolved = match resolved_result {
            Ok(dir_path) => dir_path,
            Err(error) => return Err(error),
        };
        //debug;;; println!("resolved:path {}",dir_resolved);
        if let Ok(_) = self.api.get_files_list(&dir_resolved, 0, 0) {
            //将本地表示也改变为目录切换后的版本
            self.current_path = PathBuf::from(dir_resolved);
            Ok(())
        } else {
            Err(UtilError::new("Error:chdir():the directory may not exist."))
        }
    }

    ///列出当前目录的所有文件
    ///
    ///这个函数一次网络请求最多得到1000个文件,如果超过1000则需要发起多次网络请求，速度就会变慢.
    pub fn ls(&self) -> Result<FileInfoIter, ApiError> {
        //将所有的文件都列出来
        let list_len = 1000;
        let mut ret_vec: Vec<FileInfo> = Vec::new();
        loop {
            let tmp_list =
                match self
                    .api
                    .get_files_list(self.current_path.to_str().unwrap(), 0, list_len)
                {
                    Ok(list) => list,
                    Err(error) => {
                        return Err(error);
                    }
                };
            let mut tmp_vec: Vec<FileInfo> = tmp_list.collect();
            let len = tmp_vec.len();
            ret_vec.append(&mut tmp_vec);
            if len < 1000 {
                break;
            }
        }
        Ok(FileInfoIter::new(ret_vec))
    }
}

use reqwest::blocking::Client;
use reqwest::header::CONTENT_LENGTH;
use reqwest::header::RANGE;
use reqwest::header::USER_AGENT;
//use reqwest::header::CONTENT_RANGE;
use std::fs::OpenOptions;
use std::io::Write;

///下载文件到指定的位置
///
///其中参数url,是你获取的下载链接,access_token是用户token,dst下载下来的文件在文件系统中的位置
///block_size用于分段下载，若值为0则不进行分段,若值不为0则以MB为单位进行分段
///如果is_debug:设为true则会有简单的调试信息类似下面这样:
///
///```
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
pub fn download(url: &str, dst: &str, block_size: i32, access_token: &str, is_debug: bool) {
    let mut has_downloaded: i64 = 0;
    let size: i32 = 1024 * 1024 * block_size; //每个range1MB大小,100MB
    let downloader = Client::new();
    let download_url = format!("{}&access_token={}", url, access_token);
    let mut file_to_store = OpenOptions::new()
        .append(true)
        .create(true)
        .write(true)
        .open(dst)
        .unwrap();
    if size == 0 {
        let response = downloader
            .get(&download_url)
            .header(USER_AGENT, "pan.baidu.com")
            .send()
            .unwrap();
        file_to_store
            .write_all(&(response.bytes().unwrap()))
            .unwrap();
        return;
    }
    let mut range_head = 0;
    let mut range = format!("bytes={}-{}", range_head, range_head + size - 1);
    loop {
        let requestbuild = downloader
            .get(&download_url)
            .header(USER_AGENT, "pan.baidu.com")
            .header(RANGE, &range);
        let response = requestbuild.send().unwrap();
        let len_rev = response
            .headers()
            .get(CONTENT_LENGTH)
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
            .write_all(&(response.bytes().unwrap()))
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

            range_head = range_head + size;
            range = format!("bytes={}-{}", range_head, range_head + size - 1);
            //println!("{} ====> {}",range,len_rev);
            //println!("contine get next!");
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn test_md5_util() {
        let r = calculate_file_md5_vec("D:/test.pdf");
        println!("{:?}", r);
    }
}
