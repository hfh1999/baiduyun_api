# Download 功能完善方案

> 分支: `feat/sync-and-async`
> 日期: 2026-09-02
> 状态: **待实施**(实测前提已确认,见第二节)
> 版本背景: 0.3.1 已发布。0.3.0 起承诺 API 稳定(只增不改),本方案**新增** API、旧 `util::download` 保留

## 一、现状问题

`util::download`([util.rs:273](src/util.rs))是全库质量最差的一块,且是全库唯一**无网络测试覆盖**的功能:

| # | 问题 | 后果 |
|---|---|---|
| 1 | 满屏 `.unwrap()`(打开文件/网络/header 解析失败直接 panic) | 与库"零 panic 设计"承诺矛盾 |
| 2 | `append(true)` 追加写入 | **重复下载续写脏文件**,应覆盖 |
| 3 | 签名割裂:url 与 access_token 分开传 | 调用方自行拼接;token 本应在 `YunApi` 内 |
| 4 | `block_size: i32` 传 MB,负值/边界无校验;`is_debug: bool` | 参数语义混乱 |
| 5 | 无网络测试(取链有测,真实下载 0 覆盖) | 全库唯一测试盲区 |

## 二、实测结论(2026-09-02,真实百度,369MB 文件 Desktop.7z)

| # | 实验 | 结果 | 结论 |
|---|---|---|---|
| 1 | 不带 token GET dlink | **403** `error_code:31045 user not exists` | **下载必须带 access_token** → 下载器必须在持有 token 的 `YunApi` 内 |
| 2 | 带 token GET dlink | 200, octet-stream, content-length 369265004 | 正常下载 |
| 3 | `Range: bytes=0-15` | **206**, `content-range: bytes 0-15/369265004`, 16/16 字节 | **dlink 支持 Range 分段** |

## 三、目标设计

### 3.1 `YunApi::download` — 核心下载器(新增,公共 API)

```rust
/// 下载文件到本地(自动拼接 access_token,流式落盘,覆盖已存在)
///
/// - `dlink` 来自 [Self::get_file_dlink] / [Self::get_files_dlink_vec](百度 8 小时有效)
/// - `dst` 本地文件路径;**已存在会被覆盖**
/// - 返回实际下载字节数(可与远端 size 对比校验)
pub fn download(&self, dlink: &str, dst: &str) -> Result<u64, ApiError>
```

- 下载 URL 拼接:`dlink` 若不含 `access_token` 参数则追加(实测必需,见第二节)
- 流式落盘:ureq body reader 循环写入文件,**不整文件进内存**(369MB 级文件实测可行)
- 全链路错误映射 `ApiError`(HTTP 失败/文件 IO/网络中断),零 panic
- 覆盖语义:`create(true).write(true).truncate(true)`(替代旧实现的 append 续写)
- **v1 不做分段**(单请求流式已覆盖 2GB 内单步上传对应规模);Range 分段/断点续传列为 v2 决策点(第二节实测已证明可行性)

#### 流式落盘实现细节(技术核心)

ureq 3.4 响应 body 的读取 API 决定实现形态:`body_mut()` 的 `read_to_vec()`/`read_to_string()` 会把**整段响应读进内存**(369MB 文件不可行);而 `body_mut().as_reader()` 返回实现了 `std::io::Read` 的 `BodyReader`——流式源头。

```rust
// 状态码检查通过后:
let mut file = std::fs::File::create(dst)   // create = 覆盖(truncate),修复原 append 续写 bug
    .map_err(|e| ApiError::from(format!("open local file error: {}", e).as_str()))?;
let mut reader = response.body_mut().as_reader();  // BodyReader: impl std::io::Read
let mut buf = [0u8; 64 * 1024];                    // 固定 64KB 缓冲 —— 内存恒定,与文件大小无关
let mut total: u64 = 0;
loop {
    let n = match reader.read(&mut buf) {
        Ok(n) => n,
        Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue, // 中断重试
        Err(e) => return Err(ApiError::from(format!("read download body error: {}", e).as_str())),
    };
    if n == 0 { break; }                            // EOF = 下载完成
    file.write_all(&buf[..n])                       // write_all 内部处理部分写入
        .map_err(|e| ApiError::from(format!("write local file error: {}", e).as_str()))?;
    total += n as u64;
}
```

要点:
- **非 2xx 分支**:下载失败时百度返回 JSON 错误体(实测 403 → `{"error_code":31045,...}`),先 `read_to_string()`(错误体很小)再走现有 `parse_response` 纯函数 → 产出带 errno 的 `ApiError`,错误直透承诺延续到下载;
- **必须完整读到 EOF**:ureq Agent 连接池要求 body 读净才能复用连接;中途 drop reader 连接作废——这也是 v2 断点续传的动机;
- **返回 `total`**:调用方可与远端 `FileInfo.size` 比对校验完整性(v1 不强制内置校验)。

### 3.2 `YunFs::download` — 用户入口(新增)

```rust
/// 下载当前目录下的文件到本地(自动取链 + 下载)
///
/// - `file_name` 当前目录内的文件名(需在线确认存在,与 chdir 语义一致)
/// - `local` 本地保存路径(已存在会被覆盖)
pub fn download(&mut self, file_name: &str, local: &str) -> Result<u64, ApiError>
```

实现:当前目录 `ls` 定位同名文件 → `get_file_dlink` → 调 [YunApi::download](crate::YunApi::download)。

> 决策点:`download` 需要在线 list 定位文件(无 FileId 可用)——与 `chdir` 的在线验证一致,文档注明。

### 3.3 旧 `util::download` 处理

- **保留不动**(0.3.0 稳定性承诺:不改已有签名行为),加 `#[deprecated(note = "请使用 YunApi::download / YunFs::download")]`
- `#[deprecated]` 不破坏编译(仅警告),属于允许范围

## 四、兼容性影响

| 项 | 影响 |
|---|---|
| 新增 `YunApi::download` / `YunFs::download` | 纯增量,不影响现有调用方 |
| `util::download` 加 `#[deprecated]` | 调用方出现 deprecation 警告(非错误);库内自身调用点(如有)同步换新 |

## 五、测试计划

### 离线测试(TDD,先行)

| 测试 | 覆盖 |
|---|---|
| URL 拼接纯函数:带/不带 access_token 的 dlink | 不重复追加、编码不变 |
| download 参数校验(空 dlink/空 dst?) | 边界 |

### 网络测试(真实闭环,带自清理)

| 测试 | 流程 | 清理 |
|---|---|---|
| `download_roundtrip` | 上传临时小文件 → `get_file_dlink` → `YunApi::download` 到临时目录 → **读回字节与上传内容逐字节一致** + 返回字节数 == 文件 size | 删远端文件 + 删本地临时文件 |
| `yunfs_download` | YunFs chdir 到临时目录 → `fs.download("文件名", 本地)` → 字节一致 | 同上 |
| 大文件手动验证 | Desktop.7z(369MB)整文件下载 → 大小 == 369265004 | 手动 |

> 注意:百度频控(31034),新网络测试并入现有 19 个单线程队列;每测试 1 次上传 + 1-2 次下载请求。

## 六、分层设计与决策记录(讨论定稿 2026-09-02)

### 6.1 下载功能分层(判定标准:固定高频组合 → 预制进库;自由编排 → 用户层;传输调优 → 只进引擎层)

| 层 | 能力 | 状态 |
|---|---|---|
| `YunApi::download` | 单文件流式引擎(token 拼接/覆盖/错误透传/返回字节数) | ✅ v1 已完成 |
| `YunApi::download_with(opts)` | resume + 分块并发(同一分段调度器) | v2(测速前提) |
| `YunFs::download` | 单文件傻瓜入口(定位→取链→download 三步预制) | 阶段 2 实施 |
| `YunFs::download_dir` | 目录递归下载(`cp -r` 语义) | v2 评估(6.4) |
| 用户自由编排(文件级并发) | 库只保证 `Send + Sync` + 示例演示 | 编译期断言(阶段 2) |

**裁决规则**:YunFs 只预制"文件系统语义"(下哪个/存到哪/是否递归),**永不透传传输调优参数**——`cp` 没有"用几个线程"参数。断点/分块参数只属于 `YunApi::download_with`。

### 6.2 否决"三个平级接口",定"一个引擎 + 选项"

```rust
pub fn download(&self, dlink: &str, dst: &str) -> Result<u64, ApiError>  // 便捷入口 = offset 0 + threads 1
pub fn download_with(&self, dlink: &str, dst: &str, opts: DownloadOpts) -> Result<u64, ApiError>  // v2

pub struct DownloadOpts {  // v2
    offset: u64,    // >0 = 断点续传;响应 200(服务器忽略 Range)时自动回退从头 truncate
    threads: usize, // >1 = 分块并发;每块独立可重试 = 分块天然含断点语义
}
```

理由:resume 与分块并发是同一分段调度器的两种参数,拆独立方法导致实现重复 + 组合爆炸(并发+续传=第 4 个方法?)。**v2 前提实验**:① 百度单连接是否限速 ② dlink 并发 Range 容忍度——不限速则分块并发纯添乱,只做 resume。

### 6.3 异步下载图纸(异步 feature 落地时实施,接口镜像同步)

- 接口面与同步一致(download / download_with),文档语义共享(token 拼接/覆盖/字节数)
- 分块并发实现变廉价:`join_all` 几个 range future 即可,**无需库内置线程池**(executor 用调用者的)
- 同步做不出的形态:**Stream 化下载**(逐块 yield → 进度/限速/取消,drop future 即停)列为可选项
- 文件写入:tokio::fs 真异步 vs spawn_blocking 包 std::fs——实施时定
- 不给异步接口提前锁死形态,以届时同步接口为镜像

### 6.4 `download_dir`(v2 评估项)

- **串行 v1 先做**(若做):树遍历 + **批量取链**(`get_files_dlink_vec` 攒批,避免逐文件 filemetas 触发 API 频控 31034)
- **并发的前提实验**:① 单连接限速 ② 下载域(d.pcs 等)并发连接容忍度——API 域频控已知,下载域未测
- **并发的坑(记录,防踩)**:错误语义(串行"遇错即停报告位置"干净;并发部分成功 → 返回类型变化;若做,用"遇错即停调度"保串行语义,不加部分成功报告类型)、线程数必须封顶(否则目录 1000 文件爆连接池/句柄)、失败重试与错误聚合纠缠、并发需先全量遍历收集任务(多一轮网络)
- 倾向:**串行可能即终态**——不限速则并发只是把总带宽切碎

### 6.5 已确认小决策

1. 进度回调 v1 不做:download 返回字节数,调用方自行展示;回调形态(闭包/句柄/stream)异步图纸再议。
2. `fs.download` 定位开销:当前目录 ls + find(一次额外请求)——接受;与 chdir 在线验证语义一致。
3. `fs.download` 的 `local` 为完整本地文件路径;本地目标为目录时自动拼远端文件名 → v2(`cp` 行为)。
4. 多线程义务:库保证 `YunApi: Send + Sync`(ureq::Agent 内部 Arc 已天然满足),加编译期断言防回归;文件级并发交给用户组合。

## 七、开发步骤(TDD)

### 阶段 1:核心下载器 `YunApi::download`

| 步骤 | 内容 | 验证 |
|---|---|---|
| 1.1 | RED:URL 拼接纯函数单测(不带 token 的 dlink → 追加;已带的不重复) | 编译失败 |
| 1.2 | GREEN:拼接逻辑 + `download` 实现(流式落盘、覆盖、错误映射) | lib 测试全绿 |
| 1.3 | `util::download` 加 `#[deprecated]` 指向新 API | 编译(警告可接受) |
| 1.4 | 网络测试 `download_roundtrip`(上传→取链→下载→字节对比→清理) | 网络测试通过 |
| 1.5 | **提交**:`内部修改: 新增YunApi::download流式下载器,废弃util::download` | — |

### 阶段 2:YunFs 映射

| 步骤 | 内容 | 验证 |
|---|---|---|
| 2.0 | 编译期断言 `YunApi: Send + Sync`(支撑用户层文件级并发,防回归) | 编译 |
| 2.1 | `YunFs::download`(当前目录定位 + 取链 + 下载) | lib 测试全绿 |
| 2.2 | 网络测试 `yunfs_download` 闭环 | 网络测试通过 |
| 2.3 | README/lib.rs 示例补充下载用法 | doc 测试全绿 |
| 2.4 | **提交**:`内部修改: YunFs新增download映射,个人下载闭环完成` | — |

### 阶段 3:收尾

| 步骤 | 内容 |
|---|---|
| 3.1 | 本文档状态更新为"已实施",记录实际结果 |
| 3.2 | 提交文档 |
