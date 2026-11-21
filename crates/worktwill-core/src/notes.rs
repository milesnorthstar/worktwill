use crate::{model::*, store::now, Store};
use rusqlite::{params, OptionalExtension, TransactionBehavior};

impl Store {
    fn notes_scope(&self) -> String {
        format!(
            "{}:{}",
            self.session,
            self.scan_epoch.load(std::sync::atomic::Ordering::SeqCst)
        )
    }
    pub fn notes_state(&self) -> Result<NotesState> {
        let before = self.notes_scope();
        let items = self.notes()?;
        if before != self.notes_scope() {
            return Err(AppError::new(
                "conflict",
                "数据已恢复，请重新读取便签",
                "Data was restored. Reload notes.",
            ));
        }
        Ok(NotesState {
            scope: before,
            items,
        })
    }

    pub fn notes(&self) -> Result<Vec<ScratchNote>> {
        let _guard = self.gate.read().map_err(|_| {
            AppError::new(
                "internal_error",
                "状态锁不可用",
                "App state is unavailable.",
            )
        })?;
        let db = self.connection()?;
        let notes = db.prepare("SELECT id,text,pinned,version,updated_at FROM scratch_notes WHERE deleted=0 ORDER BY pinned DESC,updated_at DESC,id")?.query_map([], |r| Ok(ScratchNote { id:r.get(0)?,text:r.get(1)?,pinned:r.get(2)?,version:r.get(3)?,updated_at:r.get(4)? }))?.collect::<rusqlite::Result<_>>()?;
        Ok(notes)
    }

    pub fn write_note(&self, request: NoteWrite) -> Result<ScratchNote> {
        if request.expected_version == u32::MAX
            || uuid::Uuid::parse_str(&request.id).is_err()
            || request.text.chars().count() > 20000
        {
            return Err(AppError::new(
                "invalid_input",
                "便签格式无效或超过 20000 字",
                "Invalid note or more than 20,000 characters.",
            ));
        }
        let _guard = self.gate.read().map_err(|_| {
            AppError::new(
                "internal_error",
                "状态锁不可用",
                "App state is unavailable.",
            )
        })?;
        if request.scope != self.notes_scope() {
            return Err(AppError::new(
                "conflict",
                "数据会话已变化，草稿尚未保存",
                "Data session changed. The draft is not saved.",
            ));
        }
        let mut db = self.connection()?;
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let old: Option<(String, bool, u32, bool, f64)> = tx
            .query_row(
                "SELECT text,pinned,version,deleted,updated_at FROM scratch_notes WHERE id=?",
                [&request.id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
            )
            .optional()?;
        if let Some((text, pinned, version, deleted, updated_at)) = &old {
            // 同一快照的重试返回原回执；其他旧写入明确冲突，墓碑不可复活。
            if *version == request.expected_version + 1
                && *text == request.text
                && *pinned == request.pinned
                && *deleted == request.deleted
            {
                return Ok(ScratchNote {
                    id: request.id,
                    text: request.text,
                    pinned: request.pinned,
                    version: *version,
                    updated_at: *updated_at,
                });
            }
            if *deleted || *version != request.expected_version {
                return Err(AppError::new(
                    "conflict",
                    "便签已变化，请保留草稿并重新读取",
                    "The note changed. Keep your draft and reload.",
                ));
            }
        } else if request.expected_version != 0 {
            return Err(AppError::new(
                "conflict",
                "便签版本已失效，请重新读取",
                "The note version expired. Reload notes.",
            ));
        }
        let note = ScratchNote {
            id: request.id,
            text: request.text,
            pinned: request.pinned,
            version: request.expected_version + 1,
            updated_at: now(),
        };
        tx.execute("INSERT INTO scratch_notes(id,text,pinned,version,updated_at,deleted) VALUES(?1,?2,?3,?4,?5,?6) ON CONFLICT(id) DO UPDATE SET text=excluded.text,pinned=excluded.pinned,version=excluded.version,updated_at=excluded.updated_at,deleted=excluded.deleted",params![note.id,note.text,note.pinned,note.version,note.updated_at,request.deleted])?;
        // 便签不进入资源索引、活动或 catalog revision，避免无关搜索游标失效。
        tx.commit()?;
        Ok(note)
    }
}
