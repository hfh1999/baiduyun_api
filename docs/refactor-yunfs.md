# YunFs 设计修正与第一批接口扩展方案

> 分支: `fix/master-quality`
> 日期: 2026-09-01
> 状态: **已实施**(2026-09-01 全部阶段完成并验证,实际结果见第八节末)
> 版本背景: 0.3.0 尚未发布,本次允许公共 API 破坏性变更

## 一、背景与动机

### 1.1 YunFs 现存问题

`src/util.rs` 的 `YunFs` 存在设计问题和真实 bug:

| # | 位置 | 问题 | 后果 |
|---|---|---|---|
| 1 | `pwd()` 调用 `get_files_list(path, 0, 0)` 在线验证 | 本地状态查询依赖网络 | 断网时 `pwd()` 失败;每次调用有网络延迟 |
| 2 | `chdir()` 注释"该函数自动定为在线" | 暗示存在"离线模式",实际没有 | 概念误导 |
| 3 | `ls()` 分页循环 `start` 恒为 0 | 目录 ≥1000 个文件时每次拿到同样的前 1000 条 | **死循环**+无限请求+内存膨胀 |
| 4 | `pwd()` 返回 `Result<String, ApiError>` | 语义上"本地查询"不该失败 | 调用方被迫处理不可能的错误 |

### 1.2 接口覆盖缺口

库目前只有 7 个只读方法,缺少网盘**写操作**:

| 缺口的百度接口 | 现状 |
|---|---|
| 创建文件夹 | 无 |
| 管理文件(删除/移动/复制/重命名) | 无 |
| 单步上传 | 无(只有下载 `util::download`) |

## 二、变更目标

1. `pwd()` 变为纯本地操作:零网络、永不失败、返回 `String`。
2. `chdir()` 保留在线验证(切换到不存在的目录应失败,语义合理),清理"在线/离线"提法。
3. 修复 `ls()` 分页死循环。
4. 新增第一批写操作接口:`mkdir` / `remove` / `mv` / `cp` / `rename` / `upload`(含 `get_upload_host`)。
5. `YunFs` 同步提供文件系统风格的方法映射,与现有 `pwd/chdir/ls` 抽象一致。
6. 修复所有 `pwd().unwrap()` 连锁调用点。

## 三、变更设计

### 3.1 `pwd()` — 纯本地化 + 签名变更(破坏性)

```rust
/// 返回当前目录(本地缓存状态,不发网络请求,永不失败)
///
/// 注意:不校验云端目录是否仍然存在;目录被外部删除时,
/// 后续 [ls](YunFs::ls) 等操作才会报错(与本地 shell 语义一致)
pub fn pwd(&self) -> String {
    self.current_path.to_str().unwrap().into()
}
```

- 签名 `Result<String, ApiError>` → `String`
- 删除在线验证请求与错误分支

### 3.2 `chdir()` — 保留验证,清理注释

- 在线验证逻辑**不变**(`get_files_list(dir, 0, 0)` 检查存在性)
- 注释改为:"切换目录时在线确认目标目录存在,不存在则操作失败"
- 删除"自动定为在线"、"离线"等提法

### 3.3 `ls()` — 修复分页

```rust
pub fn ls(&self) -> Result<FileInfoIter, ApiError> {
    let list_len = 1000;
    let mut ret_vec: Vec<FileInfo> = Vec::new();
    let mut start = 0;
    loop {
        let tmp_list =
            self.api.get_files_list(self.current_path.to_str().unwrap(), start, list_len)?;
        let mut tmp_vec: Vec<FileInfo> = tmp_list.collect();
        let len = tmp_vec.len();
        ret_vec.append(&mut tmp_vec);
        if len < list_len {
            break;
        }
        start += list_len;
    }
    Ok(FileInfoIter::new(ret_vec))
}
```

- 关键修复:`start` 每次 `+= 1000`
- 已知限制(本次不做):① 翻页依赖百度排序稳定性,理论上可能重复/遗漏条目;② 整目录拉进内存,超大目录耗内存(未来迭代器化)

### 3.4 基础设施:请求支持 POST

现有 `request()` 只支持 GET(query 参数)。写操作需要 POST,做最小扩展:

```rust
fn request<T: Serialize>(
    &self,
    in_node: YunNode,
    http_method: reqwest::Method,  // GET: 参数进 query; POST: 参数进 form body
    params: &T,
) -> Result<Value, ApiError>
```

- 现有 7 个只读方法改为显式传 `Method::GET`,行为不变
- POST 时参数序列化为 `application/x-www-form-urlencoded` body(filemanager/创建文件夹 均为表单体)
- `parse_response` / `check_errno` 等解析逻辑复用,不动
- 上传 multipart 单独走新私有方法 `request_upload`(见 3.5)

#### JSON 字符串参数统一抽象

百度部分参数要求"整个值是一段 JSON 文本"(如 `fsids=[123,456]`、`filelist=["/a.txt"]`)。现有 `serialize_fsids` 手写格式只服务 `fsids` 一个字段,抽象为通用函数,并替换旧函数:

```rust
/// 把任意 Serialize 值序列化为 JSON 文本,作为 form/query 的单个字段值
pub(crate) fn serialize_json_str<T: Serialize, S: serde::Serializer>(
    value: &T,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(&serde_json::to_string(value).map_err(serde::ser::Error::custom)?)
}
```

- 原理:内层 `serde_json::to_string` 把结构变成 JSON 文本,外层 `serialize_str` 把它编码为单个参数值;`map_err(serde::ser::Error::custom)` 做内外层错误类型桥接
- `fsids`(GetFileInfoParams)与 `filelist`(FileManagerParams)统一指向它,`serialize_fsids` 删除
- 输出与旧实现完全一致(`serde_json::to_string(&[123,456])` == `[123,456]`),由测试保证

#### FilePath trait(两个 trait 的决策)

文件管理操作的对象是**路径**,与现有 `FileId`(id 视角)是平行的第二个视角:

```rust
/// 提供文件在云端的绝对路径(供 filemanager 等操作使用)
pub trait FilePath {
    fn ret_path(&self) -> String;
}
impl FilePath for String { ... }
impl FilePath for str { ... }
impl FilePath for FileInfo { ... }        // self.path
impl FilePath for SearchResult { ... }    // self.path(补字段后)
impl FilePath for FileInfoEx { ... }      // self.path(补字段后)
```

**决策:两个 trait 而非扩展 FileId**。理由:

- `i64` 有 id 无路径——若在 FileId 里加 `ret_path`,传 `i64` 给 remove 只能编译期通过、运行时报错;独立 trait 让编译器直接拒绝,错误暴露在最早时刻;
- trait 边界即契约: `remove<T: FilePath>` 的签名本身就是文档;
- FileInfo 等类型实现两个 trait 各几行,无成本;需要两者时 `T: FileId + FilePath` 自然组合;
- 能力非成对出现的场景,大 trait 一旦建成难以拆分。

**连带修正**:`SearchResult` 与 `FileInfoEx` 补 `path: String` 字段(实测 filemetas/search 真实响应均带 path,模型缺失),补真实响应形状测试。

### 3.5 API 层新方法

#### 创建文件夹

```rust
/// 创建文件夹(百度: method=create&path=xxx&isdir=1,POST)
pub fn mkdir(&self, path: &str) -> Result<(), ApiError>
```

#### 管理文件(filemanager)

```rust
/// 删除文件/目录,支持批量(百度: opera=delete,filelist 为路径字符串数组)
/// T: FilePath——可传路径字符串或 FileInfo/SearchResult/FileInfoEx
pub fn remove<T: FilePath>(&self, paths: &[T]) -> Result<(), ApiError>
/// 移动(百度: opera=move,filelist=[{"path","dest","newname?"}])
pub fn mv<T: FilePath>(&self, from: T, to_dir: &str) -> Result<(), ApiError>
/// 复制(百度: opera=copy,filelist=[{"path","dest","newname?"}])
pub fn cp<T: FilePath>(&self, from: T, to_dir: &str) -> Result<(), ApiError>
/// 重命名(百度: opera=rename,filelist=[{"path","newname"}],一次一个)
pub fn rename<T: FilePath>(&self, path: T, new_name: &str) -> Result<(), ApiError>
```

- 统一走 `POST /rest/2.0/xpan/file?method=filemanager&opera=xxx`
- body 参数:`async=0`(同步,便于直接返回结果)、`filelist=<JSON 字符串>`
- `filelist` 用统一的 `serialize_json_str` 序列化为 JSON 字符串
- 响应 `info` 数组中单文件 errno != 0 时返回第一个错误(透传 errno/errmsg)
- 错误码(-9 文件不存在 / -7 文件名非法 / 111 有异步任务)由 errno 透传

#### 单步上传

```rust
/// 冲突策略(百度 ondup 参数)
pub enum OnDup { Fail, Overwrite, NewCopy }

/// 上传本地文件到网盘(单步上传,限 2GB)
///
/// 流程: 获取上传域名 -> multipart POST {host}/rest/2.0/pcs/file?method=upload
pub fn upload(&self, local_path: &str, remote_path: &str, ondup: OnDup)
    -> Result<UploadResult, ApiError>
```

- 新增私有方法 `get_upload_host()`:调 `POST /rest/2.0/pcs/superfile2?method=locateupload`(或文档指定接口)取上传主机名
- multipart body:`file` = 本地文件内容;URL 参数:`path`、`ondup`
- 新增 `UploadResult { path, size, md5, fs_id }` 模型
- 已知限制:文档注明"此接口可能有限制,推荐分片上传"——2GB 内小文件用,大文件走第二阶段(三步上传)
- 错误码:31024 无上传权限 / 31061 文件已存在 / 31064 路径错误

### 3.6 Util 层:YunFs 新方法

```rust
/// 创建目录(路径经 resolve_path 解析),失败时 Err
pub fn mkdir(&mut self, dir_str: &str) -> Result<(), ApiError>
/// 删除文件/目录(绝对或相对路径),失败时 Err
pub fn rm(&mut self, path: &str) -> Result<(), ApiError>
/// 移动到目录(如 fs.mv("a.txt", "/dest")),失败时 Err
pub fn mv(&mut self, from: &str, to_dir: &str) -> Result<(), ApiError>
/// 复制到目录,失败时 Err
pub fn cp(&mut self, from: &str, to_dir: &str) -> Result<(), ApiError>
/// 上传本地文件到当前目录(与 util::download 对称)
pub fn upload(&mut self, local_path: &str, file_name: &str) -> Result<(), ApiError>
```

- 路径统一走 `resolve_path` 解析(相对/绝对/../ 语义一致)
- 上传 `upload(local, file_name)` 默认 `OnDup::Fail`,文件名拼接到当前目录
- **不自动更新 `current_path`**(mkdir/rm 等不改变当前目录语义)

### 3.7 连锁调用点修复

| 文件 | 位置 | 改动 |
|---|---|---|
| `src/lib.rs` | doctest 两处 `my_fs.pwd().unwrap()` | 改为 `my_fs.pwd()` |
| `src/tests/api_tests.rs` | `get_dlink_flow` 两处 `myfs.pwd().unwrap()` | 改为 `myfs.pwd()` |

### 3.8 授权工具(方案 A:oob 半自动,example 形态)

**背景**:获取 access_token 目前全手动(开浏览器→授权→从地址栏复制),且百度隐式流(`response_type=token`)的 token 在 URL fragment(`#` 之后),**HTTP 回调服务器收不到**,无法全自动。故采用半自动方案:工具生成 URL + 自动打开浏览器 + 用户粘贴地址栏 URL + 自动解析保存。

**形态**:`examples/authorize.rs`(交互式 CLI),不进库的公共 API。

```bash
cargo run --example authorize -- --app-key=你的APP_KEY
```

**流程**:

1. 生成授权 URL(`response_type=token&redirect_uri=oob&scope=netdisk`),打印并自动打开浏览器(Windows `start` / macOS `open` / Linux `xdg-open`);
2. 提示用户把授权后地址栏的完整 URL(如 `http://openapi.baidu.com/oauth/2.0/login_success#access_token=xxx&...`)粘贴回终端;
3. 解析 fragment 提取 `access_token`(含 `expires_in` 展示);
4. 写入项目根目录 `.env`(`BAIDU_ACCESS_TOKEN=...`,已 gitignore),提示"30 天内持续使用不过期"。

**实现要点**:

- 纯标准库实现,无新依赖:URL 解析手写(按 `#`/`&`/`=` 拆分),打开浏览器用 `std::process::Command` 平台分支;
- 解析失败给出明确指引(授权被拒/URL 不完整等);
- 可选参数 `--token-only` 只打印 token 不写文件,方便需要时手抄。

## 四、范围边界

- **本次不改**:
  - `to_str().unwrap()`(理论不可达,列入 util.rs 后续任务)
  - `util::download` unwrap 链(已有后续任务)
  - `FileInfoIter` 迭代器化改造(另开任务)
  - 三步上传(预上传/分片/创建文件)与分享模块(下一阶段)
- 文档层面: `YunFs` 定位描述更新——"本地缓存路径状态 + 操作即时发请求,不存在离线模式"

## 五、兼容性影响

- `pwd()` 签名破坏性变更(`Result` → `String`),调用方需删除 `.unwrap()` / 错误处理——0.3.0 未发布,可接受
- `pwd()` 行为变化:不再校验云端目录存在性
- `chdir()` / `ls()` 签名不变
- 新增方法均为纯增量,不影响现有调用方
- `request` 私有方法签名变化,无外部影响

## 六、测试计划

### 离线测试(TDD)

| 测试 | 覆盖 |
|---|---|
| `pwd` 纯本地 | 构造 YunFs → pwd == "/",零网络 |
| `filelist` 序列化 | delete 数组 / mv/cp 对象数组 / rename 单对象 → JSON 字符串格式正确 |
| `OnDup` 序列化 | Fail/Overwrite/NewCopy → fail/overwrite/newcopy |

### 网络测试(真实 token,带自清理)

| 测试 | 流程 | 清理 |
|---|---|---|
| `mkdir` | 创建 `/apps/bypy/yunfs_test_<随机>/` → list 验证存在 | `remove` 删除 |
| `upload` | 本地生成临时小文件 → 上传 → 验证 | `remove` 删除 |
| `rename/mv/cp` | 上传临时文件 → 改名 → 移动 → 复制 | 删除所有临时文件 |
| 回归 | 现有 13 个 | — |

- 临时目录/文件名带随机后缀,避免并发冲突与残留
- 用"guard 模式"确保测试失败时也尽量清理
- **前提实测**:文档称 filemanager/upload 仅支持 `/apps/{appname}` 路径但示例用非 /apps 路径——首个网络测试先验证实际限制,再定测试目录

## 七、已知风险与决策点

1. **`/apps` 路径限制存疑**:官方权限说明与示例矛盾,实施时首个网络测试实测确定。若确实受限,测试和文档示例统一用 `/apps/bypy/` 下。
2. **上传域名**:每次 upload 先调获取域名接口(多一次请求)。简单起见不缓存,未来可优化。
3. **`ls()` 分页修复**仅改 `start` 偏移,不做去重/迭代器化 —— 待确认。
4. **`chdir()` 保留在线验证** —— 待确认。
5. **`filelist` 的 `newname`**:`mv`/`cp` 先不暴露(保留到目录语义),需要改名移动时用 rename + mv 组合 —— 待确认。
6. **两个 trait 决策**:新增 `FilePath` trait 而非扩展 `FileId`(i64 有 id 无路径,独立 trait 让编译器拒绝错误用法)—— 已确认。
7. **`serialize_json_str` 统一抽象**:替换手写的 `serialize_fsids`,fsids/filelist 共用 —— 已确认。
8. **`SearchResult`/`FileInfoEx` 补 `path` 字段**(真实响应均带 path,模型缺失),补真实响应形状测试 —— 已确认。

## 八、开发步骤(TDD)

### 阶段 1:YunFs 修正(先行,独立可交付)

| 步骤 | 内容 | 验证 |
|---|---|---|
| 1.1 | RED:`pwd` 纯本地测试(离线:构造 YunFs → pwd == "/") | 编译失败 |
| 1.2 | GREEN:`pwd()` 改 `String` + 连锁修复(lib.rs doctest、api_tests 的 `pwd().unwrap()`) | lib 测试全绿 |
| 1.3 | `chdir()` 注释清理("在线/离线"提法删除) | 编译 |
| 1.4 | `ls()` 分页修复(`start += 1000`) | lib 测试全绿 |
| 1.5 | **提交**:`内部修改: YunFs设计修正,pwd纯本地化+修复ls分页死循环` | — |

### 阶段 2:基础设施 + 第一批接口

| 步骤 | 内容 | 验证 |
|---|---|---|
| 2.1 | `request()` 支持 POST(`http_method` 参数,GET 参数进 query / POST 进 form body),现有 7 个方法适配 `Method::GET` | lib 测试全绿(回归) |
| 2.2 | RED:filelist 序列化测试(delete 字符串数组 / mv/cp 对象数组 / rename 单对象) | 编译失败 |
| 2.3 | GREEN:`FileManagerItem` 模型 + `filelist` JSON 字符串序列化 | 序列化测试全绿 |
| 2.4 | `mkdir` + 网络测试(**先实测 `/apps` 路径限制**,决定测试目录) | 网络测试通过 |
| 2.5 | `remove`/`mv`/`cp`/`rename` + 网络测试(带自清理) | 网络测试通过 |
| 2.6 | `OnDup` + `UploadResult` + `get_upload_host` + `upload`(multipart) + 网络测试(上传临时文件后清理) | 网络测试通过 |
| 2.7 | `YunFs::mkdir/rm/mv/cp/upload`(复用 `resolve_path`,离线可测路径解析) | lib 测试全绿 |
| 2.8 | **提交**:`内部修改: 新增第一批写接口mkdir/filemanager/upload及YunFs映射` | 全量+网络回归 |

### 阶段 3:授权工具(方案 A)

| 步骤 | 内容 | 验证 |
|---|---|---|
| 3.1 | `examples/authorize.rs`:生成 URL + 自动打开浏览器 + 粘贴解析 fragment + 写 .env | 手动实测(真实 APP_KEY) |
| 3.2 | README 增加授权工具使用说明 | — |
| 3.3 | **提交**:`内部修改: 新增授权工具example,半自动获取access_token` | — |

### 阶段 4:收尾

| 步骤 | 内容 |
|---|---|
| 4.1 | 本文档状态更新为"已实施",记录实际结果 |
| 4.2 | 提交文档更新 |

**实际结果(2026-09-01)**:

- 阶段 1~3 全部完成,三个提交: `62a3fc5`(YunFs 修正)、`637b810`(第一批接口)、`0d6602b`(授权工具);
- `cargo test --lib`: 56 通过 / 0 失败 / 17 ignored;`cargo test --doc`: 6 全绿;
- 网络测试 17/17 通过(新增 4 个: mkdir_remove / mv_cp_rename / upload / YunFs 相对路径),全部带自清理,路径从 .env 读取 `BAIDU_APP_NAME` 不硬编码;
- 实施中实测修正的文档偏差:
  1. create 接口不传 rtype 时默认**自动重命名**(文档称默认返回冲突),故强制 `rtype=0`;
  2. 上传冲突返回 **HTTP 400** + `error_code` 字段(非 200 + errno),`parse_response` 补充识别 `error_code`/`error_msg`;
  3. locateupload 的 uploadid 文档标必填,**实测可省略**;
  4. upload 路径强制 `/apps/{应用名}/` 下(31064),mkdir/filemanager 不受限;
  5. filemanager 删除不存在的文件静默成功(实测 errno=0)。
