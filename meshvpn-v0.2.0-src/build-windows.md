# Windows 构建指南

## 方案一：Windows 本地构建（推荐）

这是最推荐和最简单的方法！

### 前置要求

1. **安装 Rust**
   访问 https://rustup.rs/ 下载并安装 Rust 工具链

2. **安装 C++ 工具链**
   - 下载 Visual Studio Build Tools
   - 选择 "Desktop development with C++" 工作负载
   - 安装完成后重启

3. **验证安装**
   ```powershell
   rustc --version
   cargo --version
   ```

### 编译步骤

```powershell
# 1. 解压源代码
Expand-Archive meshvpn-v0.2.0-src.tar.gz

# 2. 进入项目目录
cd meshvpn-v0.2.0

# 3. 编译（生产版本）
cargo build --release

# 4. 编译完成后，可执行文件在：
target\release\meshvpn.exe
```

### 运行

```powershell
# 查看帮助
target\release\meshvpn.exe --help

# 生成配置
target\release\meshvpn.exe generate

# 查看公钥
target\release\meshvpn.exe pubkey

# 启动节点
target\release\meshvpn.exe start
```

---

## 方案二：GitHub Actions 自动化构建（可选）

如果你有 GitHub 账号，可以自动构建 Windows 版本！

创建 `.github/workflows/build.yml`：

```yaml
name: Build

on:
  push:
    tags:
      - 'v*'
  workflow_dispatch:

jobs:
  build:
    runs-on: windows-latest
    
    steps:
    - uses: actions/checkout@v2
    
    - name: Install Rust
      uses: actions-rs/toolchain@v1
      with:
        toolchain: stable
        target: x86_64-pc-windows-msvc
        profile: minimal
    
    - name: Build
      uses: actions-rs/cargo@v1
      with:
        command: build
        args: --release --target x86_64-pc-windows-msvc
    
    - name: Upload artifact
      uses: actions/upload-artifact@v2
      with:
        name: meshvpn-windows
        path: target/x86_64-pc-windows-msvc/release/meshvpn.exe
```

---

## 方案三：Linux 交叉编译（高级）

虽然比较复杂，但如果你在 Linux 环境下，可以这样：

### 安装工具链

```bash
# 1. 添加 Windows 编译目标
rustup target add x86_64-pc-windows-gnu

# 2. 安装 MinGW
apt-get install -y mingw-w64
```

### 编译

```bash
# 配置 Cargo 以使用 MinGW 链接器
# 编辑 ~/.cargo/config.toml:
# [target.x86_64-pc-windows-gnu]
# linker = "x86_64-w64-mingw32-gcc"
# rustflags = ["-C", "link-arg=-mwindows"]

# 编译
cargo build --release --target x86_64-pc-windows-gnu
```

---

## 打包分发

编译成功后，你可以这样打包：

```powershell
# 创建发布目录
mkdir -p meshvpn-windows
cp target/release/meshvpn.exe meshvpn-windows/
cp README.md meshvpn-windows/

# 打包
Compress-Archive -Path meshvpn-windows -DestinationPath meshvpn-windows.zip
```

---

## 常见问题

### Q: 遇到链接错误？
A: 确保安装了 Visual Studio Build Tools 的 C++ 组件。

### Q: 运行时提示缺少 DLL？
A: 使用静态编译（需要修改一些配置），或者打包时带上必需的 DLL。

### Q: Windows 防火墙提示？
A: 首次运行时，允许应用通过防火墙（UDP 51820 端口）。
