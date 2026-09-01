use serde::{Deserialize, Serialize};

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
    pub avatar_url: String,
    pub vip_type: i64,
    pub uk: i64,
}

///配额信息结构体,由[YunApi::get_quota_info()]返回
///
///包含了以下四个字段
///- total,总空间大小
///- expire,7天内是否由容量到期
///- used,已使用大小,单位B
///- free,剩余大小,单位B
///
///要想要方便的进行单位转换参看[这个函数](crate::util::human_quota())
#[derive(Serialize, Deserialize)]
pub struct QuotaInfo {
    pub total: i64,
    pub expire: bool,
    pub used: i64,
    pub free: i64,
}

///缩略图信息结构体
///
///百度 list/search 接口返回的 `thumbs` 字段实际是对象(官方文档参数表标注为 string,与示例矛盾,实测为对象):
///- icon,小尺寸缩略图
///- url1/url2/url3,三个尺寸的缩略图URL
///
///目录等无缩略图条目返回空对象 `{}`,全部字段为 None
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct Thumbs {
    pub icon: Option<String>,
    pub url1: Option<String>,
    pub url2: Option<String>,
    pub url3: Option<String>,
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
///- size,文件大小,单位B,要想要方便的进行单位转换参看[这个函数](crate::util::human_quota())
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
    pub thumbs: Option<Thumbs>,
    pub dir_empty: Option<i64>,
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
///- size,文件大小,单位B,要想要方便的进行单位转换参看[这个函数](crate::util::human_quota())
///  下面几个是文件类型为图片才有效:
///- height 图片高度.
///- width 图片宽度.
///- date_taken 图片的拍摄时间.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FileInfoEx {
    pub category: i64,
    pub dlink: String,
    pub file_name: String,
    pub is_dir: i64,
    pub server_ctime: i64,
    pub server_mtime: i64,
    pub size: i64,
    pub height: i64,
    pub width: i64,
    pub date_taken: i64,
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
    pub thumbs: Option<Thumbs>,
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

impl FileId for i64 {
    fn ret_file_id(&self) -> i64 {
        *self
    }
}

impl FileId for FileInfo {
    fn ret_file_id(&self) -> i64 {
        self.fs_id
    }
}

impl FileId for SearchResult {
    fn ret_file_id(&self) -> i64 {
        self.fs_id
    }
}

impl Iterator for FileInfoIter {
    type Item = FileInfo;
    fn next(&mut self) -> Option<Self::Item> {
        if self.inner_count >= self.inner_data.len() {
            None
        } else {
            let tmp = Some(self.inner_data[self.inner_count].clone());
            self.inner_count += 1;
            tmp
        }
    }
}

#[derive(Serialize)]
pub struct GetFileListParams {
    pub dir: String,
    pub start: i64,
    pub limit: i64,
}

#[derive(Serialize)]
pub struct GetFileInfoParams {
    #[serde(serialize_with = "serialize_fsids")]
    pub fsids: Vec<i64>,
    pub dlink: i64,
    pub extra: i64,
}

fn serialize_fsids<S>(fsids: &[i64], serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    let tmp_string: String = fsids
        .iter()
        .map(|x| x.to_string())
        .collect::<Vec<_>>()
        .join(",");
    serializer.serialize_str(&format!("[{}]", tmp_string))
}

#[derive(Serialize)]
pub struct SearchParams {
    pub key: String,
    pub dir: String,
    pub recursion: i64,
    pub page: i64,
    pub num: i64,
    pub web: i64,
}

#[derive(Serialize)]
pub struct EmptyParams;
