//! Set one entry in someone else's JSON file and change nothing else: not the
//! order of keys, the spacing, or the settings ByteHive does not own. A file
//! that is not plain JSON (comments, say) is left alone. The result is parsed
//! back and must differ from the old file in that one entry only, or it is
//! not written.

use serde_json::Value;

/// `text` with `root[key][name]` set to `value`; None when it already is.
pub fn set_member(text: &str, key: &str, name: &str, value: &Value) -> Result<Option<String>, String> {
    let (bom, body) = match text.strip_prefix('\u{feff}') {
        Some(rest) => ("\u{feff}", rest),
        None => ("", text),
    };
    let old: Value = serde_json::from_str(body).map_err(|e| format!("not plain JSON: {e}"))?;
    if !old.is_object() {
        return Err("not a JSON object".into());
    }
    if old.get(key).and_then(|m| m.get(name)) == Some(value) {
        return Ok(None);
    }
    let root = body.find('{').ok_or("no object")?;
    let new = match find(body, root, key)? {
        Some((_, start, _)) => {
            if !body[start..].starts_with('{') {
                return Err(format!("{key} is not an object"));
            }
            match find(body, start, name)? {
                Some((line_start, vs, ve)) => {
                    let indent = indent_at(body, line_start);
                    format!("{}{}{}", &body[..vs], pretty(value, &indent), &body[ve..])
                }
                None => insert(body, start, name, value),
            }
        }
        None => {
            let mut obj = serde_json::Map::new();
            obj.insert(name.to_string(), value.clone());
            insert(body, root, key, &Value::Object(obj))
        }
    };
    check(&old, &new, key, name, value)?;
    Ok(Some(format!("{bom}{new}")))
}

/// The new text must parse, hold `value` there, and be the old one elsewhere.
fn check(old: &Value, new: &str, key: &str, name: &str, value: &Value) -> Result<(), String> {
    let mut got: Value = serde_json::from_str(new).map_err(|e| format!("the edit broke the file: {e}"))?;
    if got.get(key).and_then(|m| m.get(name)) != Some(value) {
        return Err("the edit did not land".into());
    }
    let mut want = old.clone();
    let strip = |v: &mut Value| {
        if let Some(m) = v.get_mut(key).and_then(Value::as_object_mut) {
            m.remove(name);
            if m.is_empty() {
                v.as_object_mut().map(|o| o.remove(key));
            }
        }
    };
    strip(&mut want);
    strip(&mut got);
    if got != want {
        return Err("the edit changed more than its entry".into());
    }
    Ok(())
}

/// `"name": value` added first in the object whose `{` is at `open`.
fn insert(body: &str, open: usize, name: &str, value: &Value) -> String {
    let after = &body[open + 1..];
    let empty = after.trim_start().starts_with('}');
    // The indent of the object's first entry, else its own line's plus two.
    let first = after.find(|c: char| !c.is_whitespace()).map(|i| open + 1 + i);
    let indent = match first.filter(|_| !empty) {
        Some(i) => indent_at(body, line_start(body, i)),
        None => format!("{}  ", indent_at(body, line_start(body, open))),
    };
    let key = serde_json::to_string(name).expect("a string");
    let comma = if empty { "" } else { "," };
    format!("{}\n{indent}{key}: {}{comma}{}", &body[..=open], pretty(value, &indent), &body[open + 1..])
}

fn pretty(value: &Value, indent: &str) -> String {
    serde_json::to_string_pretty(value).expect("a value").replace('\n', &format!("\n{indent}"))
}

fn line_start(body: &str, at: usize) -> usize {
    body[..at].rfind('\n').map(|i| i + 1).unwrap_or(0)
}

fn indent_at(body: &str, line_start: usize) -> String {
    body[line_start..].chars().take_while(|c| *c == ' ' || *c == '\t').collect()
}

/// The member `name` of the object whose `{` is at `open`: the start of its
/// line, and where its value starts and ends.
fn find(body: &str, open: usize, name: &str) -> Result<Option<(usize, usize, usize)>, String> {
    let b = body.as_bytes();
    let mut i = skip_ws(b, open + 1);
    while i < b.len() && b[i] != b'}' {
        let key_at = i;
        let key_end = skip_value(b, i)?;
        let key: String = serde_json::from_str(&body[key_at..key_end]).map_err(|e| e.to_string())?;
        i = skip_ws(b, key_end);
        if b.get(i) != Some(&b':') {
            return Err("expected :".into());
        }
        let vs = skip_ws(b, i + 1);
        let ve = skip_value(b, vs)?;
        if key == name {
            return Ok(Some((line_start(body, key_at), vs, ve)));
        }
        i = skip_ws(b, ve);
        if b.get(i) == Some(&b',') {
            i = skip_ws(b, i + 1);
        }
    }
    Ok(None)
}

fn skip_ws(b: &[u8], mut i: usize) -> usize {
    while i < b.len() && b[i].is_ascii_whitespace() {
        i += 1;
    }
    i
}

/// The end of the JSON value starting at `i`.
fn skip_value(b: &[u8], mut i: usize) -> Result<usize, String> {
    match b.get(i) {
        Some(b'"') => {
            i += 1;
            while i < b.len() {
                match b[i] {
                    b'\\' => i += 2,
                    b'"' => return Ok(i + 1),
                    _ => i += 1,
                }
            }
            Err("a string never ends".into())
        }
        Some(b'{') | Some(b'[') => {
            let mut depth = 0usize;
            while i < b.len() {
                match b[i] {
                    b'"' => {
                        i = skip_value(b, i)?;
                        continue;
                    }
                    b'{' | b'[' => depth += 1,
                    b'}' | b']' => {
                        depth -= 1;
                        if depth == 0 {
                            return Ok(i + 1);
                        }
                    }
                    _ => {}
                }
                i += 1;
            }
            Err("an object never ends".into())
        }
        Some(_) => {
            while i < b.len() && !matches!(b[i], b',' | b'}' | b']') && !b[i].is_ascii_whitespace() {
                i += 1;
            }
            Ok(i)
        }
        None => Err("the file ends early".into()),
    }
}
