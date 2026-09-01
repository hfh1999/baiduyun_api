# baiduyun_api

百度网盘开放平台的 Rust 封装——读写、搜索、上传一库搞定，错误信息直透百度真实原因。

[![github](https://img.shields.io/badge/github-hfh1999%2Fbaiduyun__api-blue)](https://github.com/hfh1999/baiduyun_api/)
[![crates.io](https://img.shields.io/badge/crates.io-0.3.0-green)](https://crates.io/crates/baiduyun_api)
[![docs.rs](https://img.shields.io/badge/docs.rs-baiduyun__api-orange)](https://docs.rs/baiduyun_api/)

## 特性

- **完整的读写能力**：用户信息、空间配额、文件列表、文件信息、下载链接、关键词搜索
- **写操作**：创建文件夹、删除、移动、复制、重命名、单步上传（≤2GB）
- **YunFs**：类本地文件系统的抽象——`pwd`/`chdir`/`ls`/`mkdir`/`rm`/`mv`/`cp`/`upload`，支持相对路径
- **错误直透**：百度返回的 `errno` + `errmsg` 原样传递，不再有"谜之错误文案"
- **零 panic 设计**：网络、解析、格式异常一律返回 `Result`，不崩溃
- **开箱即用**：授权工具自动获取 token、演示 CLI 覆盖全部常用操作

## 快速开始

```toml
[dependencies]
baiduyun_api = "0.3"
```

```rust
use baiduyun_api::YunApi;

fn main() -> Result<(), baiduyun_api::ApiError> {
    let api = YunApi::new("你的access_token");
    let user = api.get_user_info()?;
    println!("百度账号: {}", user.baidu_name);
    Ok(())
}
```

## 使用示例

### 列出目录内容

```rust
let list = api.get_files_list("/apps", 0, 100)?;
for file in list {
    println!("{}  {}B", file.server_filename, file.size);
}
```

### 搜索文件

```rust
// 递归搜索根目录,支持中文关键字
let results = api.search_with_key("唱戏机", "/", true, 1, 100, false)?;
for item in results {
    println!("{} -> {}", item.server_filename, item.path);
}
```

### 上传文件

```rust
use baiduyun_api::OnDup;

// 注意: 上传路径必须位于 /apps/{你的应用名}/ 下(百度限制)
let result = api.upload("./photo.jpg", "/apps/myapp/photo.jpg", OnDup::Fail)?;
println!("上传成功: {}", result.path);
```

### 用 YunFs 像操作本地文件系统一样

```rust
use baiduyun_api::{util, YunApi};

let mut fs = util::YunFs::new(&api);
fs.chdir("学习资料/")?;
fs.mkdir("新目录")?;        // 相对路径自动解析
fs.upload("./a.txt", "a.txt")?;
for item in fs.ls()? {
    println!("{}", item.server_filename);
}
fs.rm("a.txt")?;
```

## 获取 access_token

### 方式一：授权工具(推荐)

```bash
cargo run --example authorize -- --app-key=你的APP_KEY
```

自动打开浏览器完成授权，token 自动写入 `.env`（已被 gitignore，不会提交）。
前提：在 [百度网盘开放平台](https://pan.baidu.com/union) 创建应用获取 App Key。

### 方式二：手动

1. 浏览器访问 `https://openapi.baidu.com/oauth/2.0/authorize?response_type=token&client_id=你的APP_KEY&redirect_uri=oob&scope=netdisk`
2. 授权后地址栏会显示 `...login_success#access_token=xxx...`，复制 `access_token` 的值

token 有效期 30 天，持续使用不会过期。

## 演示 CLI

先确保 `.env` 中有 `BAIDU_ACCESS_TOKEN`（可用授权工具生成），再执行：

```bash
cargo run --example cli -- ls /          # 列出根目录
cargo run --example cli -- search 唱戏机
cargo run --example cli -- upload ./a.jpg /apps/你的应用名/a.jpg
```

输出示例：

```text
名称                        大小      修改时间(UTC)        类型
apps                      -         2026-09-01 07:59    目录
毕业/                      -         2020-06-07 14:04    目录
毕业.rar                   90 MB    2020-06-07 14:04    文件
```

> Git Bash 用户注意：`/` 开头的参数会被 MSYS 转换成 Windows 路径，请加 `MSYS_NO_PATHCONV=1` 前缀运行。

## API 稳定性

从 **0.3.0 开始 API 稳定**：之后只增加新接口，不会变动已有接口的签名和行为。
所有模型字段均经真实网络响应验证。

## 已知限制

- 上传路径必须位于 `/apps/{你的应用名}/` 下（百度接口限制，非本库可解）
- 单步上传文件上限 2GB（更大文件需分片上传，规划中）
- 文件列表单次最多 1000 条，`YunFs::ls` 自动翻页（大目录耗时较长）

## Todo

- [ ] 大文件分片上传（预上传/分片/创建文件）
- [ ] 分享服务（创建分享链接/提取码/转存）
- [ ] 异步 API（feature 切换，同步 API 不受影响）

## License

MIT
