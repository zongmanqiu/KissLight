# KissLight 测试指南

## 测试框架

使用 Rust 内置的 `#[test]` 和 `cargo test`。

## 运行测试

```powershell
# 运行全部测试
cargo test

# 运行特定测试
cargo test test_name

# 显示测试输出
cargo test -- --nocapture
```

## 测试分类

### 单元测试

位于各模块文件内，使用 `#[cfg(test)]` 块。

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_something() {
        assert_eq!(1 + 1, 2);
    }
}
```

### 集成测试

位于 `tests/` 目录，测试公开 API。

## 测试策略

1. **核心逻辑**：测试 `core/` 模块的纯函数
2. **配置读写**：测试配置文件的加载和保存
3. **工具函数**：测试 UTF-8/UTF-16 转换

## 注意事项

- `ui/` 模块涉及 GUI，难以自动化测试，以手动测试为主
- 测试时不要修改系统注册表
- 测试时不要修改系统 Gamma 设置
