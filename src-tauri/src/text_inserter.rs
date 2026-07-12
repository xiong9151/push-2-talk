// 文本插入模块
// 使用 Win32 SendInput 的 KEYEVENTF_UNICODE 直接输入文本
// 完全不经过剪贴板，避免剪贴板内容被覆盖
use anyhow::Result;
use std::thread;
use std::time::Duration;

use crate::win32_input;

pub struct TextInserter;

impl TextInserter {
    pub fn new() -> Result<Self> {
        Ok(Self)
    }

    pub fn insert_text(&mut self, text: &str) -> Result<()> {
        tracing::info!("准备插入文本: {}", text);

        // 使用 Win32 SendInput 的 KEYEVENTF_UNICODE 直接输入
        // 不经过剪贴板，无任何副作用
        win32_input::send_unicode_text(text)?;

        // 等待输入完成
        thread::sleep(Duration::from_millis(50));

        tracing::info!("Unicode 文本输入完成");
        Ok(())
    }
}