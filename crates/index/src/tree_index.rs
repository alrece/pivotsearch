//! tree_index：SQLite 持久化已索引文件树状态。
//!
//! 替代经典桌面搜索工具的 Java 序列化方案——SQLite 跨版本稳定、可查询、可恢复。
//! 这是增量算法的状态基础：记录每个已索引文件的 path/mtime/parser，
//! 增量时比对磁盘 mtime 决定新增/修改/删除。

use pivotsearch_contracts::{IndexId, PivotsearchError, Result, Uid};
use rusqlite::{params, Connection};
use std::path::Path;

/// SQLite 持久化的文件树状态。
pub struct TreeIndex {
    conn: Connection,
}

/// 一条已索引文件记录。
#[derive(Debug, Clone)]
pub struct IndexedFile {
    pub uid: Uid,
    pub path: String,
    pub mtime: i64,
    pub parser: Option<String>,
    pub index_id: IndexId,
}

/// 一个索引根的元信息。
#[derive(Debug, Clone)]
pub struct IndexRoot {
    pub id: IndexId,
    pub path: String,
    pub display_name: Option<String>,
    pub created_at: i64,
}

impl TreeIndex {
    /// 打开（或创建）SQLite 数据库。
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path).map_err(|e| PivotsearchError::Sqlite(e.to_string()))?;
        Self::init_schema(&conn)?;
        Ok(Self { conn })
    }

    /// 内存数据库（测试用）。
    #[cfg(test)]
    pub fn open_memory() -> Result<Self> {
        let conn = Connection::open_in_memory().map_err(|e| PivotsearchError::Sqlite(e.to_string()))?;
        Self::init_schema(&conn)?;
        Ok(Self { conn })
    }

    fn init_schema(conn: &Connection) -> Result<()> {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS index_roots (
                id           TEXT PRIMARY KEY,
                path         TEXT NOT NULL UNIQUE,
                display_name TEXT,
                created_at   INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS indexed_files (
                uid       TEXT PRIMARY KEY,
                path      TEXT NOT NULL,
                mtime     INTEGER NOT NULL,
                parser    TEXT,
                index_id  TEXT NOT NULL,
                FOREIGN KEY (index_id) REFERENCES index_roots(id)
            );
            CREATE INDEX IF NOT EXISTS idx_files_index_id ON indexed_files(index_id);
            CREATE INDEX IF NOT EXISTS idx_files_path ON indexed_files(path);",
        )
        .map_err(|e| PivotsearchError::Sqlite(e.to_string()))?;
        Ok(())
    }

    // ── index_roots 管理 ──

    /// 添加一个索引根。
    pub fn add_index_root(
        &self,
        id: &str,
        path: &str,
        display_name: Option<&str>,
        created_at: i64,
    ) -> Result<()> {
        self.conn
            .execute(
                "INSERT OR IGNORE INTO index_roots (id, path, display_name, created_at) VALUES (?1, ?2, ?3, ?4)",
                params![id, path, display_name, created_at],
            )
            .map_err(|e| PivotsearchError::Sqlite(e.to_string()))?;
        Ok(())
    }

    /// 列出所有索引根。
    pub fn list_index_roots(&self) -> Result<Vec<IndexRoot>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, path, display_name, created_at FROM index_roots")
            .map_err(|e| PivotsearchError::Sqlite(e.to_string()))?;
        let rows = stmt
            .query_map([], |row| {
                Ok(IndexRoot {
                    id: row.get(0)?,
                    path: row.get(1)?,
                    display_name: row.get(2)?,
                    created_at: row.get(3)?,
                })
            })
            .map_err(|e| PivotsearchError::Sqlite(e.to_string()))?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| PivotsearchError::Sqlite(e.to_string()))
    }

    /// 移除索引根（同时级联删除其下所有文件记录）。
    pub fn remove_index_root(&self, id: &str) -> Result<()> {
        self.conn
            .execute("DELETE FROM indexed_files WHERE index_id = ?1", params![id])
            .map_err(|e| PivotsearchError::Sqlite(e.to_string()))?;
        self.conn
            .execute("DELETE FROM index_roots WHERE id = ?1", params![id])
            .map_err(|e| PivotsearchError::Sqlite(e.to_string()))?;
        Ok(())
    }

    // ── indexed_files 管理 ──

    /// 取某索引根下所有已索引文件（供增量算法 unseenDocs 用）。
    pub fn files_for_index(&self, index_id: &str) -> Result<Vec<IndexedFile>> {
        let mut stmt = self
            .conn
            .prepare("SELECT uid, path, mtime, parser, index_id FROM indexed_files WHERE index_id = ?1")
            .map_err(|e| PivotsearchError::Sqlite(e.to_string()))?;
        let rows = stmt
            .query_map(params![index_id], |row| {
                Ok(IndexedFile {
                    uid: row.get(0)?,
                    path: row.get(1)?,
                    mtime: row.get(2)?,
                    parser: row.get(3)?,
                    index_id: row.get(4)?,
                })
            })
            .map_err(|e| PivotsearchError::Sqlite(e.to_string()))?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| PivotsearchError::Sqlite(e.to_string()))
    }

    /// upsert 一条文件记录（新增或更新 mtime/parser）。
    pub fn upsert_file(&self, file: &IndexedFile) -> Result<()> {
        self.conn
            .execute(
                "INSERT INTO indexed_files (uid, path, mtime, parser, index_id) VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(uid) DO UPDATE SET mtime=excluded.mtime, parser=excluded.parser",
                params![file.uid, file.path, file.mtime, file.parser, file.index_id],
            )
            .map_err(|e| PivotsearchError::Sqlite(e.to_string()))?;
        Ok(())
    }

    /// 按 uid 删除一条文件记录。
    pub fn delete_file(&self, uid: &str) -> Result<()> {
        self.conn
            .execute("DELETE FROM indexed_files WHERE uid = ?1", params![uid])
            .map_err(|e| PivotsearchError::Sqlite(e.to_string()))?;
        Ok(())
    }

    /// 按 uid 查询单条（mtime 二次校验用）。
    pub fn get_file(&self, uid: &str) -> Result<Option<IndexedFile>> {
        let mut stmt = self
            .conn
            .prepare("SELECT uid, path, mtime, parser, index_id FROM indexed_files WHERE uid = ?1")
            .map_err(|e| PivotsearchError::Sqlite(e.to_string()))?;
        let mut rows = stmt
            .query_map(params![uid], |row| {
                Ok(IndexedFile {
                    uid: row.get(0)?,
                    path: row.get(1)?,
                    mtime: row.get(2)?,
                    parser: row.get(3)?,
                    index_id: row.get(4)?,
                })
            })
            .map_err(|e| PivotsearchError::Sqlite(e.to_string()))?;
        rows.next()
            .transpose()
            .map_err(|e| PivotsearchError::Sqlite(e.to_string()))
    }

    /// 统计某索引根的文件数（状态展示用）。
    pub fn count_files(&self, index_id: &str) -> Result<u64> {
        self.conn
            .query_row(
                "SELECT COUNT(*) FROM indexed_files WHERE index_id = ?1",
                params![index_id],
                |row| row.get::<_, i64>(0),
            )
            .map(|n| n as u64)
            .map_err(|e| PivotsearchError::Sqlite(e.to_string()))
    }

    /// 按 parser 类型分组统计文件数（索引详情用）。
    /// 返回 Vec<(parser_name_or_"未知", count)>，按 count 降序。
    pub fn stats_by_parser(&self, index_id: &str) -> Result<Vec<(String, u64)>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT parser, COUNT(*) as cnt FROM indexed_files WHERE index_id = ?1 GROUP BY parser ORDER BY cnt DESC",
            )
            .map_err(|e| PivotsearchError::Sqlite(e.to_string()))?;
        let rows = stmt
            .query_map(params![index_id], |row| {
                let parser: Option<String> = row.get(0)?;
                let count: i64 = row.get(1)?;
                let name = parser.unwrap_or_else(|| "未解析/不支持".to_string());
                Ok((name, count as u64))
            })
            .map_err(|e| PivotsearchError::Sqlite(e.to_string()))?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| PivotsearchError::Sqlite(e.to_string()))
    }

    /// 取最近修改的文件（按 mtime 降序，限 N 条）。
    pub fn recent_files(&self, index_id: &str, limit: u64) -> Result<Vec<IndexedFile>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT uid, path, mtime, parser, index_id FROM indexed_files WHERE index_id = ?1 ORDER BY mtime DESC LIMIT ?2",
            )
            .map_err(|e| PivotsearchError::Sqlite(e.to_string()))?;
        let rows = stmt
            .query_map(params![index_id, limit as i64], |row| {
                Ok(IndexedFile {
                    uid: row.get(0)?,
                    path: row.get(1)?,
                    mtime: row.get(2)?,
                    parser: row.get(3)?,
                    index_id: row.get(4)?,
                })
            })
            .map_err(|e| PivotsearchError::Sqlite(e.to_string()))?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| PivotsearchError::Sqlite(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_and_init_schema() {
        let ti = TreeIndex::open_memory().unwrap();
        // 表存在
        let count: i64 = ti
            .conn
            .query_row("SELECT COUNT(*) FROM index_roots", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn add_and_list_index_roots() {
        let ti = TreeIndex::open_memory().unwrap();
        ti.add_index_root("idx-1", "/home/foo/docs", Some("我的文档"), 1000).unwrap();
        ti.add_index_root("idx-2", "/home/bar/notes", None, 2000).unwrap();

        let roots = ti.list_index_roots().unwrap();
        assert_eq!(roots.len(), 2);
        assert_eq!(roots[0].id, "idx-1");
        assert_eq!(roots[0].display_name.as_deref(), Some("我的文档"));
    }

    #[test]
    fn upsert_and_get_file() {
        let ti = TreeIndex::open_memory().unwrap();
        ti.add_index_root("idx-1", "/docs", None, 1000).unwrap();

        let file = IndexedFile {
            uid: "file:///docs/a.txt".to_string(),
            path: "/docs/a.txt".to_string(),
            mtime: 5000,
            parser: Some("TextParser".to_string()),
            index_id: "idx-1".to_string(),
        };
        ti.upsert_file(&file).unwrap();

        let got = ti.get_file("file:///docs/a.txt").unwrap().unwrap();
        assert_eq!(got.mtime, 5000);
        assert_eq!(got.parser.as_deref(), Some("TextParser"));

        // upsert 更新 mtime
        let mut file2 = file.clone();
        file2.mtime = 6000;
        ti.upsert_file(&file2).unwrap();
        let got2 = ti.get_file("file:///docs/a.txt").unwrap().unwrap();
        assert_eq!(got2.mtime, 6000);
    }

    #[test]
    fn delete_file_and_remove_index_cascade() {
        let ti = TreeIndex::open_memory().unwrap();
        ti.add_index_root("idx-1", "/docs", None, 1000).unwrap();
        ti.upsert_file(&IndexedFile {
            uid: "file:///docs/a.txt".to_string(),
            path: "/docs/a.txt".to_string(),
            mtime: 5000,
            parser: None,
            index_id: "idx-1".to_string(),
        }).unwrap();
        assert_eq!(ti.count_files("idx-1").unwrap(), 1);

        // 删除单文件
        ti.delete_file("file:///docs/a.txt").unwrap();
        assert_eq!(ti.count_files("idx-1").unwrap(), 0);

        // 再加一个，然后级联删索引根
        ti.upsert_file(&IndexedFile {
            uid: "file:///docs/b.txt".to_string(),
            path: "/docs/b.txt".to_string(),
            mtime: 5000,
            parser: None,
            index_id: "idx-1".to_string(),
        }).unwrap();
        ti.remove_index_root("idx-1").unwrap();
        assert_eq!(ti.count_files("idx-1").unwrap(), 0);
        assert!(ti.list_index_roots().unwrap().is_empty());
    }
}
