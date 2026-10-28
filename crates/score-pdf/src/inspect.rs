//! A minimal reader for the PDFs the portable writer produces, so tests on every OS
//! can check pages, paper sizes and the text a reader would extract.
use std::{collections::HashMap, io::Read};

pub struct Page {
    pub size: (f64, f64),
    /// The page's text, one line per text-showing operator.
    pub text: String,
}

struct Object<'a> {
    dict: &'a [u8],
    stream: Option<Vec<u8>>,
}

pub fn pages(pdf: &[u8]) -> Vec<Page> {
    assert!(pdf.starts_with(b"%PDF-"), "missing PDF header");
    assert!(
        pdf.trim_ascii_end().ends_with(b"%%EOF"),
        "missing PDF trailer"
    );
    let objects = objects(pdf);
    let tree = objects
        .values()
        .find(|o| contains(o.dict, b"/Type/Pages"))
        .expect("page tree");
    references(after(tree.dict, b"/Kids[").expect("page list"))
        .into_iter()
        .map(|id| {
            let page = &objects[&id];
            assert!(contains(page.dict, b"/Type/Page"), "page object");
            let media: Vec<f64> = String::from_utf8_lossy(
                after(page.dict, b"/MediaBox[")
                    .and_then(|rest| rest.split(|&b| b == b']').next())
                    .expect("media box"),
            )
            .split_whitespace()
            .map(|n| n.parse().expect("media box number"))
            .collect();
            let fonts = fonts(&objects, page.dict);
            let contents = reference(page.dict, b"/Contents ").expect("page contents");
            let content = objects[&contents]
                .stream
                .as_deref()
                .expect("content stream");
            Page {
                size: (media[2] - media[0], media[3] - media[1]),
                text: text(content, &fonts),
            }
        })
        .collect()
}

fn objects(pdf: &[u8]) -> HashMap<u32, Object<'_>> {
    let mut objects = HashMap::new();
    let mut at = 0;
    while let Some(found) = find(&pdf[at..], b" 0 obj\n") {
        let header = at + found;
        let digits = pdf[..header]
            .iter()
            .rev()
            .take_while(|b| b.is_ascii_digit())
            .count();
        let id = std::str::from_utf8(&pdf[header - digits..header])
            .unwrap()
            .parse()
            .unwrap();
        let start = header + b" 0 obj\n".len();
        let rest = &pdf[start..];
        let object_end = find(rest, b"endobj").expect("object end");
        // The dictionary is text, so whichever marker comes first is in it; stream
        // data is binary and is skipped by its length.
        let (dict, stream, end) = match find(rest, b">>\nstream\n").filter(|&s| s < object_end) {
            Some(split) => {
                let dict = &rest[..split + 2];
                let length: usize = number(dict, b"/Length ").expect("stream length");
                let data_start = split + b">>\nstream\n".len();
                let data = &rest[data_start..data_start + length];
                let end = data_start
                    + length
                    + find(&rest[data_start + length..], b"endobj").expect("object end");
                let data = if contains(dict, b"/FlateDecode") {
                    let mut out = Vec::new();
                    flate2::read::ZlibDecoder::new(data)
                        .read_to_end(&mut out)
                        .expect("inflate stream");
                    out
                } else {
                    data.to_vec()
                };
                (dict, Some(data), end)
            }
            None => (&rest[..object_end], None, object_end),
        };
        objects.insert(id, Object { dict, stream });
        at = start + end;
    }
    objects
}

/// Each font resource name with its glyph-to-text map.
fn fonts(objects: &HashMap<u32, Object>, page: &[u8]) -> HashMap<String, HashMap<u16, String>> {
    let resources = &objects[&reference(page, b"/Resources ").expect("resources")];
    let Some(list) = after(resources.dict, b"/Font<<") else {
        return HashMap::new();
    };
    let list = &list[..find(list, b">>").unwrap()];
    String::from_utf8_lossy(list)
        .split('/')
        .filter(|entry| !entry.is_empty())
        .map(|entry| {
            let (name, id) = entry.split_once(' ').unwrap();
            let id: u32 = id.split_whitespace().next().unwrap().parse().unwrap();
            let unicode = reference(objects[&id].dict, b"/ToUnicode ").expect("ToUnicode");
            let cmap = objects[&unicode].stream.as_deref().unwrap();
            (name.to_owned(), cmap_entries(cmap))
        })
        .collect()
}

fn cmap_entries(cmap: &[u8]) -> HashMap<u16, String> {
    let cmap = String::from_utf8_lossy(cmap);
    let mut map = HashMap::new();
    for section in cmap.split("beginbfchar").skip(1) {
        let section = section.split("endbfchar").next().unwrap();
        let hex: Vec<_> = section
            .split(['<', '>'])
            .map(str::trim)
            .filter(|t| !t.is_empty())
            .collect();
        for pair in hex.chunks(2) {
            let units: Vec<u16> = (0..pair[1].len() / 4)
                .map(|i| u16::from_str_radix(&pair[1][i * 4..i * 4 + 4], 16).unwrap())
                .collect();
            map.insert(
                u16::from_str_radix(pair[0], 16).unwrap(),
                String::from_utf16_lossy(&units),
            );
        }
    }
    map
}

/// Decodes the strings shown in a content stream with the fonts selected by `Tf`.
fn text(content: &[u8], fonts: &HashMap<String, HashMap<u16, String>>) -> String {
    let mut text = String::new();
    let mut font: Option<&HashMap<u16, String>> = None;
    let mut last_name = String::new();
    let mut i = 0;
    while i < content.len() {
        match content[i] {
            b'/' => {
                let end = content[i + 1..]
                    .iter()
                    .position(|b| b" /[]()<>\n\r\t".contains(b))
                    .map_or(content.len(), |p| i + 1 + p);
                last_name = String::from_utf8_lossy(&content[i + 1..end]).into_owned();
                i = end;
            }
            b'(' => {
                let (bytes, end) = literal(content, i + 1);
                if let Some(font) = font {
                    for code in bytes.chunks(2) {
                        let code = u16::from_be_bytes([code[0], *code.get(1).unwrap_or(&0)]);
                        text.push_str(font.get(&code).map_or("\u{fffd}", String::as_str));
                    }
                }
                i = end;
            }
            b'T' if content.get(i + 1) == Some(&b'f') => {
                font = fonts.get(&last_name);
                i += 2;
            }
            b'T' if matches!(content.get(i + 1), Some(b'j' | b'J')) => {
                text.push('\n');
                i += 2;
            }
            _ => i += 1,
        }
    }
    text
}

/// A literal string's bytes, starting after its `(`, and the index after its `)`.
fn literal(content: &[u8], mut i: usize) -> (Vec<u8>, usize) {
    let mut bytes = Vec::new();
    let mut depth = 0;
    while i < content.len() {
        match content[i] {
            b'\\' => {
                i += 1;
                match content[i] {
                    b'n' => bytes.push(b'\n'),
                    b'r' => bytes.push(b'\r'),
                    b't' => bytes.push(b'\t'),
                    b'b' => bytes.push(8),
                    b'f' => bytes.push(12),
                    b'0'..=b'7' => {
                        let digits = content[i..]
                            .iter()
                            .take(3)
                            .take_while(|b| (b'0'..=b'7').contains(b))
                            .count();
                        let octal = std::str::from_utf8(&content[i..i + digits]).unwrap();
                        bytes.push(u8::from_str_radix(octal, 8).unwrap());
                        i += digits - 1;
                    }
                    other => bytes.push(other),
                }
            }
            b'(' => {
                depth += 1;
                bytes.push(b'(');
            }
            b')' if depth == 0 => return (bytes, i + 1),
            b')' => {
                depth -= 1;
                bytes.push(b')');
            }
            other => bytes.push(other),
        }
        i += 1;
    }
    panic!("unterminated string");
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}
fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    find(haystack, needle).is_some()
}
fn after<'a>(haystack: &'a [u8], key: &[u8]) -> Option<&'a [u8]> {
    find(haystack, key).map(|at| &haystack[at + key.len()..])
}
fn number<T: std::str::FromStr>(dict: &[u8], key: &[u8]) -> Option<T> {
    let rest = after(dict, key)?;
    let digits = rest.iter().take_while(|b| b.is_ascii_digit()).count();
    std::str::from_utf8(&rest[..digits]).ok()?.parse().ok()
}
fn reference(dict: &[u8], key: &[u8]) -> Option<u32> {
    number(dict, key)
}
fn references(list: &[u8]) -> Vec<u32> {
    let list = &list[..find(list, b"]").expect("list end")];
    String::from_utf8_lossy(list)
        .split(" R")
        .filter_map(|r| r.split_whitespace().next()?.parse().ok())
        .collect()
}
