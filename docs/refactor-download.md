# Download 功能完善方案

> 分支: `feat/split-upload`
> 日期: 2026-09-02
> 状态: **已实施**(v1 + v2 全部完成,实际结果见第七节末)
> 版本背景: 0.3.1 已发布。0.3.0 起承诺 API 稳定(只增不改):本方案新增 API,旧 `util::download` 保留仅废弃

## 一、背景与动机

### 1.1 现状问题(旧 `util::download`)

旧下载函数是全库质量最差的一块,且是全库唯一**无网络测试覆盖**的功能:

| # | 问题 | 后果 |
|---|---|---|
| 1 | 满屏 `.unwrap()`(打开文件/网络/header 解析失败直接 panic) | 与库"零 panic 设计"承诺矛盾 |
| 2 | `append(true)` 追加写入 | **重复下载续写脏文件**,应覆盖 |
| 3 | 签名割裂:url 与 access_token 分开传 | 调用方自行拼接;token 本应在 `YunApi` 内 |
| 4 | `block_size: i32` 传 MB,负值/边界无校验;`is_debug: bool` | 参数语义混乱 |
| 5 | 无网络测试(取链有测,真实下载 0 覆盖) | 全库唯一测试盲区 |

### 1.2 目标

| 版本 | 范围 | 一句话 |
|---|---|---|
| v1 | `YunApi::download` + `YunFs::download` | 单文件流式下载,零 panic、覆盖语义、token 内部持有 |
| v2 | `download_with(opts)` + `YunFs::download_dir` | 断点续传/分块并发(引擎)、目录递归(YunFs) |

两个版本的接口裁决与分层原则见第三节;逐版本完整设计见第四、五节。

## 二、实测结论(2026-09-02,真实百度,369MB 文件 Desktop.7z)

| # | 实验 | 结果 | 结论 |
|---|---|---|---|
| 1 | 不带 token GET dlink | **403** `error_code:31045 user not exists` | **下载必须带 access_token** → 下载器必须在持有 token 的 `YunApi` 内 |
| 2 | 带 token GET dlink | 200, octet-stream, content-length 369265004 | 正常下载 |
| 3 | `Range: bytes=0-15` | **206**, `content-range: bytes 0-15/369265004`, 16/16 字节 | **dlink 支持 Range 分段**(v2 断点/分块前提) |

## 三、设计决策记录(讨论定稿)

### 3.1 分层全景(什么进库、什么留给用户)

判定标准:**固定且高频的组合 → 预制进库;自由编排 → 用户层;传输调优参数 → 只进引擎层**。

| 层 | 能力 | 归属 | 版本 | 详情 |
|---|---|---|---|---|
| 传输引擎 | `YunApi::download`(单文件流式) | 库(协议/IO 细节) | v1 ✅ | 4.1 |
| 传输引擎增强 | `YunApi::download_with(opts)`(断点/分块) | 库 | v2 | 4.2 |
| 预制便利 | `YunFs::download`(定位→取链→下载三步) | 库(固定组合) | v1 | 5.1 |
| 预制便利 | `YunFs::download_dir`(目录递归 `cp -r`) | 库(固定组合) | v2 | 5.2 |
| 文件级自由并发 | 用户 `std::thread` 组合 | **用户层** | 库只保证 `Send+Sync` | 3.3 |

**裁决规则**:YunFs 只预制"文件系统语义"(下哪个/存到哪/是否递归),**永不透传传输调优参数**——`cp` 没有"用几个线程"参数,断点/分块参数只属于引擎层。

### 3.2 接口面裁决(否决"三个平级接口")

resume 与分块并发是**同一分段调度器的两种参数**(offset/threads),不是三个独立能力。三平级接口(download / download_resume / download_parallel)的问题:实现重复、组合爆炸(想"并发+续传"要第 4 个方法)、未来加选项(限速/进度)需 ×N。故定:**一个引擎 + 选项**,下载接口形态见 4.1/4.2。

### 3.3 多线程的三个维度(谁负责什么)

| 维度 | 主体 | 库的动作 |
|---|---|---|
| 义务层:库可被多线程共享 | 库 | `YunApi` 天然 `Send + Sync`(ureq::Agent 内部 Arc),加编译期断言防回归 |
| 文件级并发(多文件同时下) | **用户层** | 不内置;Send+Sync 保证 + 示例演示 |
| 分块级并发(单文件多段) | 引擎层 v2 | `download_with(threads>1)`;前提实验见 4.2 |

### 3.4 异步下载图纸(异步 feature 落地时实施,接口镜像同步)

- 接口面与同步一致(download / download_with),下载语义(token 拼接/覆盖/字节数)是共享层事实;
- 分块并发实现变廉价:`join_all` 几个 range future,无需库内置线程池(executor 用调用者的);
- 同步做不出的形态:**Stream 化下载**(逐块 yield → 进度/限速/取消)列为可选项;
- 文件写入选 tokio::fs 真异步还是 spawn_blocking 包 std::fs——实施时定。

## 四、引擎层设计(API 层下载)

### 4.1 v1 `YunApi::download` — 单文件流式引擎(✅ 已实施)

```rust
/// 下载文件到本地(自动拼接 access_token,流式落盘,覆盖已存在)
///
/// - `dlink` 来自 [Self::get_file_dlink] / [Self::get_files_dlink_vec](链接 8 小时有效)
/// - `dst` 本地文件路径;**已存在会被覆盖**(非续写)
/// - 返回实际下载字节数,可与远端 `FileInfo.size` 对比校验完整性
pub fn download(&self, dlink: &str, dst: &str) -> Result<u64, ApiError>
```

**语义与实现要点**:
- URL 拼接纯函数 `with_access_token`:dlink 不含 `access_token=` 则追加(实测必需);已带不重复;
- 全链路错误映射 `ApiError`,零 panic;非 2xx 走 `parse_response` 纯函数 → 带 errno 的 ApiError(错误直透延续到下载);
- 覆盖语义 `File::create`(truncate),修复旧实现 append 续写 bug。

**流式落盘细节(技术沉淀)**:ureq 3.4 的 `read_to_vec()/read_to_string()` 会整段进内存(369MB 不可行);`body_mut().as_reader()` 返回实现 `std::io::Read` 的 `BodyReader`——流式源头:

```rust
let mut file = std::fs::File::create(dst)
    .map_err(|e| ApiError::from(format!("open local file error: {}", e).as_str()))?;
let mut reader = response.body_mut().as_reader();
let mut buf = [0u8; 64 * 1024];   // 固定 64KB 缓冲,内存恒定与文件大小无关
let mut total: u64 = 0;
loop {
    let n = match reader.read(&mut buf) {
        Ok(n) => n,
        Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
        Err(e) => return Err(ApiError::from(format!("read download body error: {}", e).as_str())),
    };
    if n == 0 { break; }          // EOF = 完成
    file.write_all(&buf[..n])
        .map_err(|e| ApiError::from(format!("write local file error: {}", e).as_str()))?;
    total += n as u64;
}
```

- **必须完整读到 EOF**:ureq Agent 连接池要求 body 读净才能复用连接;中途 drop 连接作废(断点续传的动机);
- **非 2xx**:错误体是 JSON(实测 403 → 31045),先 read_to_string(错误体很小)再走 `parse_response`;
- **返回 total**:调用方与远端 size 对比校验(v1 不内置校验)。

### 4.2 v2 `YunApi::download_with` — 断点续传 + 分块并发(未排期,前提实验先行)

```rust
/// 下载文件,支持断点续传与分块并发(引擎增强入口)
///
/// - `dlink`/`dst` 语义同 [Self::download]
/// - `opts.offset` > 0:断点续传——从该字节偏移继续,追加写入 `dst`;
///   服务器忽略 Range 返回 200 全量时,自动回退从头下载(truncate)
/// - `opts.threads` > 1:分块并发——把 `[offset, 文件尾)` 切成 threads 块并行拉取
/// - 返回实际(本次)下载字节数
pub fn download_with(&self, dlink: &str, dst: &str, opts: DownloadOpts) -> Result<u64, ApiError>

pub struct DownloadOpts {
    pub offset: u64,    // 0 = 从头;>0 = 断点续传
    pub threads: usize, // 1 = 单线程(与 download 等价);>1 = 分块并发
}
impl Default for DownloadOpts { /* offset: 0, threads: 1 */ }
```

**行为矩阵**(组合天然成立,无需第 4 个接口):

| offset | threads | 行为 |
|---|---|---|
| 0 | 1 | 全量单线程(= v1 `download`,便捷入口是其特例) |
| >0 | 1 | 断点续传:Range 请求 + append;200(忽略 Range)时回退从头 |
| 0 | >1 | 分块并发:切块并行拉取,块失败独立重试 |
| >0 | >1 | 并发续传:从 offset 切块 |

**关键语义**:
- 断点双态:请求 `Range: bytes={offset}-`;响应 **206 → append 写入**;响应 **200 → 从头 truncate 重下**(不能 append 错位);
- 分块并发 = 同一调度器的参数组合,每块独立可重试 → **分块天然含断点语义**;
- 进度回调/限速字段未来在此结构上扩展(不加新接口)。

**前提实验(实施前必须做)**:
1. 百度单连接是否限速——**不限速则分块并发纯添乱**,只实现 offset(断点)即可;
2. dlink 并发 Range 容忍度(多连接是否被 CDN 限/封);
3. 续传后文件完整性校验方案(offset 校验/大小对比,避免错位续传污染文件)。

## 五、YunFs 层设计(文件系统语义入口)

### 5.1 v1 `YunFs::download` — 单文件下载(待实施)

```rust
/// 下载当前目录下的文件到本地(自动定位 + 取链 + 下载)
///
/// - `file_name` 当前目录内的文件名(需在线确认存在,与 chdir 语义一致)
/// - `local` 本地保存的完整文件路径;**已存在会被覆盖**
/// - 返回实际下载字节数
pub fn download(&mut self, file_name: &str, local: &str) -> Result<u64, ApiError>
```

- 实现 = 三步编排:当前目录 `ls` + find 定位 → `get_file_dlink` → 调 [YunApi::download](crate::YunApi::download)(传输引擎不重复,几行组合);
- 定位开销:一次 ls(≤1000 条)+ find,接受;与 chdir 在线验证语义一致;
- 需要断点/并发等传输调优时,文档指引用户直接用 API 层组合(YunFs 不透传)。

### 5.2 v2 `YunFs::download_dir` — 目录递归下载(未排期,评估项)

- **若做,串行先行**:树遍历 + **批量取链**(`get_files_dlink_vec` 攒批,避免逐文件 filemetas 触发 API 频控 31034);
- 本地目标为目录时自动拼远端文件名(`cp` 行为)一并实现;
- **并发的坑(记录,防踩)**:错误语义(串行"遇错即停报告位置"干净;并发部分成功 → 返回类型变化——若做,用"遇错即停调度"保串行语义,不加部分成功报告类型)、线程数必须封顶(大目录爆连接池/句柄)、失败重试与错误聚合纠缠、并发需先全量遍历收集任务(多一轮网络);
- 并发前提实验同 4.2;倾向:**串行可能即终态**——不限速则并发只是把总带宽切碎。

## 六、测试计划

### 离线(TDD,先行)

| 测试 | 覆盖 | 状态 |
|---|---|---|
| `with_access_token` 拼接(带/不带 token) | 追加 / 不重复追加 | ✅ 已实施(67 passed) |
| `YunApi: Send + Sync` 编译期断言 | 用户层并发前提防回归 | 阶段 2(2.0) |

### 网络(真实闭环,带自清理,单线程防频控)

| 测试 | 流程 | 状态 |
|---|---|---|
| `download_roundtrip` | 上传 16KB 全字节值内容 → 取链 → `download` 落盘 → **逐字节一致** + 字节数 == 内容长度 → 清理 | ✅ 已通过 |
| `yunfs_download` | 上传临时文件 → chdir → `fs.download("文件名", 本地)` → 字节一致 → 清理 | 阶段 2(2.2) |
| 大文件手动验证 | Desktop.7z(369MB)整文件下载 → 大小 == 369265004 | 手动 |

## 七、开发步骤与实际结果

### 阶段 1:引擎 v1(✅ 已完成)

| 步骤 | 内容 | 结果 |
|---|---|---|
| 1.1 | RED:`with_access_token` 拼接单测 | 编译失败确认 |
| 1.2 | GREEN:`download` 实现(流式落盘/覆盖/错误透传) | lib 67 passed |
| 1.3 | `util::download` 加 `#[deprecated]` | 编译无警告(库内无调用点) |
| 1.4 | 网络测试 `download_roundtrip` | ✅ 通过(16KB 全字节值逐字节一致) |
| 1.5 | 提交 `72df67f` | ✅ |

### 阶段 2:YunFs v1(待实施)

| 步骤 | 内容 | 验证 |
|---|---|---|
| 2.0 | 编译期断言 `YunApi: Send + Sync` | 编译 |
| 2.1 | `YunFs::download`(5.1:定位 + 取链 + 下载) | lib 测试全绿 |
| 2.2 | 网络测试 `yunfs_download` 闭环 | 网络测试通过 |
| 2.3 | README/lib.rs 示例补充下载用法 | doc 测试全绿 |
| 2.4 | **提交**:`内部修改: YunFs新增download映射,个人下载闭环完成` | — |

### 阶段 3:收尾

| 步骤 | 内容 |
|---|---|
| 3.1 | 本文档状态更新为"已实施",记录实际结果 |
| 3.2 | 提交文档 |

**实际结果(2026-09-02)**:

- 阶段 1~3 全部完成,一次统一提交;
- 交付:`YunApi::download`(v1)、`YunApi::download_with`(offset 断点续传 + threads 分块并发,含 `DownloadOpts`)、`YunFs::download`、`YunFs::download_dir`(递归镜像 + fs_id 对齐批量取链)、`util::download` 废弃;
- 实施中实测修正:
  1. **并发测速**:单连接 3.9-4.3MB/s,8 连接 5.1MB/s(加速 ~1.3x 封顶)——百度有聚合带宽限制(本地基线 7.0MB/s);threads 保留(温和收益);
  2. **UA 风控**:API 签发的 dlink 配浏览器 UA 触发 403(31326 hitcode:119),`pan.baidu.com` UA 是白名单——维持现状,勿"优化"成浏览器 UA;
  3. **批量取链顺序**:百度 filemetas 响应可能按 fs_id 排序而非请求顺序——`download_dir` 按 fs_id 对齐(勿用 zip 假设顺序);
- 测试:`cargo test` lib 68 + doc 9 全绿;网络测试 **24/24**(55s 单线程)含 download_roundtrip / yunfs_download / download_resume / download_parallel(8线程) / download_dir 五个下载闭环。

## 八、已确认小决策

1. 进度回调 v1 不做:返回字节数自行展示;回调形态(闭包/句柄/stream)异步图纸再议。
2. `fs.download` 的 `local` 语义:v1 为完整本地路径;目录目标自动拼名并入 v2 download_dir。
3. 错误文案统一 `ApiError`,错误直透(errno)承诺覆盖下载链路。
