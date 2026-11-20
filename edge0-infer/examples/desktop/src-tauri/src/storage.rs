use serde::Serialize;
use serde_json::Value;
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    sync::Mutex,
};

pub struct Store {
    pub config: PathBuf,
    pub data: PathBuf,
    lock: Mutex<()>,
}
#[derive(Serialize)]
pub struct Archive {
    pub id: String,
    pub conversation: Option<Value>,
    pub error: Option<String>,
}
impl Store {
    pub fn new(config: PathBuf, data: PathBuf) -> Self {
        Self {
            config,
            data,
            lock: Mutex::new(()),
        }
    }
    pub fn list(&self) -> Result<Vec<Archive>, String> {
        let _guard = self.lock.lock().map_err(|_| "存储锁定失败")?;
        let directory = self.data.join("conversations");
        if !directory.exists() {
            return Ok(vec![]);
        }
        let mut archives = vec![];
        for entry in fs::read_dir(directory).map_err(|_| "无法列出对话")? {
            let path = entry.map_err(|_| "无法读取对话条目")?.path();
            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }
            let Some(id) = path
                .file_stem()
                .and_then(|s| s.to_str())
                .filter(|s| valid_id(s))
            else {
                continue;
            };
            let result = fs::read(&path)
                .map_err(|_| "无法读取存档")
                .and_then(|bytes| {
                    serde_json::from_slice::<Value>(&bytes).map_err(|_| "存档 JSON 无效")
                })
                .and_then(|value| {
                    validate_conversation(id, &value)?;
                    Ok(value)
                });
            archives.push(match result {
                Ok(value) => Archive {
                    id: id.into(),
                    conversation: Some(value),
                    error: None,
                },
                Err(error) => Archive {
                    id: id.into(),
                    conversation: None,
                    error: Some(error.into()),
                },
            });
        }
        Ok(archives)
    }
    pub fn save(&self, id: &str, conversation: &Value) -> Result<(), String> {
        let _guard = self.lock.lock().map_err(|_| "存储锁定失败")?;
        validate_conversation(id, conversation)?;
        let path = self.data.join("conversations").join(format!("{id}.json"));
        if path.exists() {
            let old: Value = serde_json::from_slice(
                &fs::read(&path).map_err(|_| "无法读取已有存档")?,
            )
            .map_err(|_| "无法覆盖一个不可读的存档")?;
            validate_conversation(id, &old).map_err(|_| "无法覆盖一个已损坏的存档")?;
        }
        atomic_json(&path, conversation)
    }
    pub fn delete(&self, id: &str) -> Result<(), String> {
        let _guard = self.lock.lock().map_err(|_| "存储锁定失败")?;
        if !valid_id(id) {
            return Err("对话标识无效".into());
        }
        let path = self.data.join("conversations").join(format!("{id}.json"));
        if path.exists() {
            fs::remove_file(path).map_err(|_| "无法删除对话")?;
        }
        Ok(())
    }
    pub fn load_totals(&self) -> Result<Option<Value>, String> {
        let _guard = self.lock.lock().map_err(|_| "存储锁定失败")?;
        let path = self.data.join("totals.json");
        if !path.exists() {
            return Ok(None);
        }
        serde_json::from_slice(&fs::read(path).map_err(|_| "无法读取统计")?)
            .map(Some)
            .map_err(|_| "无法读取已保存的统计".into())
    }
    pub fn save_totals(&self, totals: &Value) -> Result<(), String> {
        let _guard = self.lock.lock().map_err(|_| "存储锁定失败")?;
        atomic_json(&self.data.join("totals.json"), totals)
    }
}
fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 80 && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
}
fn validate_conversation(id: &str, value: &Value) -> Result<(), &'static str> {
    if !valid_id(id)
        || value.get("id").and_then(Value::as_str) != Some(id)
        || !value.get("createdAt").is_some_and(Value::is_number)
        || !value.get("updatedAt").is_some_and(Value::is_number)
        || !value.get("systemPrompt").is_some_and(Value::is_string)
        || !value.get("thinking").is_some_and(Value::is_boolean)
        || !value
            .get("messages")
            .and_then(Value::as_array)
            .is_some_and(|messages| {
                messages.iter().all(|m| {
                    matches!(
                        m.get("role").and_then(Value::as_str),
                        Some("user" | "assistant")
                    ) && m.get("content").is_some_and(Value::is_string)
                        && m.get("reasoning_content").is_none_or(Value::is_string)
                        && m.get("reasoningExpanded").is_none_or(Value::is_boolean)
                        && m.get("stopped").is_none_or(Value::is_boolean)
                        && m.get("metrics").is_none_or(|metrics| {
                            metrics.as_object().is_some_and(|fields| {
                                fields
                                    .values()
                                    .all(|v| v.as_f64().is_some_and(|n| n.is_finite() && n >= 0.0))
                            })
                        })
                })
            })
    {
        return Err("对话存档无效");
    }
    Ok(())
}
pub fn atomic_json(path: &Path, value: &impl Serialize) -> Result<(), String> {
    let parent = path.parent().ok_or("存储位置无效")?;
    fs::create_dir_all(parent).map_err(|_| "无法创建存储目录")?;
    let temporary = path.with_extension("new");
    let mut file = fs::File::create(&temporary).map_err(|_| "无法创建临时存档")?;
    file.write_all(&serde_json::to_vec_pretty(value).map_err(|_| "无法编码存档")?)
        .map_err(|_| "无法写入存档")?;
    file.sync_all().map_err(|_| "无法刷新存档到磁盘")?;
    drop(file);
    // std::fs::rename replaces the destination atomically on supported desktop filesystems.
    fs::rename(&temporary, path).map_err(|_| "无法替换存档".into())
}
pub fn valid_token(token: &str) -> bool {
    token.len() == 49
        && token.starts_with("e0_sk_")
        && token[6..]
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}
pub fn write_token(directory: &Path, token: &str) -> Result<(), String> {
    if !valid_token(token) {
        return Err("API 令牌格式无效".into());
    }
    fs::create_dir_all(directory).map_err(|_| "无法创建令牌目录")?;
    let path = directory.join("api-token.new");
    let mut options = fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
        options.mode(0o600);
        if path.exists() {
            fs::set_permissions(&path, fs::Permissions::from_mode(0o600))
                .map_err(|_| "无法保护令牌文件")?;
        }
    }
    let mut file = options
        .open(&path)
        .map_err(|_| "无法创建令牌文件")?;
    file.write_all(token.as_bytes())
        .and_then(|_| file.sync_all())
        .map_err(|_| "无法写入令牌文件")?;
    drop(file);
    fs::rename(path, directory.join("api-token")).map_err(|_| "无法替换令牌文件".into())
}
pub fn read_token(directory: &Path) -> Result<Option<String>, String> {
    let path = directory.join("api-token");
    if !path.exists() {
        return Ok(None);
    }
    let token = fs::read_to_string(path).map_err(|_| "无法读取 API 令牌")?;
    if !valid_token(&token) {
        return Err("已保存的 API 令牌无效".into());
    }
    Ok(Some(token))
}
#[cfg(test)]
mod tests {
    use super::*;
    fn directory() -> PathBuf {
        std::env::temp_dir().join(format!(
            "edge0-storage-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ))
    }
    #[test]
    fn archive_roundtrip_and_damage_preservation() {
        let directory = directory();
        let store = Store::new(directory.join("config"), directory.clone());
        let value = serde_json::json!({"id":"test", "createdAt":1,"updatedAt":2,"systemPrompt":"fixed","thinking":true,"messages":[{"role":"assistant","content":"reply","reasoning_content":"thought","metrics":{"input_tokens":4}}]});
        store.save("test", &value).unwrap();
        assert_eq!(store.list().unwrap()[0].conversation.as_ref(), Some(&value));
        store.save("test", &value).unwrap();
        let path = directory.join("conversations/test.json");
        fs::write(&path, b"broken").unwrap();
        assert!(store.list().unwrap()[0].error.is_some());
        assert!(store.save("test", &value).is_err());
        assert_eq!(fs::read(&path).unwrap(), b"broken");
        assert!(store.delete("../test").is_err());
        store.delete("test").unwrap();
        assert!(store.list().unwrap().is_empty());
        fs::remove_dir_all(directory).unwrap();
    }
    #[test]
    fn token_format_permissions_and_replacement() {
        let directory = directory();
        let token = format!("e0_sk_{}", "A".repeat(43));
        assert!(valid_token(&token));
        assert!(!valid_token("e0_sk_short"));
        assert!(!valid_token(&format!("e0_sk_{}", "+".repeat(43))));
        write_token(&directory, &token).unwrap();
        assert_eq!(read_token(&directory).unwrap(), Some(token.clone()));
        write_token(&directory, &token).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(directory.join("api-token"))
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
        }
        fs::remove_dir_all(directory).unwrap();
    }
}
