//! Model Artifact Inspection
//!
//! **File:** `model_artifact.rs`
//! **Author:** Kevin Thomas
//! **Date:** 2026
//!
//! MIT License
//!
//! Copyright (c) 2026 Kevin Thomas
//!
//! Permission is hereby granted, free of charge, to any person obtaining a copy
//! of this software and associated documentation files (the "Software"), to deal
//! in the Software without restriction, including without limitation the rights
//! to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
//! copies of the Software, and to permit persons to whom the Software is
//! furnished to do so, subject to the following conditions:
//!
//! The above copyright notice and this permission notice shall be included in
//! all copies or substantial portions of the Software.
//!
//! THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
//! IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
//! FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
//! AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
//! LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
//! OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
//! SOFTWARE.

// Import dependencies from serde_json
use serde_json::Value;

/// `GGUF` file magic (`"GGUF"` little-endian).
const GGUF_MAGIC: u32 = 0x4655_4747;

/// Largest `safetensors` header accepted, in bytes.
const MAX_SAFETENSORS_HEADER: u64 = 100 << 20;

/// Maximum metadata keys recorded.
const MAX_METADATA: usize = 512;

/// Maximum tensors recorded.
const MAX_TENSORS: usize = 8192;

/// Maximum elements in a metadata array before parsing stops.
const MAX_ARRAY: u64 = 4_000_000;

/// `GGML` tensor type names indexed by type id.
const GGML_TYPES: [&str; 30] = [
    "F32", "F16", "Q4_0", "Q4_1", "Q4_2", "Q4_3", "Q5_0", "Q5_1", "Q8_0", "Q8_1", "Q2_K", "Q3_K",
    "Q4_K", "Q5_K", "Q6_K", "Q8_K", "IQ2_XXS", "IQ2_XS", "IQ3_XXS", "IQ1_S", "IQ4_NL", "IQ3_S",
    "IQ2_S", "IQ4_XS", "I8", "I16", "I32", "I64", "F64", "IQ1_M",
];

/// Recognized model artifact container.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModelFormat {
    /// A `GGUF` model file.
    Gguf,

    /// A `safetensors` tensor archive.
    Safetensors,
}

/// One recovered tensor declaration.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct ModelTensor {
    /// Tensor name.
    pub name: String,

    /// Element type label.
    pub dtype: String,

    /// Element count.
    pub elements: u64,
}

/// Recovered model artifact inventory.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct ModelArtifact {
    /// Container format.
    pub format: Option<ModelFormat>,

    /// Version label.
    pub version: String,

    /// Model name or architecture, when known.
    pub name: String,

    /// Declared tensor count.
    pub tensor_count: u64,

    /// Total element count across tensors.
    pub parameters: u64,

    /// Recorded tensors.
    pub tensors: Vec<ModelTensor>,

    /// Recorded metadata keys.
    pub metadata: Vec<String>,
}

/// Reports whether a buffer begins with the `GGUF` magic.
///
/// # Arguments
///
/// * `bytes` - Start of the target file.
///
/// # Returns
///
/// `true` when the magic matches.
#[must_use]
pub fn is_gguf(bytes: &[u8]) -> bool {
    bytes.len() >= 4 && u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) == GGUF_MAGIC
}

/// Reports whether a buffer looks like a `safetensors` archive.
///
/// # Arguments
///
/// * `bytes` - Start of the target file.
///
/// # Returns
///
/// `true` when a bounded JSON header follows the length prefix.
#[must_use]
pub fn is_safetensors(bytes: &[u8]) -> bool {
    let Some(len) = header_len(bytes) else {
        return false;
    };
    len <= MAX_SAFETENSORS_HEADER && bytes.get(8) == Some(&b'{')
}

/// Reads the `safetensors` header length prefix.
///
/// # Arguments
///
/// * `bytes` - Start of the target file.
///
/// # Returns
///
/// The header length, when the file is large enough.
fn header_len(bytes: &[u8]) -> Option<u64> {
    let head = bytes.get(..8)?;
    let mut buf = [0u8; 8];
    buf.copy_from_slice(head);
    Some(u64::from_le_bytes(buf))
}

/// Parses a model artifact of either supported format.
///
/// # Arguments
///
/// * `bytes` - Raw target bytes.
///
/// # Returns
///
/// The recovered inventory, or `None`.
#[must_use]
pub fn parse_model(bytes: &[u8]) -> Option<ModelArtifact> {
    if is_gguf(bytes) {
        return parse_gguf(bytes);
    }
    if is_safetensors(bytes) {
        return parse_safetensors(bytes);
    }
    None
}

/// Formats a model artifact summary.
///
/// # Arguments
///
/// * `model` - Recovered inventory.
///
/// # Returns
///
/// The summary text.
#[must_use]
pub fn format_model_summary(model: &ModelArtifact) -> String {
    let label = model.format.map_or("model", format_label);
    let name = display_name(&model.name);
    let tensors = model.tensor_count;
    let params = model.parameters;
    let keys = model.metadata.len();
    format!("{label} ({name}): {tensors} tensors, {params} parameters, {keys} metadata keys.\n")
}

/// Returns a display name for a possibly empty model name.
///
/// # Arguments
///
/// * `name` - Model name.
///
/// # Returns
///
/// The name, or `unnamed`.
fn display_name(name: &str) -> &str {
    if name.is_empty() {
        "unnamed"
    } else {
        name
    }
}

/// Returns the display label for a model container.
///
/// # Arguments
///
/// * `format` - Container format.
///
/// # Returns
///
/// The label.
fn format_label(format: ModelFormat) -> &'static str {
    match format {
        ModelFormat::Gguf => "GGUF",
        ModelFormat::Safetensors => "safetensors",
    }
}

/// A `GGUF` metadata value of interest.
enum GgufValue {
    /// String value.
    Str(String),

    /// Skipped numeric, array, or unsupported value.
    Other,
}

/// A bounds-checked byte cursor.
struct Cursor<'a> {
    /// Source bytes.
    bytes: &'a [u8],

    /// Current offset.
    pos: usize,
}

impl<'a> Cursor<'a> {
    /// Builds a cursor at the given offset.
    ///
    /// # Arguments
    ///
    /// * `bytes` - Source bytes.
    /// * `pos` - Starting offset.
    ///
    /// # Returns
    ///
    /// The cursor.
    fn new(bytes: &'a [u8], pos: usize) -> Cursor<'a> {
        Cursor { bytes, pos }
    }

    /// Reads a little-endian `u32`.
    ///
    /// # Returns
    ///
    /// The value, or `None` at the end of input.
    fn u32(&mut self) -> Option<u32> {
        let b = self.take(4)?;
        Some(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    /// Reads a little-endian `u64`.
    ///
    /// # Returns
    ///
    /// The value, or `None` at the end of input.
    fn u64(&mut self) -> Option<u64> {
        let b = self.take(8)?;
        let mut buf = [0u8; 8];
        buf.copy_from_slice(b);
        Some(u64::from_le_bytes(buf))
    }

    /// Borrows the next `n` bytes.
    ///
    /// # Arguments
    ///
    /// * `n` - Byte count.
    ///
    /// # Returns
    ///
    /// The slice, or `None` when out of bounds.
    fn take(&mut self, n: usize) -> Option<&'a [u8]> {
        let end = self.pos.checked_add(n)?;
        let slice = self.bytes.get(self.pos..end)?;
        self.pos = end;
        Some(slice)
    }

    /// Advances by `n` bytes.
    ///
    /// # Arguments
    ///
    /// * `n` - Byte count.
    ///
    /// # Returns
    ///
    /// `Some(())` when the advance is in bounds.
    fn skip(&mut self, n: usize) -> Option<()> {
        self.take(n).map(|_| ())
    }

    /// Reads a `GGUF` string (length-prefixed, not null-terminated).
    ///
    /// # Returns
    ///
    /// The string, or `None` when out of bounds.
    fn string(&mut self) -> Option<String> {
        let len = self.u64()?;
        let capped = usize::try_from(len.min(4096)).ok()?;
        let slice = self.take(capped)?;
        Some(String::from_utf8_lossy(slice).to_string())
    }
}

/// Parses a `GGUF` model file.
///
/// # Arguments
///
/// * `bytes` - Raw target bytes.
///
/// # Returns
///
/// The recovered inventory, or `None`.
fn parse_gguf(bytes: &[u8]) -> Option<ModelArtifact> {
    let (mut cursor, version, tensor_count, kv_count) = gguf_header(bytes)?;
    let mut model = gguf_model(version, tensor_count);
    read_gguf_metadata(&mut cursor, kv_count, &mut model)?;
    read_gguf_tensors(&mut cursor, tensor_count, &mut model)?;
    model.parameters = model.tensors.iter().map(|t| t.elements).sum();
    Some(model)
}

/// Reads the `GGUF` header fields.
///
/// # Arguments
///
/// * `bytes` - Raw target bytes.
///
/// # Returns
///
/// The cursor, version, tensor count, and pair count.
fn gguf_header(bytes: &[u8]) -> Option<(Cursor<'_>, u32, u64, u64)> {
    let mut cursor = Cursor::new(bytes, 4);
    let version = cursor.u32()?;
    let tensors = cursor.u64()?;
    let pairs = cursor.u64()?;
    Some((cursor, version, tensors, pairs))
}

/// Builds an empty `GGUF` inventory.
///
/// # Arguments
///
/// * `version` - Format version.
/// * `tensor_count` - Declared tensor count.
///
/// # Returns
///
/// The inventory.
fn gguf_model(version: u32, tensor_count: u64) -> ModelArtifact {
    ModelArtifact {
        format: Some(ModelFormat::Gguf),
        version: format!("GGUF v{version}"),
        tensor_count,
        ..ModelArtifact::default()
    }
}

/// Reads `GGUF` metadata key/value pairs.
///
/// # Arguments
///
/// * `cursor` - Input cursor.
/// * `count` - Declared pair count.
/// * `model` - Output inventory.
///
/// # Returns
///
/// `Some(())` when parsing completes.
fn read_gguf_metadata(
    cursor: &mut Cursor<'_>,
    count: u64,
    model: &mut ModelArtifact,
) -> Option<()> {
    let limit = usize::try_from(count.min(MAX_METADATA as u64)).ok()?;
    for _ in 0..limit {
        let key = cursor.string()?;
        let kind = cursor.u32()?;
        let value = read_value(cursor, kind)?;
        record_metadata(&key, value, model);
    }
    Some(())
}

/// Records one metadata pair and captures the model name.
///
/// # Arguments
///
/// * `key` - Metadata key.
/// * `value` - Parsed value.
/// * `model` - Output inventory.
fn record_metadata(key: &str, value: GgufValue, model: &mut ModelArtifact) {
    model.metadata.push(key.to_string());
    if let GgufValue::Str(text) = value {
        if is_name_key(key) {
            model.name.clone_from(&text);
        }
    }
}

/// Reports whether a metadata key names the model.
///
/// # Arguments
///
/// * `key` - Metadata key.
///
/// # Returns
///
/// `true` for known naming keys.
fn is_name_key(key: &str) -> bool {
    matches!(
        key,
        "general.name" | "general.architecture" | "general.basename" | "tokenizer.ggml.model"
    )
}

/// Reads a `GGUF` metadata value, skipping array contents.
///
/// # Arguments
///
/// * `cursor` - Input cursor.
/// * `kind` - Value type id.
///
/// # Returns
///
/// The scalar or string value, or `None` when unsupported.
fn read_value(cursor: &mut Cursor<'_>, kind: u32) -> Option<GgufValue> {
    match kind {
        8 => Some(GgufValue::Str(cursor.string()?)),
        9 => skip_array(cursor).map(|()| GgufValue::Other),
        _ => read_scalar(cursor, kind),
    }
}

/// Reads a fixed-size `GGUF` scalar.
///
/// # Arguments
///
/// * `cursor` - Input cursor.
/// * `kind` - Value type id.
///
/// # Returns
///
/// The numeric value, or `None` when unsupported.
fn read_scalar(cursor: &mut Cursor<'_>, kind: u32) -> Option<GgufValue> {
    let size = scalar_size(kind)?;
    cursor.take(size)?;
    Some(GgufValue::Other)
}

/// Returns the byte size of a fixed-size `GGUF` scalar.
///
/// # Arguments
///
/// * `kind` - Value type id.
///
/// # Returns
///
/// The size, or `None` when the type is variable or unsupported.
fn scalar_size(kind: u32) -> Option<usize> {
    match kind {
        0 | 1 | 7 => Some(1),
        2 | 3 => Some(2),
        4..=6 => Some(4),
        10..=12 => Some(8),
        _ => None,
    }
}

/// Skips a `GGUF` array payload.
///
/// # Arguments
///
/// * `cursor` - Input cursor.
///
/// # Returns
///
/// `Some(())` when the array is skipped.
fn skip_array(cursor: &mut Cursor<'_>) -> Option<()> {
    let element = cursor.u32()?;
    let count = cursor.u64()?;
    if element == 8 {
        return skip_strings(cursor, count);
    }
    skip_fixed(cursor, element, count)
}

/// Skips a counted run of fixed-size `GGUF` values.
///
/// # Arguments
///
/// * `cursor` - Input cursor.
/// * `element` - Element type id.
/// * `count` - Element count.
///
/// # Returns
///
/// `Some(())` when skipped.
fn skip_fixed(cursor: &mut Cursor<'_>, element: u32, count: u64) -> Option<()> {
    if count > MAX_ARRAY {
        return None;
    }
    let size = scalar_size(element)?;
    let bytes = size.checked_mul(usize::try_from(count).ok()?)?;
    cursor.skip(bytes)
}

/// Skips a counted run of `GGUF` strings.
///
/// # Arguments
///
/// * `cursor` - Input cursor.
/// * `count` - String count.
///
/// # Returns
///
/// `Some(())` when all strings are skipped.
fn skip_strings(cursor: &mut Cursor<'_>, count: u64) -> Option<()> {
    if count > MAX_ARRAY {
        return None;
    }
    for _ in 0..count {
        let len = cursor.u64()?;
        cursor.skip(usize::try_from(len.min(1 << 20)).ok()?)?;
    }
    Some(())
}

/// Reads `GGUF` tensor declarations.
///
/// # Arguments
///
/// * `cursor` - Input cursor.
/// * `count` - Declared tensor count.
/// * `model` - Output inventory.
///
/// # Returns
///
/// `Some(())` when parsing completes.
fn read_gguf_tensors(cursor: &mut Cursor<'_>, count: u64, model: &mut ModelArtifact) -> Option<()> {
    let limit = usize::try_from(count.min(MAX_TENSORS as u64)).ok()?;
    for _ in 0..limit {
        model.tensors.push(read_gguf_tensor(cursor)?);
    }
    Some(())
}

/// Reads one `GGUF` tensor declaration.
///
/// # Arguments
///
/// * `cursor` - Input cursor.
///
/// # Returns
///
/// The tensor, or `None` on malformed input.
fn read_gguf_tensor(cursor: &mut Cursor<'_>) -> Option<ModelTensor> {
    let name = cursor.string()?;
    let dims = cursor.u32()?;
    let elements = read_dimensions(cursor, dims)?;
    let kind = cursor.u32()?;
    cursor.u64()?;
    Some(ModelTensor { name, dtype: ggml_type(kind), elements })
}

/// Reads tensor dimensions and returns the element count.
///
/// # Arguments
///
/// * `cursor` - Input cursor.
/// * `dims` - Dimension count.
///
/// # Returns
///
/// The product of the dimensions, or `None` when empty.
fn read_dimensions(cursor: &mut Cursor<'_>, dims: u32) -> Option<u64> {
    if dims == 0 || dims > 8 {
        return None;
    }
    let mut total = 1u64;
    for _ in 0..dims {
        total = total.saturating_mul(cursor.u64()?);
    }
    Some(total)
}

/// Returns the `GGML` type name for a type id.
///
/// # Arguments
///
/// * `kind` - Type id.
///
/// # Returns
///
/// The type name.
fn ggml_type(kind: u32) -> String {
    let index = kind as usize;
    GGML_TYPES
        .get(index)
        .map_or_else(|| format!("type{kind}"), |name| (*name).to_string())
}

/// Parses a `safetensors` archive.
///
/// # Arguments
///
/// * `bytes` - Raw target bytes.
///
/// # Returns
///
/// The recovered inventory, or `None`.
fn parse_safetensors(bytes: &[u8]) -> Option<ModelArtifact> {
    let len = header_len(bytes)?;
    let header = bytes.get(8..8 + usize::try_from(len).ok()?)?;
    let json = serde_json::from_slice::<Value>(header).ok()?;
    let mut model = safetensors_model();
    read_tensors(&json, &mut model);
    finish(model)
}

/// Finalizes a parsed inventory, rejecting empty artifacts.
///
/// # Arguments
///
/// * `model` - Parsed inventory.
///
/// # Returns
///
/// The finalized inventory, or `None` when no tensors exist.
fn finish(mut model: ModelArtifact) -> Option<ModelArtifact> {
    if model.tensors.is_empty() {
        return None;
    }
    model.tensor_count = model.tensors.len() as u64;
    model.parameters = model.tensors.iter().map(|t| t.elements).sum();
    Some(model)
}

/// Builds an empty safetensors inventory.
///
/// # Returns
///
/// The inventory.
fn safetensors_model() -> ModelArtifact {
    ModelArtifact {
        format: Some(ModelFormat::Safetensors),
        version: "safetensors".to_string(),
        ..ModelArtifact::default()
    }
}

/// Collects tensor declarations from the safetensors header.
///
/// # Arguments
///
/// * `json` - Parsed header object.
/// * `model` - Output inventory.
fn read_tensors(json: &Value, model: &mut ModelArtifact) {
    let Some(entries) = json.as_object() else {
        return;
    };
    for (name, value) in entries.iter().take(MAX_TENSORS) {
        record_entry(name, value, model);
    }
}

/// Records one header entry as metadata or a tensor.
///
/// # Arguments
///
/// * `name` - Entry name.
/// * `value` - Entry value.
/// * `model` - Output inventory.
fn record_entry(name: &str, value: &Value, model: &mut ModelArtifact) {
    if name == "__metadata__" {
        record_metadata_keys(value, model);
        return;
    }
    model.tensors.push(tensor_from(name, value));
}

/// Records safetensors metadata keys.
///
/// # Arguments
///
/// * `value` - Metadata object.
/// * `model` - Output inventory.
fn record_metadata_keys(value: &Value, model: &mut ModelArtifact) {
    let Some(entries) = value.as_object() else {
        return;
    };
    model.metadata.extend(entries.keys().take(MAX_METADATA).cloned());
}

/// Builds a tensor from one safetensors header entry.
///
/// # Arguments
///
/// * `name` - Tensor name.
/// * `value` - Header entry object.
///
/// # Returns
///
/// The tensor declaration.
fn tensor_from(name: &str, value: &Value) -> ModelTensor {
    let dtype = value.get("dtype").and_then(Value::as_str).unwrap_or("");
    let elements = shape_elements(value.get("shape"));
    ModelTensor { name: name.to_string(), dtype: dtype.to_string(), elements }
}

/// Multiplies a tensor shape array.
///
/// # Arguments
///
/// * `shape` - Shape JSON value.
///
/// # Returns
///
/// The element count.
fn shape_elements(shape: Option<&Value>) -> u64 {
    let Some(items) = shape.and_then(Value::as_array) else {
        return 0;
    };
    items.iter().filter_map(Value::as_u64).fold(1, u64::saturating_mul)
}

#[cfg(test)]
mod tests {
    // Import all parent module items
    use super::*;

    /// Builds a little-endian `u32`.
    ///
    /// # Arguments
    ///
    /// * `value` - Value to encode.
    ///
    /// # Returns
    ///
    /// The encoded bytes.
    fn u32_le(value: u32) -> Vec<u8> {
        value.to_le_bytes().to_vec()
    }

    /// Builds a little-endian `u64`.
    ///
    /// # Arguments
    ///
    /// * `value` - Value to encode.
    ///
    /// # Returns
    ///
    /// The encoded bytes.
    fn u64_le(value: u64) -> Vec<u8> {
        value.to_le_bytes().to_vec()
    }

    /// Appends a `GGUF` string.
    ///
    /// # Arguments
    ///
    /// * `out` - Output buffer.
    /// * `text` - String to append.
    fn gguf_string(out: &mut Vec<u8>, text: &str) {
        out.extend_from_slice(&u64_le(text.len() as u64));
        out.extend_from_slice(text.as_bytes());
    }

    /// Builds a minimal synthetic `GGUF` file.
    ///
    /// # Returns
    ///
    /// The encoded file.
    fn gguf() -> Vec<u8> {
        let mut buf = u32_le(GGUF_MAGIC);
        buf.extend_from_slice(&u32_le(3));
        buf.extend_from_slice(&u64_le(1));
        buf.extend_from_slice(&u64_le(1));
        gguf_kv(&mut buf);
        gguf_tensor(&mut buf);
        buf
    }

    /// Appends the synthetic `GGUF` metadata pair.
    ///
    /// # Arguments
    ///
    /// * `buf` - Output buffer.
    fn gguf_kv(buf: &mut Vec<u8>) {
        gguf_string(buf, "general.name");
        buf.extend_from_slice(&u32_le(8));
        gguf_string(buf, "tiny-model");
    }

    /// Appends the synthetic `GGUF` tensor declaration.
    ///
    /// # Arguments
    ///
    /// * `buf` - Output buffer.
    fn gguf_tensor(buf: &mut Vec<u8>) {
        gguf_string(buf, "tok.weight");
        buf.extend_from_slice(&u32_le(2));
        buf.extend_from_slice(&u64_le(4));
        buf.extend_from_slice(&u64_le(2));
        buf.extend_from_slice(&u32_le(0));
        buf.extend_from_slice(&u64_le(0));
    }

    /// Verifies `GGUF` detection and parsing.
    #[test]
    fn test_parse_gguf() {
        let model = parse_model(&gguf()).expect("gguf");
        assert_eq!(model.name, "tiny-model");
        assert_eq!(model.tensor_count, 1);
        assert_eq!(model.parameters, 8);
        assert_eq!(model.tensors[0].dtype, "F32");
    }

    /// Verifies `safetensors` detection and parsing.
    #[test]
    fn test_parse_safetensors() {
        let header = br#"{"__metadata__":{"format":"pt"},"w":{"dtype":"F32","shape":[2,3],"data_offsets":[0,24]}}"#;
        let mut buf = u64_le(header.len() as u64);
        buf.extend_from_slice(header);
        buf.extend_from_slice(&[0u8; 24]);
        let model = parse_model(&buf).expect("safetensors");
        assert_eq!(model.tensors.len(), 1);
        assert_eq!(model.tensors[0].elements, 6);
        assert_eq!(model.metadata[0], "format");
    }

    /// Verifies that plain data is not treated as a model artifact.
    #[test]
    fn test_rejects_plain_data() {
        assert!(parse_model(b"plain data").is_none());
    }
}
