//! Parser trait + ParseResult + ParserRegistry。

use crate::error::Result;
use crate::types::Uid;
use std::path::Path;

/// 单个文件解析的结果（纯数据结构）。
///
/// 解析层与写入层解耦：Parser 只产出本结构，由 index crate 组装 Tantivy Document。
#[derive(Debug, Clone, Default)]
pub struct ParseResult {
    /// 正文纯文本（必需，可为空——如纯图片 PDF 无文字层）。
    pub content: String,
    /// 标题（无则由 index crate 退化为去扩展名文件名）。
    pub title: Option<String>,
    /// 作者列表。
    pub authors: Vec<String>,
    /// 其他元数据（Subject/Keywords 等），拼接到 content 一起索引。
    pub misc_metadata: Vec<String>,
    /// 解析器名（由 ParserRegistry 注入，而非 Parser 自设）。
    pub parser_name: &'static str,
}

impl ParseResult {
    pub fn new(content: String) -> Self {
        Self {
            content,
            title: None,
            authors: Vec::new(),
            misc_metadata: Vec::new(),
            parser_name: "",
        }
    }
}

/// 文件解析器 trait。
///
/// 每种格式实现一个 Parser，注册到 ParserRegistry。
/// 选择策略：mime 优先（魔数检测）→ 扩展名 fallback → 多 parser 容错尝试。
pub trait Parser: Send + Sync {
    /// 该 parser 处理的扩展名（小写，无点），如 ["pdf"]。
    fn extensions(&self) -> &[&str];

    /// 该 parser 声明的 mime 类型，如 ["application/pdf"]。
    fn mimes(&self) -> &[&str];

    /// 解析单个文件，产出纯文本结果。
    fn parse(&self, path: &Path) -> Result<ParseResult>;

    /// parser 名（用于 ParseResult.parser_name 注入和索引字段）。
    fn name(&self) -> &'static str;
}

/// Parser 注册表的抽象（具体实现在 parser crate）。
/// 通过此 trait 让 core 编排层不依赖具体实现。
pub trait ParserRegistry: Send + Sync {
    /// 按两级策略选择 parser 并解析。
    /// 1. mime 检测命中 → 按匹配度排序依次尝试（容错）
    /// 2. 扩展名 fallback → 精确匹配第一个
    /// 3. 兜底 → UnsupportedFormat 或仅索引文件名
    fn parse(&self, path: &Path) -> Result<ParseResult>;

    /// 判断扩展名是否可被任一 parser 处理（watcher 事件过滤用）。
    fn can_parse_by_name(&self, file_name: &str) -> bool;

    /// 列出所有已注册 parser 的名（调试/设置页用）。
    fn list_parser_names(&self) -> Vec<&'static str>;
}

/// 内部使用的 UID 提取（从 uid 反推 path）。
pub fn extract_path_from_uid(uid: &Uid) -> Option<&str> {
    uid.strip_prefix("file://")
}
