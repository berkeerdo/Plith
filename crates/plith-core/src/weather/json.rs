//! A minimal JSON reader: recursive descent over a `&str`.

/// How deep arrays and objects may nest before the reader gives up.
const MAX_DEPTH: usize = 64;

/// A parsed JSON value.
#[derive(Debug, Clone, PartialEq)]
pub enum Json {
    /// `null`.
    Null,
    /// `true` or `false`.
    Bool(bool),
    /// Any number.
    Num(f64),
    /// A string with escapes resolved.
    Str(String),
    /// An array.
    Arr(Vec<Json>),
    /// An object, in source order.
    Obj(Vec<(String, Json)>),
}

impl Json {
    /// The value under `key` when this is an object that has it.
    pub fn get(&self, key: &str) -> Option<&Json> {
        match self {
            Json::Obj(pairs) => pairs.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    /// The number, when this is one.
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Json::Num(n) => Some(*n),
            _ => None,
        }
    }

    /// The string, when this is one.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Json::Str(s) => Some(s),
            _ => None,
        }
    }

    /// The elements, when this is an array.
    pub fn as_arr(&self) -> Option<&[Json]> {
        match self {
            Json::Arr(items) => Some(items),
            _ => None,
        }
    }
}

/// Parses one complete JSON document; trailing garbage is an error.
pub fn parse(text: &str) -> Result<Json, String> {
    let mut reader = Reader { text, pos: 0 };
    let value = reader.value(0)?;
    reader.skip_ws();
    if reader.pos != text.len() {
        return Err("trailing data after JSON value".to_string());
    }
    Ok(value)
}

struct Reader<'a> {
    text: &'a str,
    pos: usize,
}

impl Reader<'_> {
    fn peek(&self) -> Option<u8> {
        self.text.as_bytes().get(self.pos).copied()
    }

    fn skip_ws(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            self.pos += 1;
        }
    }

    fn expect(&mut self, byte: u8) -> Result<(), String> {
        if self.peek() == Some(byte) {
            self.pos += 1;
            Ok(())
        } else {
            Err(format!("expected '{}' at {}", char::from(byte), self.pos))
        }
    }

    fn literal(&mut self, word: &str, value: Json) -> Result<Json, String> {
        if self.text[self.pos..].starts_with(word) {
            self.pos += word.len();
            Ok(value)
        } else {
            Err(format!("bad literal at {}", self.pos))
        }
    }

    fn value(&mut self, depth: usize) -> Result<Json, String> {
        if depth > MAX_DEPTH {
            return Err("JSON nested too deeply".to_string());
        }
        self.skip_ws();
        match self.peek() {
            None => Err("unexpected end of JSON".to_string()),
            Some(b'n') => self.literal("null", Json::Null),
            Some(b't') => self.literal("true", Json::Bool(true)),
            Some(b'f') => self.literal("false", Json::Bool(false)),
            Some(b'"') => self.string().map(Json::Str),
            Some(b'[') => self.array(depth),
            Some(b'{') => self.object(depth),
            Some(b'-' | b'0'..=b'9') => self.number(),
            Some(_) => Err(format!("unexpected character at {}", self.pos)),
        }
    }

    fn number(&mut self) -> Result<Json, String> {
        let start = self.pos;
        while matches!(
            self.peek(),
            Some(b'-' | b'+' | b'.' | b'e' | b'E' | b'0'..=b'9')
        ) {
            self.pos += 1;
        }
        self.text[start..self.pos]
            .parse::<f64>()
            .map(Json::Num)
            .map_err(|_| format!("bad number at {start}"))
    }

    fn array(&mut self, depth: usize) -> Result<Json, String> {
        self.expect(b'[')?;
        let mut items = Vec::new();
        self.skip_ws();
        if self.peek() == Some(b']') {
            self.pos += 1;
            return Ok(Json::Arr(items));
        }
        loop {
            items.push(self.value(depth + 1)?);
            self.skip_ws();
            match self.peek() {
                Some(b',') => self.pos += 1,
                Some(b']') => {
                    self.pos += 1;
                    return Ok(Json::Arr(items));
                }
                _ => return Err(format!("expected ',' or ']' at {}", self.pos)),
            }
        }
    }

    fn object(&mut self, depth: usize) -> Result<Json, String> {
        self.expect(b'{')?;
        let mut pairs = Vec::new();
        self.skip_ws();
        if self.peek() == Some(b'}') {
            self.pos += 1;
            return Ok(Json::Obj(pairs));
        }
        loop {
            self.skip_ws();
            let key = self.string()?;
            self.skip_ws();
            self.expect(b':')?;
            pairs.push((key, self.value(depth + 1)?));
            self.skip_ws();
            match self.peek() {
                Some(b',') => self.pos += 1,
                Some(b'}') => {
                    self.pos += 1;
                    return Ok(Json::Obj(pairs));
                }
                _ => return Err(format!("expected ',' or '}}' at {}", self.pos)),
            }
        }
    }

    fn string(&mut self) -> Result<String, String> {
        self.expect(b'"')?;
        let mut out = String::new();
        loop {
            let Some(ch) = self.text[self.pos..].chars().next() else {
                return Err("unterminated string".to_string());
            };
            self.pos += ch.len_utf8();
            match ch {
                '"' => return Ok(out),
                '\\' => out.push(self.escape()?),
                c if u32::from(c) < 0x20 => return Err("control character in string".to_string()),
                c => out.push(c),
            }
        }
    }

    fn escape(&mut self) -> Result<char, String> {
        let Some(byte) = self.peek() else {
            return Err("unterminated escape".to_string());
        };
        self.pos += 1;
        match byte {
            b'"' => Ok('"'),
            b'\\' => Ok('\\'),
            b'/' => Ok('/'),
            b'b' => Ok('\u{8}'),
            b'f' => Ok('\u{c}'),
            b'n' => Ok('\n'),
            b'r' => Ok('\r'),
            b't' => Ok('\t'),
            b'u' => self.unicode(),
            _ => Err("bad escape".to_string()),
        }
    }

    fn hex4(&mut self) -> Result<u32, String> {
        let end = self.pos + 4;
        let digits = self.text.get(self.pos..end).ok_or("short \\u escape")?;
        if !digits.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err("bad \\u escape".to_string());
        }
        self.pos = end;
        u32::from_str_radix(digits, 16).map_err(|_| "bad \\u escape".to_string())
    }

    fn unicode(&mut self) -> Result<char, String> {
        let first = self.hex4()?;
        if (0xD800..0xDC00).contains(&first) {
            if !self.text[self.pos..].starts_with("\\u") {
                return Err("lone high surrogate".to_string());
            }
            self.pos += 2;
            let second = self.hex4()?;
            if !(0xDC00..0xE000).contains(&second) {
                return Err("bad surrogate pair".to_string());
            }
            let code = 0x10000 + ((first - 0xD800) << 10) + (second - 0xDC00);
            return char::from_u32(code).ok_or_else(|| "bad surrogate pair".to_string());
        }
        char::from_u32(first).ok_or_else(|| "lone low surrogate".to_string())
    }
}

#[cfg(test)]
mod tests;
