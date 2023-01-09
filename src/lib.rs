//! 这是一个rust写成的百度云api库, **不提供**作弊功能!
//!
//!
//!# 一,简介
//!这个库提供方便地使用百度云官方api的方法
//!
//! 对用户的云盘进行访问前首先要获取access_token,具体请看官网的[这里](https://pan.baidu.com/union/document/entrance#%E6%8E%A5%E5%85%A5%E6%B5%81%E7%A8%8B)
//!
//! ⚠️access_token获取的方法的简要描述:
//!
//! 最开始你登录百度账号并且创建一个云盘app,获取它的APP_key
//!
//! 在浏览器地址栏输入如下内容 其中"你的APP KEY"替换成你的APP Key
//!
//! ``` https://openapi.baidu.com/oauth/2.0/authorize?response_type=token&client_id=你的APP KEY&redirect_uri=oob&scope=netdisk```
//!
//! 然后点击授权后,会跳转到另外的一个空白网页上,此时查看地址栏上的地址大概是这样的样子:
//!
//! ```http://openapi.baidu.com/oauth/2.0/login_success#expires_in=2592000&access_token={access_token}&session_secret={session_secret}&session_key={session_key}&scope=basic+netdisk```
//!
//! 其中access_token后面一段是我们需要的,保存下来即可
//!
//! 使用期限是30天,但如果这个access_token一直在使用的话 是不会过期的,过期需要重新查询.
//!
//!**注意:本库不提供作弊功能!!!**
//!# 二,功能演示
//!## 1.列出用户信息
//!下面是示例如何列出用户信息:
//!```
//!use baiduyun_api::YunApi;
//!//...
//!//--snip--
//!//...
//!let api = YunApi::new();
//!let access_token ="User's access_token";
//!let user_info = api.get_user_info().unwrap();
//!    println!("baidu_name :{}", user_info.baidu_name);
//!    println!("vip :{}", user_info.vip_type);
//!```
//!
//!## 2.列出云盘信息
//!列出云盘的存储空间信息的实例如下:
//!```
//!use baiduyun_api::YunApi;
//!//...
//!//--snip--
//!//...
//!let api = YunApi::new();
//!let access_token ="User's access_token";
//!let quota_info = api.get_quota_info().unwrap();
//!println!("总空间 :{}", quota_info.total);
//!println!("剩余空间 :{}", quota_info.free);
//!```
//!
//!
//!
//!
//!## 3.使用util设施
//!我编写了一些基础设施帮助你开发自己的程序,先看看[YunFs](util::YunFs)如何使用:
//!```
//!use baiduyun_api::YunApi;
//!use baiduyun_api::util
//!
//!//...
//!//--snip--
//!//...
//!let access_token ="User's access_token.";
//!let api = YunApi::new(access_token);
//!let mut my_fs = util::YunFs::new(&api);
//!println!("current dir:====>{}",my_fs.pwd().unwrap());
//!my_fs.chdir("../").unwrap();
//!my_fs.chdir("/apps").unwrap();
//!my_fs.chdir("../").unwrap();
//!my_fs.chdir("/apps/").unwrap();
//!my_fs.chdir("../").unwrap();
//!my_fs.chdir("./apps/bypy/唱戏机").unwrap();
//!let tmp_list = my_fs.ls().unwrap();
//!for item in tmp_list{
//!    println!("filename:{};filesize={}KB",item.server_filename,util::human_quota(item.size).0)
//!}
//!```
//!结果为:  
//!```
//! current dir:====>/
//! filename:45部高清黄梅戏mp4;filesize=0KB
//! filename:黄梅戏视频;filesize=0KB
//! filename:庐剧视频标清3;filesize=0KB
//! filename:庐剧视频高清1;filesize=0KB
//! filename:庐剧视频高清2;filesize=0KB
//! filename:庐剧视频合集;filesize=0KB
//! filename:相声小品大杂烩290部视频;filesize=0KB
//!```
//!再看看一个简陋的单线程下载设施[download](util::download):
//!```
//!fn download_test() {
//!        let key =
//!            "your_access_key_to_user.";
//!        let api = YunApi::new(key);
//!        let mut myfs = util::YunFs::new(&api);
//!        println!("current dir ===> {}", myfs.pwd().unwrap());
//!        myfs.chdir("学习资料/").unwrap();
//!        println!("current dir ===> {}", myfs.pwd().unwrap());
//!        let files = myfs.ls().unwrap();
//!        let mut file_to_download: FilePtr = FilePtr::default();
//!        for item in files {
//!            if item.server_filename.contains("中文第六版@www.java1234.com.pdf") {
//!                println!("pdf: -> {}; id ={} ", item.server_filename, item.fs_id);
//!                file_to_download = item;
//!            }
//!        }
//!        let link = api.get_file_dlink(file_to_download).unwrap();
//!        util::download(&link, "D:/test.pdf", key, true);//这里打开了debug输出
//!    }
//!
//!```
//!结果如下:
//!```
//!current dir ===> /
//!current dir ===> /学习资料
//!pdf: -> 数据库系统概念_中文第六版@www.java1234.com.pdf; id =816997609436448
//!recieve data total 20 MB
//!recieve data total 40 MB
//!recieve data total 60 MB
//!recieve data total 80 MB
//!recieve data total 100 MB
//!recieve data total 120 MB
//!recieve data total 140 MB
//!recieve data total 160 MB
//!recieve data total 161 MB
//!finish download.
//!```

use serde::{Deserialize, Serialize};

mod error;
pub mod util;
mod yunapi;
pub use error::ApiError;
pub use yunapi::YunApi;

///用户信息结构体,由[YunApi::get_user_info()]返回
///
///包含了以下五个字段
///- baidu_name,百度账号名
///- netdisk_name,网盘账号名
///- avatar_url,头像图片url
///- vip_type,vip的类型
///- uk,用户id
#[derive(Serialize, Deserialize)]
pub struct UserInfo {
    pub baidu_name: String,
    pub netdisk_name: String,
    pub avatar_url: String, // 头像的url
    pub vip_type: i64,      //vip 类型
    pub uk: i64,            //用户id
}

///配额信息结构体,由[YunApi::get_quota_info()]返回
///
///包含了以下四个字段
///- total,总空间大小
///- expire,7天内是否由容量到期
///- used,已使用大小,单位B
///- free,剩余大小,单位B
///
///要想要方便的进行单位转换参看[这个函数](util::human_quota())
#[derive(Serialize, Deserialize)]
pub struct QuotaInfo {
    pub total: i64,
    pub expire: bool,
    pub used: i64,
    pub free: i64,
}

///文件信息结构体
///
///包含了以下字段：
///- path,文件的绝对路径
///- category,文件类型:1 视频、2 音频、3 图片、4 文档、5 应用、6 其他、7 种子
///- fs_id,文件在云端的唯一标识ID
///- isdir,是否目录，0 文件、1 目录
///- local_ctime,文件在客户端创建时间
///- local_mtime,文件在客户端修改时间
///- server_ctime,文件在服务器创建时间
///- server_mtime,文件在服务器修改时间
///- server_filename,文件名称
///- md5,文件的md5值，只有是文件类型时，该KEY才存在
///- size,文件大小,单位B,要想要方便的进行单位转换参看[这个函数](util::human_quota())
///- thumbs,只有请求参数带WEB且该条目分类为图片时，该KEY才存在，包含三个尺寸的缩略图URL
///- dir_empty,该目录是否存在子目录,只有请求参数带WEB且该条目为目录时,该KEY才存在,0为存在,1为不存在
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct FileInfo {
    pub path: String,
    pub category: i64,
    pub fs_id: i64,
    pub isdir: i64,
    pub local_ctime: i64,
    pub local_mtime: i64,
    pub server_ctime: i64,
    pub server_mtime: i64,
    pub server_filename: String,
    pub md5: Option<String>,
    pub size: i64,
    pub thumbs: Option<String>,
    pub dir_empty: Option<i64>,
}
impl FileId for FileInfo {
    fn ret_file_id(&self) -> i64 {
        return self.fs_id;
    }
}

///拓展的文件信息结构体,由get_file_info返回.
///
///包含了以下字段：
///- category,文件类型:1 视频、2 音频、3 图片、4 文档、5 应用、6 其他、7 种子
///- dlink,文件的下载链接.
///- file_name,文件名.
///- isdir,是否目录，0 文件、1 目录
///- server_ctime,文件在服务器创建时间
///- server_mtime,文件在服务器修改时间
///- size,文件大小,单位B,要想要方便的进行单位转换参看[这个函数](util::human_quota())
/// 下面几个是文件类型为图片才有效:
///- height 图片高度.
///- width 图片宽度.
///- date_taken 图片的拍摄时间.
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct FileInfoEx {
    pub category: i64,
    pub dlink: Option<String>,
    pub file_name: Option<String>,
    pub is_dir: Option<i64>,
    pub server_ctime: i64,
    pub server_mtime: i64,
    pub size: i64,
    //pub thumbs:
    pub height: Option<i64>,
    pub width: Option<i64>,
    pub date_taken: Option<i64>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct SearchResult {
    pub category: i64,
    pub fs_id: i64,
    pub isdir: i64,
    pub local_ctime: i64,
    pub local_mtime: i64,
    pub server_ctime: i64,
    pub server_mtime: i64,
    pub md5: Option<String>,
    pub size: i64,
    pub thumbs: Option<String>,
}

impl FileId for SearchResult {
    fn ret_file_id(&self) -> i64 {
        return self.fs_id;
    }
}

/// 预上传时的返回值
///
/// - path: 文件的绝对路径
/// - uploadid: 上传id
/// - return_type: 返回类型，1 文件在云端不存在、2 文件在云端已存在
/// - block_list: 需要上传的分片序号，索引从0开始
#[derive(Serialize, Deserialize)]
pub struct PreUploadRet {
    pub path: String,
    pub uploadid: String,
    pub return_type: u8,
    pub block_list: Vec<u32>,
}

///Upload接口的返回值
///
#[derive(Serialize, Deserialize)]
pub struct UploadRet {
    pub md5: String,
}

/// Create接口的返回值
///
#[derive(Deserialize, Serialize)]
pub struct CreateRet {
    fs_id: u64,
    md5: String,
    server_filename: String,
    category: u8,
    path: String,
    size: usize,
    ctime: u64,
    mtime: u64,
    isdir: u8,
}
/// 上传时的重命名策略
///
/// - NoRename 不重命名,返回冲突
/// - Rename 只要path冲突就进行重命名
/// - RenameStrict 当path和block_list都不同时才进行重命名
/// - Override 直接覆盖
pub enum UploadNameStrategy {
    NoRename = 0,
    Rename = 1,
    RenameStrict = 2,
    Override = 3,
}
/// [FileInfo] 的迭代器,可被clone.
#[derive(Clone)]
pub struct FileInfoIter {
    inner_data: Vec<FileInfo>,
    inner_count: usize,
}
impl FileInfoIter {
    /// 从Vec创建一个迭代器.
    pub fn new(in_vec: Vec<FileInfo>) -> FileInfoIter {
        FileInfoIter {
            inner_data: in_vec,
            inner_count: 0,
        }
    }
}

pub trait FileId {
    fn ret_file_id(&self) -> i64 {
        0_i64
    }
}

impl Iterator for FileInfoIter {
    type Item = FileInfo;
    fn next(&mut self) -> Option<Self::Item> {
        if self.inner_count >= self.inner_data.len() {
            return None;
        } else {
            let tmp = Some((&self).inner_data[self.inner_count].clone());
            self.inner_count += 1;
            return tmp;
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::read_to_string;
    #[test]
    fn test_api() {
        // load key form file to prevent key to reveal.
        let key = read_to_string("D:\\rust\\baiduyun_space\\baiduyun_api\\key.txt").unwrap();
        let api = YunApi::new(&key);
        let list = api.get_files_list("/", 0, 10).unwrap();
        let list_vec: Vec<FileInfo> = list.collect();
        assert!(list_vec.len() == 10);
        println!("list len = {}", list_vec.len());
        let infos = api.get_files_info(&list_vec).unwrap();
        for item in infos {
            println!("time = {}", item.server_ctime);
        }
    }

    #[test]
    #[should_panic]
    fn error_key() {
        let key = "++++123.64295f7207e0dcc4612276a7955e11f9.YaWhelqaKCPDHKxghpjx7shiRLRS44h1gcl4t7-.ckQMUQ";
        let api = YunApi::new(key);
        api.get_files_list("/", 0, 10).unwrap();
    }

    #[test]
    fn test_search() {
        let key = read_to_string("D:\\rust\\baiduyun_space\\baiduyun_api\\key.txt").unwrap();
        let api = YunApi::new(&key);
        let r = api
            .search_with_key("唱戏机", "/", true, 1, 100, false)
            .unwrap();
        for item in r {
            println!("item = {}", item.fs_id);
        }
    }

    #[test]
    fn test_serde_url() {
        extern crate serde_qs;

        #[derive(Serialize, Deserialize)]
        struct FormatStruct {
            value: u8,
            data: Vec<String>,
            flag: bool,
        }

        let test_data = FormatStruct {
            value: 8,
            data: vec!["hello".to_owned(), "thanks".to_owned(), "haha".to_owned()],
            flag: true,
        };

        println!(
            "test_data's string == {}",
            serde_qs::to_string(&test_data).unwrap()
        );
    }

    #[test]
    fn test_serde_json() {}
}
