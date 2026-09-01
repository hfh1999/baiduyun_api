//! 这是一个rust写成的百度云api库, **不提供**作弊功能!
//!
//!
//!# 一,简介
//!这个库提供方便地使用百度云官方api的方法 最新的版本请看crates.io的版本号
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
//!```no_run
//!use baiduyun_api::YunApi;
//!let access_token = "User's access_token";
//!let api = YunApi::new(access_token);
//!let user_info = api.get_user_info().unwrap();
//!println!("baidu_name :{}", user_info.baidu_name);
//!println!("vip :{}", user_info.vip_type);
//!```
//!
//!## 2.列出云盘信息
//!列出云盘的存储空间信息的实例如下:
//!```no_run
//!use baiduyun_api::YunApi;
//!let access_token = "User's access_token";
//!let api = YunApi::new(access_token);
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
//!```no_run
//!use baiduyun_api::YunApi;
//!use baiduyun_api::util;
//!
//!let access_token = "User's access_token.";
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
//!```text
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
//!```no_run
//!use baiduyun_api::{util, FileInfo, YunApi};
//!fn download_test() {
//!        let key = "your_access_key_to_user.";
//!        let api = YunApi::new(key);
//!        let mut myfs = util::YunFs::new(&api);
//!        println!("current dir ===> {}", myfs.pwd().unwrap());
//!        myfs.chdir("学习资料/").unwrap();
//!        println!("current dir ===> {}", myfs.pwd().unwrap());
//!        let files = myfs.ls().unwrap();
//!        let mut file_to_download: FileInfo = FileInfo::default();
//!        for item in files {
//!            if item.server_filename.contains("中文第六版@www.java1234.com.pdf") {
//!                println!("pdf: -> {}; id ={} ", item.server_filename, item.fs_id);
//!                file_to_download = item;
//!            }
//!        }
//!        let link = api.get_file_dlink(file_to_download).unwrap();
//!        util::download(&link, "D:/test.pdf", 100, &key, true);//这里打开了debug输出
//!    }
//!
//!```
//!结果如下:
//!```text
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

pub use error::ApiError;
pub use util::YunFs;
pub use yunapi::YunApi;

mod error;
mod models;
pub mod util;
mod yunapi;

pub use models::{UserInfo, QuotaInfo, FileInfo, FileInfoEx, SearchResult, FileInfoIter, FileId, Thumbs};

#[cfg(test)]
mod tests;
