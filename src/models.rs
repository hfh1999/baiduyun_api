use serde::{Deserialize, Serialize};

///用户信息结构体,由[crate::YunApi::get_user_info]返回
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

///配额信息结构体,由[crate::YunApi::get_quota_info]返回
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
///- thumbs,只有请求参数带WEB且该条目分类为图片时,该KEY才存在,详见[Thumbs]
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

///拓展的文件信息结构体,由[crate::YunApi::get_files_info]返回.
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
    /// 真实响应字段名为 `filename`
    #[serde(rename = "filename")]
    pub file_name: String,
    /// 真实响应字段名为 `isdir`
    #[serde(rename = "isdir")]
    pub is_dir: i64,
    pub server_ctime: i64,
    pub server_mtime: i64,
    pub size: i64,
    /// 非图片类型文件不返回该字段
    pub height: Option<i64>,
    /// 非图片类型文件不返回该字段
    pub width: Option<i64>,
    /// 非图片类型文件不返回该字段
    pub date_taken: Option<i64>,
    /// 文件在云端的唯一标识 ID
    pub fs_id: i64,
    /// 文件在云端的绝对路径
    pub path: String,
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
    /// 文件在云端的绝对路径
    pub path: String,
    /// 文件名称
    pub server_filename: String,
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

impl FileId for FileInfoEx {
    fn ret_file_id(&self) -> i64 {
        self.fs_id
    }
}

/// 引用自动透传(传 &FileInfo、&i64 等引用也可直接使用,与 [FilePath] 对称)
impl<T: FileId + ?Sized> FileId for &T {
    fn ret_file_id(&self) -> i64 {
        (**self).ret_file_id()
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
    #[serde(serialize_with = "serialize_json_str")]
    pub fsids: Vec<i64>,
    pub dlink: i64,
    pub extra: i64,
}

/// 把任意 Serialize 值序列化为 JSON 文本,作为 form/query 的单个字段值
///
/// 百度部分参数要求"整个值是一段 JSON 文本"(如 `fsids=[123,456]`、`filelist=["/a.txt"]`)。
/// 原理:内层 serde_json 把结构变成 JSON 文本,外层 serialize_str 编码为单个参数值。
pub(crate) fn serialize_json_str<T: Serialize, S: serde::Serializer>(
    value: &T,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(&serde_json::to_string(value).map_err(serde::ser::Error::custom)?)
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

///提供文件在云端的绝对路径(供 filemanager 等操作使用)
///
///与 [FileId](trait@FileId)(id 视角)是平行的两个视角:
///i64 只有 id 无路径,故独立成 trait 让编译器拒绝错误用法
pub trait FilePath {
    fn ret_path(&self) -> String;
}

impl FilePath for String {
    fn ret_path(&self) -> String {
        self.clone()
    }
}

impl FilePath for str {
    fn ret_path(&self) -> String {
        self.to_string()
    }
}

impl FilePath for FileInfo {
    fn ret_path(&self) -> String {
        self.path.clone()
    }
}

impl FilePath for SearchResult {
    fn ret_path(&self) -> String {
        self.path.clone()
    }
}

impl FilePath for FileInfoEx {
    fn ret_path(&self) -> String {
        self.path.clone()
    }
}

/// 引用自动透传(传 &FileInfo、&String 等引用也可直接使用)
impl<T: FilePath + ?Sized> FilePath for &T {
    fn ret_path(&self) -> String {
        (**self).ret_path()
    }
}

/// 管理文件(filemanager)的 filelist 条目
///
/// 百度协议要求的三种形态:
/// - delete: 纯路径字符串(支持批量)
/// - move/copy: {path, dest, newname?}
/// - rename: {path, newname}(一次一个)
#[derive(Serialize)]
#[serde(untagged)]
pub enum FileManagerItem {
    Delete(String),
    MoveCopy {
        path: String,
        dest: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        newname: Option<String>,
    },
    Rename {
        path: String,
        newname: String,
    },
}

/// filemanager 的 URL query 参数
///
/// 实测确认: opera 必须放 URL query(body 会报 errno=2)
#[derive(Serialize)]
pub struct FileManagerQuery {
    pub opera: String,
}

/// filemanager 的 form body 参数
#[derive(Serialize)]
pub struct FileManagerParams {
    #[serde(serialize_with = "serialize_json_str")]
    pub filelist: Vec<FileManagerItem>,
    #[serde(rename = "async")]
    pub async_: i64,
}

/// 创建文件夹/创建文件(create)的 form body 参数
#[derive(Serialize)]
pub struct CreateParams {
    pub path: String,
    pub isdir: i64,
    /// 命名策略: 0=冲突时返回错误
    ///
    /// 实测确认: 不传 rtype 时百度默认自动重命名,必须显式传 0 才能获得"已存在报错"语义
    pub rtype: i64,
}

/// 上传冲突策略(百度 ondup 参数)
#[derive(Serialize, Debug, Clone, Copy, PartialEq)]
pub enum OnDup {
    /// 冲突时失败(默认)
    #[serde(rename = "fail")]
    Fail,
    /// 冲突时覆盖
    #[serde(rename = "overwrite")]
    Overwrite,
    /// 冲突时自动重命名
    #[serde(rename = "newcopy")]
    NewCopy,
}

/// 上传结果(单步上传 method=upload 的响应)
#[derive(Deserialize, Debug, Clone)]
pub struct UploadResult {
    pub path: String,
    pub size: i64,
    /// 文件 MD5(文档注明只有提交文件时才返回)
    pub md5: Option<String>,
    pub fs_id: i64,
}

/// locateupload(获取上传域名)的 query 参数
#[derive(Serialize)]
pub struct LocateUploadParams {
    /// 固定 250528(文档标注)
    pub appid: i64,
    pub path: String,
    /// 固定 2.0
    pub upload_version: String,
}
