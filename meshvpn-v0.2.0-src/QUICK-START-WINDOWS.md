# ⚡ Windows 超快速编译指南 - 5 分钟搞定！

## 🎯 目标
在 Windows 上编译出 meshvpn.exe，**尽量少安装东西**！

---

## 📥 第 1 步：下载并安装 Rust (2分钟)

这是必须的，但很简单！

1. **访问：https://rustup.rs/**
2. **下载 64-bit 安装程序** (rustup-init.exe)
3. **运行安装程序**
   - 看到 "1) Proceed with installation (default)"
   - **直接按回车！**
   - 等它完成

---

## 🛠️ 第 2 步：安装 C++ 工具 (2分钟)

这也是必须的，但我们可以用最简化的方式安装！

### 方法 A：推荐 - 下载并安装 (最简单)

1. **访问：https://visualstudio.microsoft.com/visual-cpp-build-tools/**
2. **点击 "Download Build Tools"**
3. **运行下载的安装程序**
4. **选择 "Desktop development with C++"** (只选这个！)
5. **点击右下角 "Install"** (需要下载约 1GB)
6. **安装完后重启电脑** (重要！)

---

## 🚀 第 3 步：编译！(1分钟)

1. **解压我们的源代码包** `meshvpn-v0.2.0-src.tar.gz`
2. **打开 PowerShell** (在开始菜单搜 "PowerShell")
3. **进入项目目录**，例如：
   ```powershell
   cd C:\Users\你的用户名\Downloads\meshvpn-v0.2.0
   ```
4. **编译！**
   ```powershell
   cargo build --release
   ```
5. **完成！** 你的 exe 在：
   ```
   target\release\meshvpn.exe
   ```

---

## 💡 就是这么简单！

| 步骤 | 时间 | 操作 |
|------|------|------|
| 1 | 2分钟 | 安装 Rust |
| 2 | 2分钟 | 安装 C++ 工具 |
| 3 | 1分钟 | 运行 cargo build |

---

## ⚠️ 可能遇到的问题

**问题：提示 `cargo` 不是命令？**
> 解决：重启电脑，或者重新打开 PowerShell

**问题：编译出错？**
> 解决：确认 C++ 工具安装完了，并且重启了电脑

---

## 🎉 编译成功后！

运行你的 exe：
```powershell
.\target\release\meshvpn.exe --help
```

就这么简单！
