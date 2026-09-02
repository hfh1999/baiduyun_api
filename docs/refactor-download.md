# Download 功能完善方案

> 分支: `feat/split-upload`
> 日期: 2026-09-02
> 状态: **部分实施**——v1(`YunApi::download`)已完成,阶段 2(`YunFs::download`)待实施(见第七节)
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

1. 新增 `YunApi::download`:单文件流式下载引擎(零 panic、覆盖语义、token 内部持有)。
2. 新增 `YunFs::download`:文件系统语义的下载入口(定位→取链→下载预制)。
3. 网络闭环测试补盲区;`YunApi` 保证 `Send + Sync`(用户层并发前提)。
4. 下载扩展能力(v2)与多线程边界以"设计决策记录"(第三节)定稿。

## 二、实测结论(2026-09-02,真实百度,369MB 文件 Desktop.7z)

| # | 实验 | 结果 | 结论 |
|---|---|---|---|
| 1 | 不带 token GET dlink | **403** `error_code:31045 user not exists` | **下载必须带 access_token** → 下载器必须在持有 token 的 `YunApi` 内 |
| 2 | 带 token GET dlink | 200, octet-stream, content-length 369265004 | 正常下载 |
| 3 | `Range: bytes=0-15` | **206**, `content-range: bytes 0-15/369265004`, 16/16 字节 | **dlink 支持 Range 分段**(v2 前提) |

## 三、设计决策记录(讨论定稿)

### 3.1 分层原则:什么进库、什么留给用户

判定标准:**固定且高频的组合 → 预制进库;自由编排 → 用户层;传输调优参数 → 只进引擎层**。

| 层 | 能力 | 归属 | 状态 |
|---|---|---|---|
| 传输引擎 | `YunApi::download`(单文件流式) | 库(协议/IO 细节) | ✅ v1 已完成 |
| 传输引擎增强 | `YunApi::download_with(opts)`(resume/分块) | 库 | v2(前提实验见 8.1) |
| 预制便利 | `YunFs::download`(定位→取链→下载三步) | 库(固定组合) | 阶段 2 |
| 预制便利 | `YunFs::download_dir`(目录递归) | 库(固定组合) | v2 评估(8.2) |
| 文件级自由并发 | 用户用 `std::thread` 组合 | **用户层**,库只保证 `Send + Sync` | 编译期断言(阶段 2) |

**裁决规则**:YunFs 只预制"文件系统语义"(下哪个/存到哪/是否递归),**永不透传传输调优参数**——`cp` 没有"用几个线程"参数,断点/分块参数只属于引擎层 `download_with`。

### 3.2 接口面裁决:一个引擎 + 选项(否决"三个平级接口")

resume 与分块并发是**同一分段调度器的两种参数**,不是三个独立能力:

```rust
// 便捷入口(v1,已实施)= offset 0 + threads 1
pub fn download(&self, dlink: &str, dst: &str) -> Result<u64, ApiError>
// 增强入口(v2)
pub fn download_with(&self, dlink: &str, dst: &str, opts: DownloadOpts) -> Result<u64, ApiError>
pub struct DownloadOpts { offset: u64, threads: usize }  // 未来可加:进度回调/限速
```

否决三平级接口(download / download_resume / download_parallel)的理由:实现重复(Range 逻辑各写一遍)、组合爆炸(想"并发+续传"要第 4 个方法)、未来加选项(限速/进度)需 ×N。

### 3.3 多线程的三个维度(谁负责什么)

| 维度 | 主体 | 库的动作 |
|---|---|---|
| 义务层:库可被多线程共享 | 库 | `YunApi` 天然 `Send + Sync`(ureq::Agent 内部 Arc),加编译期断言防回归 |
| 文件级并发(多文件同时下) | **用户层** | 不内置;Send+Sync 保证 + 示例演示 |
| 分块级并发(单文件多段) | 库引擎层(v2) | `download_with(threads>1)`;前提实验见 8.1 |

### 3.4 异步下载图纸(异步 feature 落地时实施,接口镜像同步)

- 接口面与同步一致(download / download_with),下载语义(token 拼接/覆盖/字节数)是共享层事实;
- 分块并发实现变廉价:`join_all` 几个 range future,无需库内置线程池(executor 用调用者的);
- 同步做不出的形态:**Stream 化下载**(逐块 yield → 进度/限速/取消)列为可选项;
- 文件写入选 tokio::fs 真异步还是 spawn_blocking 包 std::fs——实施时定。

## 四、v1 设计(已实施:`YunApi::download`)

### 4.1 签名与语义

```rust
/// 下载文件到本地(自动拼接 access_token,流式落盘,覆盖已存在)
///
/// - `dlink` 来自 [Self::get_file_dlink] / [Self::get_files_dlink_vec](链接 8 小时有效)
/// - `dst` 本地文件路径;**已存在会被覆盖**(非续写)
/// - 返回实际下载字节数,可与远端 `FileInfo.size` 对比校验完整性
pub fn download(&self, dlink: &str, dst: &str) -> Result<u64, ApiError>
```

- URL 拼接纯函数 `with_access_token`:dlink 不含 `access_token=` 则追加(实测必需);已带不重复。
- 全链路错误映射 `ApiError`,零 panic;非 2xx 走 `parse_response` 纯函数 → 带 errno 的 ApiError(错误直透延续到下载)。

### 4.2 流式落盘实现细节(技术沉淀)

ureq 3.4 的 `body_mut().read_to_vec()/read_to_string()` 会把整段响应读进内存(369MB 不可行);`body_mut().as_reader()` 返回实现 `std::io::Read` 的 `BodyReader`——流式源头:

```rust
let mut file = std::fs::File::create(dst)  // create = 覆盖(truncate)
    .map_err(|e| ApiError::from(format!("open local file error: {}", e).as_str()))?;
let mut reader = response.body_mut().as_reader();
let mut buf = [0u8; 64 * 1024];            // 固定 64KB 缓冲,内存恒定与文件大小无关
let mut total: u64 = 0;
loop {
    let n = match reader.read(&mut buf) {
        Ok(n) => n,
        Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
        Err(e) => return Err(ApiError::from(format!("read download body error: {}", e).as_str())),
    };
    if n == 0 { break; }                   // EOF = 完成
    file.write_all(&buf[..n])
        .map_err(|e| ApiError::from(format!("write local file error: {}", e).as_str()))?;
    total += n as u64;
}
```

关键点:
- **必须完整读到 EOF**:ureq Agent 连接池要求 body 读净才能复用连接;中途 drop 连接作废(断点续传动机);
- **非 2xx**:错误体是 JSON(实测 403 → 31045),先 read_to_string(错误体很小)再走 `parse_response`;
- **返回 total**:调用方与远端 size 对比校验(v1 不内置校验)。

### 4.3 旧 `util::download` 处理

保留不动(0.3.0 稳定承诺),加 `#[deprecated(note = "请使用 YunApi::download / YunFs::download")]`——不破坏编译(仅警告)。

## 五、阶段 2 设计(待实施:`YunFs::download`)

```rust
/// 下载当前目录下的文件到本地(自动定位 + 取链 + 下载)
///
/// - `file_name` 当前目录内的文件名(需在线确认存在,与 chdir 语义一致)
/// - `local` 本地保存的完整文件路径;**已存在会被覆盖**
/// - 返回实际下载字节数
pub fn download(&mut self, file_name: &str, local: &str) -> Result<u64, ApiError>
```

- 实现 = 三步编排:当前目录 `ls` + find 定位 → `get_file_dlink` → `YunApi::download`;传输引擎不重复(5 行内);
- 定位开销:一次 ls(≤1000 条)+ find,接受;
- `local` 为完整本地文件路径;本地目标是目录时自动拼远端文件名 → v2(`cp` 行为);
- 需要断点/并发等传输调优时,文档指引用户直接用 API 层组合(YunFs 不透传)。

## 六、测试计划

### 离线(TDD,先行)

| 测试 | 覆盖 | 状态 |
|---|---|---|
| `with_access_token` 拼接(带/不带 token) | 追加 / 不重复追加 | ✅ 已实施(67 passed) |
| `YunApi: Send + Sync` 编译期断言 | 用户层并发前提防回归 | 阶段 2(2.0) |

### 网络(真实闭环,带自清理,单线程防频控)

| 测试 | 流程 | 状态 |
|---|---|---|
| `download_roundtrip` | 上传 16KB 全字节值内容 → 取链 → `YunApi::download` 落盘 → **逐字节一致** + 字节数 == 内容长度 → 清理 | ✅ 已通过 |
| `yunfs_download` | 上传临时文件 → chdir → `fs.download("文件名", 本地)` → 字节一致 → 清理 | 阶段 2(2.2) |
| 大文件手动验证 | Desktop.7z(369MB)整文件下载 → 大小 == 369265004 | 手动 |

## 七、开发步骤与实际结果

### 阶段 1:核心下载器(✅ 已完成)

| 步骤 | 内容 | 结果 |
|---|---|---|
| 1.1 | RED:`with_access_token` 拼接单测 | 编译失败确认 |
| 1.2 | GREEN:`download` 实现(流式落盘/覆盖/错误透传) | lib 67 passed |
| 1.3 | `util::download` 加 `#[deprecated]` | 编译无警告(库内无调用点) |
| 1.4 | 网络测试 `download_roundtrip` | ✅ 通过(16KB 全字节值逐字节一致) |
| 1.5 | 提交 `72df67f` | ✅ |

### 阶段 2:YunFs 映射(待实施)

| 步骤 | 内容 | 验证 |
|---|---|---|
| 2.0 | 编译期断言 `YunApi: Send + Sync` | 编译 |
| 2.1 | `YunFs::download`(定位 + 取链 + 下载) | lib 测试全绿 |
| 2.2 | 网络测试 `yunfs_download` 闭环 | 网络测试通过 |
| 2.3 | README/lib.rs 示例补充下载用法 | doc 测试全绿 |
| 2.4 | **提交**:`内部修改: YunFs新增download映射,个人下载闭环完成` | — |

### 阶段 3:收尾

| 步骤 | 内容 |
|---|---|
| 3.1 | 本文档状态更新为"已实施",记录实际结果 |
| 3.2 | 提交文档 |

## 八、v2 图纸(未排期,前提实验先行)

### 8.1 `download_with(opts)` — 引擎增强

- **前提实验(实施前必须做)**:① 百度单连接是否限速(不限速则分块并发纯添乱) ② dlink 并发 Range 容忍度——只做 resume 的门槛低,分块并发的门槛高;
- 语义:offset > 0 断点续传——请求 `Range: bytes={offset}-`,响应 **206 → append 写入**;响应 **200(服务器忽略 Range)→ 必须从头 truncate**(双态处理);
- threads > 1:每块独立可重试 = 分块天然含断点语义,无需独立 resume 接口。

### 8.2 `download_dir` — 目录递归(v2 评估)

- 若做,串行先行:树遍历 + **批量取链**(`get_files_dlink_vec` 攒批,避免逐文件 filemetas 触发频控 31034);
- 并发的坑(记录,防踩):错误语义(串行"遇错即停报告位置"干净;并发部分成功 → 返回类型变化,若做用"遇错即停调度"保串行语义)、线程数必须封顶(大目录爆连接池/句柄)、重试与错误聚合纠缠、并发需先全量遍历收集任务(多一轮网络);
- 倾向:**串行可能即终态**——不限速则并发只是把总带宽切碎。

## 九、已确认小决策

1. 进度回调 v1 不做:返回字节数自行展示;回调形态(闭包/句柄/stream)异步图纸再议。
2. `fs.download` 的 `local` 语义:v1 为完整本地路径;目录目标自动拼名 v2。
3. 错误文案统一 `ApiError`,错误直透(errno)承诺覆盖下载链路。
