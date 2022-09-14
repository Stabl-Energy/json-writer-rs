#![warn(missing_docs)]

//!
//! Simple and fast crate for writing JSON to a string writer without creating intermediate objects.
//!
//! # Usage
//!
//! Basic usage (infallible writer):
//! ```
//! use json_writer::write_object;
//!
//! let number: i32 = 42;
//! let mut object_str = String::new();
//!
//! let mut object_writer = write_object(&mut object_str).unwrap();
//! object_writer.member("number", number).unwrap();
//! object_writer.end().unwrap();
//!
//! assert_eq!(&object_str, "{\"number\":42}");
//! ```
//!
//! Various examples:
//!
//! ```
//! use json_writer::{Null, to_json_string, write_object};
//!
//! // Values
//! assert_eq!(to_json_string("Hello World\n"), "\"Hello World\\n\"");
//! assert_eq!(to_json_string(3.141592653589793f64), "3.141592653589793");
//! assert_eq!(to_json_string(true), "true");
//! assert_eq!(to_json_string(false), "false");
//! assert_eq!(to_json_string(Null), "null");
//!
//! // Options of values
//! assert_eq!(to_json_string(Option::<u8>::Some(42)), "42");
//! assert_eq!(to_json_string(Option::<u8>::None), "null");
//!
//! // Slices and vectors
//! let numbers: [u8; 4] = [1,2,3,4];
//! assert_eq!(to_json_string(&numbers[..]), "[1,2,3,4]");
//! let numbers_vec: Vec<u8> = vec!(1u8,2u8,3u8,4u8);
//! assert_eq!(to_json_string(&numbers_vec), "[1,2,3,4]");
//! let strings: [&str; 4] = ["a","b","c","d"];
//! assert_eq!(to_json_string(&strings[..]), "[\"a\",\"b\",\"c\",\"d\"]");
//!
//! // Hash-maps:
//! let mut map = std::collections::HashMap::<String,String>::new();
//! map.insert("Hello".to_owned(), "World".to_owned());
//! assert_eq!(to_json_string(&map), "{\"Hello\":\"World\"}");
//!
//! // Objects:
//! let mut object_str = String::new();
//! let mut object_writer = write_object(&mut object_str).unwrap();
//!
//! // Values
//! object_writer.member("number", 42i32).unwrap();
//! object_writer.member("slice", &numbers[..]).unwrap();
//!
//! // Nested arrays
//! let mut nested_array = object_writer.array("array").unwrap();
//! nested_array.value(42u32).unwrap();
//! nested_array.value("?").unwrap();
//! nested_array.end().unwrap();
//!
//! // Nested objects
//! let nested_object = object_writer.object("object").unwrap();
//! nested_object.end().unwrap();
//!
//! object_writer.end().unwrap();
//! assert_eq!(&object_str, "{\"number\":42,\"slice\":[1,2,3,4],\"array\":[42,\"?\"],\"object\":{}}");
//! ```
//!
//! ## Writing large files
//!
//! You can manually flush the buffer to a file in order to write large files without running out of memory.
//!
//! Example:
//!
//! ```
//! use json_writer::write_array;
//! use std::io::Write;
//!
//! fn write_numbers(file: &mut std::fs::File) -> std::io::Result<()> {
//!     let mut buffer = String::new();
//!     let mut array = write_array(&mut buffer).unwrap();
//!     for i in 1i32 ..= 1000000i32 {
//!         array.value(i).unwrap();
//!         let buffer = array.writer_mut();
//!         if buffer.len() > 2000 {
//!             // Manual flush
//!             let written = file.write(buffer.as_bytes())?;
//!             drop(buffer.drain(0..written));
//!         }
//!     }
//!     array.end().unwrap();
//!     std::io::Write::write_all(file, buffer.as_bytes())?;
//!
//!     Ok(())
//! }
//! ```
//!
//! # Limitations
//!
//! Because there is no intermediate representations, all values must be written in the order they appear in the JSON output.
//! The Borrow checker ensures sub-objects are closed before anything else can be written after them.
//! ```compile_fail
//! use json_writer::write_object;
//!
//! let mut object_str = String::new();
//! let mut object_writer = write_object(&mut object_str).unwrap();
//! let mut nested_a = object_writer.object("a").unwrap();
//! let mut nested_b = object_writer.object("b").unwrap();
//!
//! // Compile error: The borrow checker ensures the values are appended in the correct order.
//! // You can only write one object at a time.
//! nested_a.member("id", "a").unwrap();
//! nested_b.member("id", "b").unwrap();
//! ```
//!
//! The writer does **not** check for duplicate keys
//!
//! ```
//! use json_writer::write_object;
//!
//! let mut object_str = String::new();
//!
//! let mut object_writer = write_object(&mut object_str).unwrap();
//! object_writer.member("number", 42i32).unwrap();
//! object_writer.member("number", 43i32).unwrap();
//! object_writer.end().unwrap();
//!
//! assert_eq!(&object_str, "{\"number\":42,\"number\":43}");
//! ```
//!

type WriteResult = Result<(), std::fmt::Error>;

///
/// Helper for appending a JSON object to the borrowed writer.
///
/// Can be created with [`write_object`].
///
/// Appends '{' on creation.
/// Appends '}' when closed.
///
pub struct JSONObjectWriter<'a, W: std::fmt::Write> {
    writer: &'a mut W,
    empty: bool,
}

///
/// Helper for appending a JSON array to the borrowed writer.
///
/// Can be created with [`write_array`].
///
/// Appends '[' on creation.
/// Appends ']' when closed.
///
pub struct JSONArrayWriter<'a, W: std::fmt::Write> {
    writer: &'a mut W,
    empty: bool,
}

///
/// Represents the null value in json.
///
/// **Note**: [`Option::None`] may be used instead in most cases.
///
#[derive(Debug, Copy, Clone)]
pub struct Null;

impl<'a, W: std::fmt::Write> JSONObjectWriter<'a, W> {
    ///
    /// Creates a new JSONObjectWriter that writes to the given buffer.
    ///
    /// Writes '{' to the buffer immediately.
    ///
    #[inline(always)]
    fn new(buffer: &'a mut W) -> Result<JSONObjectWriter<'a, W>, std::fmt::Error> {
        buffer.write_char('{')?;
        Ok(JSONObjectWriter {
            writer: buffer,
            empty: true,
        })
    }

    ///
    /// Starts writing a nested object with given key:
    ///
    /// Escapes key, writes ",\"key\":{" and returns a JSONObjectWriter
    /// The ',' is only written if this is the first member.
    ///
    #[inline(always)]
    pub fn object<'b>(&'b mut self, key: &str) -> Result<JSONObjectWriter<'b, W>, std::fmt::Error> {
        self.write_key(key)?;
        JSONObjectWriter::new(self.writer)
    }

    ///
    /// Starts writing a nested array with given key:
    ///
    /// Escapes key, writes ",\"key\":[" and returns a JSONArrayWriter.
    /// The ',' is only written if this is the first member.
    ///
    #[inline(always)]
    pub fn array<'b>(&'b mut self, key: &str) -> Result<JSONArrayWriter<'b, W>, std::fmt::Error> {
        self.write_key(key)?;
        JSONArrayWriter::new(self.writer)
    }

    ///
    /// Appends a new object member to the buffer.
    ///
    /// Escapes key and writes ",\"key\":value" to the buffer.
    /// The ',' is only written if this is the first member.
    ///
    #[inline(always)]
    pub fn member<T: JSONWriterValue>(&mut self, key: &str, value: T) -> WriteResult {
        self.write_key(key)?;
        value.write_json(self.writer)
    }

    ///
    /// Writes a key without any value.
    ///
    /// Escapes key and writes ",\"key\":".
    /// The ',' is only written if this is the first key.
    ///
    /// Consider using the methods value(key, value), object(key) and array(key) instead of using this method directly.
    ///
    /// <p style="background:rgba(255,181,77,0.16);padding:0.75em;">
    /// <strong>Warning:</strong>
    /// If you use this method, you will have to write the value to the buffer yourself afterwards.
    /// </p>
    ///
    #[inline(never)]
    pub fn write_key(&mut self, key: &str) -> WriteResult {
        self.write_comma()?;
        write_string(self.writer, key)?;
        self.writer.write_char(':')
    }

    ///
    /// Writes a comma unless at the beginning of the object.
    ///
    /// <p style="background:rgba(255,181,77,0.16);padding:0.75em;">
    /// <strong>Warning:</strong>
    /// If you use this method, you will have to write the value to the buffer youself afterwards.
    /// </p>
    ///
    // #[inline(never)]
    pub fn write_comma(&mut self) -> WriteResult {
        if self.empty {
            self.empty = false;
            Ok(())
        } else {
            self.writer.write_char(',')
        }
    }

    ///
    /// Returns a borrow of the encapsulated writer.
    ///
    pub fn writer(&self) -> &W {
        self.writer
    }

    ///
    /// Returns a mutable borrow of the encapsulated writer.
    ///
    /// Make sure you know what you are doing if you are using the writer directly.
    /// Especially use the `write_comma` method when manually appending members to make sure
    /// the internal state is kept intact.
    ///
    pub fn writer_mut(&mut self) -> &mut W {
        self.writer
    }

    ///
    /// Consumes this writer.
    ///
    /// Writes '}' to the encapsulated writer.
    ///
    /// Prefer using this method instead of dropping the writer directly because
    /// dropping ignores any errors the encapsulated writer might produce.
    ///
    #[inline(always)]
    pub fn end(self) -> WriteResult {
        let result = self.writer.write_char('}');
        // make sure we don't write it twice
        std::mem::forget(self);
        result
    }
}

///
/// Dropping ignores any errors that might occur in the encapsulated writer.
///
impl<W: std::fmt::Write> Drop for JSONObjectWriter<'_, W> {
    #[inline(always)]
    fn drop(&mut self) {
        let _ignored = self.writer.write_char('}');
    }
}

impl<'a, W: std::fmt::Write> JSONArrayWriter<'a, W> {
    ///
    /// Creates a new JSONArrayWriter that writes to the given buffer.
    ///
    /// Writes '[' to the buffer immediately.
    ///
    #[inline(always)]
    fn new(buffer: &'a mut W) -> Result<JSONArrayWriter<'a, W>, std::fmt::Error> {
        buffer.write_char('[')?;
        Ok(JSONArrayWriter {
            writer: buffer,
            empty: true,
        })
    }

    ///
    /// Starts writing a nested object as array entry.
    ///
    /// Writes '{' and returns a JSONObjectWriter
    ///
    #[inline(always)]
    pub fn object(&mut self) -> Result<JSONObjectWriter<'_, W>, std::fmt::Error> {
        self.write_comma()?;
        JSONObjectWriter::new(self.writer)
    }

    ///
    /// Starts writing a nested array as array entry.
    ///
    /// Writes '[' and returns a JSONArrayWriter
    ///
    #[inline(always)]
    pub fn array(&mut self) -> Result<JSONArrayWriter<'_, W>, std::fmt::Error> {
        self.write_comma()?;
        JSONArrayWriter::new(self.writer)
    }

    ///
    /// Writes given value as array entry.
    ///
    /// Writes ",value" to the buffer.
    /// The ',' is only written if this is the first member.
    ///
    #[inline(always)]
    pub fn value<T: JSONWriterValue>(&mut self, value: T) -> WriteResult {
        self.write_comma()?;
        value.write_json(self.writer)
    }

    ///
    /// Writes a comma unless at the beginning of the array
    ///
    /// <p style="background:rgba(255,181,77,0.16);padding:0.75em;">
    /// <strong>Warning:</strong>
    /// If you use this method, you will have to write the value to the buffer yourself afterwards.
    /// </p>
    ///
    // #[inline(never)]
    pub fn write_comma(&mut self) -> WriteResult {
        if self.empty {
            self.empty = false;
            Ok(())
        } else {
            self.writer.write_char(',')
        }
    }

    ///
    /// Returns a borrow of the encapsulated writer.
    ///
    pub fn writer(&self) -> &W {
        self.writer
    }

    ///
    /// Returns a mutable borrow of the encapsulated writer.
    ///
    /// Make sure you know what you are doing if you are using the writer directly.
    /// Especially use the `write_comma` method to manually append values to make sure
    /// the internal state is kept intact.
    ///
    pub fn writer_mut(&mut self) -> &mut W {
        self.writer
    }

    ///
    /// Consumes this writer.
    ///
    /// Writes ']' to the encapsulated writer.
    ///
    /// Prefer using this method instead of dropping the writer directly because
    /// dropping ignores any errors the encapsulated writer might produce.
    ///
    #[inline(always)]
    pub fn end(self) -> WriteResult {
        let result = self.writer.write_char(']');
        // make sure we don't write it twice
        std::mem::forget(self);
        result
    }
}

///
/// Dropping ignores any errors that might occur in the encapsulated writer.
///
impl<W: std::fmt::Write> Drop for JSONArrayWriter<'_, W> {
    #[inline(always)]
    fn drop(&mut self) {
        let _ignored = self.writer.write_char(']');
    }
}

///
/// Types with this trait can be converted to JSON
///
pub trait JSONWriterValue {
    ///
    /// Appends a JSON representation of self to the output buffer
    ///
    fn write_json<W: std::fmt::Write>(self, output_buffer: &mut W) -> WriteResult;
}

///
/// Serializes as a JSON string.
///
impl JSONWriterValue for &str {
    #[inline(always)]
    fn write_json<W: std::fmt::Write>(self, output_buffer: &mut W) -> WriteResult {
        write_string(output_buffer, self)
    }
}

///
/// Serializes as a JSON string.
///
impl JSONWriterValue for &String {
    #[inline(always)]
    fn write_json<W: std::fmt::Write>(self, output_buffer: &mut W) -> WriteResult {
        write_string(output_buffer, self)
    }
}

///
/// Serializes as a JSON number.
///
/// If value is finite then value is converted to string and appended to buffer.
/// If value is NaN or infinity, then the string "null" is appended to buffer (without the quotes).
///
impl JSONWriterValue for f64 {
    #[inline(always)]
    fn write_json<W: std::fmt::Write>(self, output_buffer: &mut W) -> WriteResult {
        write_float(output_buffer, self)
    }
}

///
/// Serializes as a JSON number.
///
/// If value is finite then value is converted to string and appended to buffer.
/// If value is NaN or infinity, then the string "null" is appended to buffer (without the quotes).
///
impl JSONWriterValue for f32 {
    #[inline(always)]
    fn write_json<W: std::fmt::Write>(self, output_buffer: &mut W) -> WriteResult {
        write_float(output_buffer, self as f64)
    }
}

///
/// Serializes as a JSON number.
///
impl JSONWriterValue for u32 {
    #[inline(always)]
    fn write_json<W: std::fmt::Write>(self, output_buffer: &mut W) -> WriteResult {
        let mut buf = itoa::Buffer::new();
        output_buffer.write_str(buf.format(self))
    }
}

///
/// Serializes as a JSON number.
///
impl JSONWriterValue for i32 {
    #[inline(always)]
    fn write_json<W: std::fmt::Write>(self, output_buffer: &mut W) -> WriteResult {
        let mut buf = itoa::Buffer::new();
        output_buffer.write_str(buf.format(self))
    }
}

///
/// Serializes as a JSON number.
///
impl JSONWriterValue for u16 {
    #[inline(always)]
    fn write_json<W: std::fmt::Write>(self, output_buffer: &mut W) -> WriteResult {
        let mut buf = itoa::Buffer::new();
        output_buffer.write_str(buf.format(self))
    }
}

///
/// Serializes as a JSON number.
///
impl JSONWriterValue for i16 {
    #[inline(always)]
    fn write_json<W: std::fmt::Write>(self, output_buffer: &mut W) -> WriteResult {
        let mut buf = itoa::Buffer::new();
        output_buffer.write_str(buf.format(self))
    }
}

///
/// Serializes as a JSON number.
///
impl JSONWriterValue for u8 {
    #[inline(always)]
    fn write_json<W: std::fmt::Write>(self, output_buffer: &mut W) -> WriteResult {
        let mut buf = itoa::Buffer::new();
        output_buffer.write_str(buf.format(self))
    }
}

///
/// Serializes as a JSON number.
///
impl JSONWriterValue for i8 {
    #[inline(always)]
    fn write_json<W: std::fmt::Write>(self, output_buffer: &mut W) -> WriteResult {
        let mut buf = itoa::Buffer::new();
        output_buffer.write_str(buf.format(self))
    }
}

///
/// Serializes as a JSON boolean.
///
impl JSONWriterValue for bool {
    #[inline(always)]
    fn write_json<W: std::fmt::Write>(self, output_buffer: &mut W) -> WriteResult {
        output_buffer.write_str(if self { "true" } else { "false" })
    }
}

///
/// Serializes as a JSON null.
///
impl JSONWriterValue for Null {
    #[inline(always)]
    fn write_json<W: std::fmt::Write>(self, output_buffer: &mut W) -> WriteResult {
        output_buffer.write_str("null")
    }
}

impl<T: JSONWriterValue + Copy> JSONWriterValue for &T {
    #[inline(always)]
    fn write_json<W: std::fmt::Write>(self, output_buffer: &mut W) -> WriteResult {
        (*self).write_json(output_buffer)
    }
}

// impl JSONWriterValue for serde_json::value::Value::Null {
//     #[inline(always)]
//     fn write_json(&self, output_buffer: &mut String) {
//         buffer.write_str("null");
//     }
// }

///
/// Serializes either as a JSON null or the encapsulated value.
///
impl<T: JSONWriterValue> JSONWriterValue for Option<T> {
    #[inline(always)]
    fn write_json<W: std::fmt::Write>(self, output_buffer: &mut W) -> WriteResult {
        match self {
            None => output_buffer.write_str("null"),
            Some(value) => value.write_json(output_buffer),
        }
    }
}

///
/// Serializes as a JSON array.
///
impl<Item> JSONWriterValue for &Vec<Item>
where
    for<'b> &'b Item: JSONWriterValue,
{
    #[inline(always)]
    fn write_json<W: std::fmt::Write>(self, output_buffer: &mut W) -> WriteResult {
        (&self[..]).write_json(output_buffer)
    }
}

///
/// Serializes as a JSON array.
///
impl<Item> JSONWriterValue for &[Item]
where
    for<'b> &'b Item: JSONWriterValue,
{
    fn write_json<W: std::fmt::Write>(self, output_buffer: &mut W) -> WriteResult {
        let mut array = JSONArrayWriter::new(output_buffer)?;
        for item in self.iter() {
            array.value(item)?;
        }
        Ok(())
    }
}

///
/// Serializes as a JSON object.
///
impl<Key: AsRef<str>, Item> JSONWriterValue for &std::collections::HashMap<Key, Item>
where
    for<'b> &'b Item: JSONWriterValue,
{
    fn write_json<W: std::fmt::Write>(self, output_buffer: &mut W) -> WriteResult {
        let mut obj = JSONObjectWriter::new(output_buffer)?;
        for (key, value) in self.iter() {
            obj.member(key.as_ref(), value)?;
        }
        Ok(())
    }
}

///
/// Serializes as a JSON object.
///
impl<Key: AsRef<str>, Item> JSONWriterValue for &std::collections::BTreeMap<Key, Item>
where
    for<'b> &'b Item: JSONWriterValue,
{
    fn write_json<W: std::fmt::Write>(self, output_buffer: &mut W) -> WriteResult {
        let mut obj = JSONObjectWriter::new(output_buffer)?;
        for (key, value) in self.iter() {
            obj.member(key.as_ref(), value)?;
        }
        Ok(())
    }
}

///
/// Serializes the given `value` to JSON.
///
/// This is the same as calling [`write_value`] with an empty [`String`] as buffer.
///
#[inline]
pub fn to_json_string<T: JSONWriterValue>(value: T) -> String {
    let mut result = String::new();
    // String never returns an error in it's Write implementation.
    value.write_json(&mut result).unwrap();
    result
}

///
/// Writes the `value` as JSON to the `output_buffer`.
///
pub fn write_value<W: std::fmt::Write, T: JSONWriterValue>(
    output_buffer: &mut W,
    value: T,
) -> WriteResult {
    value.write_json(output_buffer)
}

///
/// Borrows the `output_buffer` and starts writing an object.
///
/// Creates and returns a new [`JSONObjectWriter`] that writes to the given buffer.
/// Use it to append members and finish the object.
///
/// Writes '{' to the buffer immediately.
///
pub fn write_object<W: std::fmt::Write>(
    output_buffer: &mut W,
) -> Result<JSONObjectWriter<'_, W>, std::fmt::Error> {
    JSONObjectWriter::new(output_buffer)
}

///
/// Borrows the `output_buffer` and starts writing an array.
///
/// Creates and returns a new [`JSONArrayWriter`] that writes to the given buffer.
/// Use it to append values and finish the array.
///
/// Writes '[' to the buffer immediately.
///
pub fn write_array<W: std::fmt::Write>(
    output_buffer: &mut W,
) -> Result<JSONArrayWriter<'_, W>, std::fmt::Error> {
    JSONArrayWriter::new(output_buffer)
}

///
/// Quotes and escapes `input` and appends result to `output_buffer`.
///
#[inline(never)]
fn write_string<W: std::fmt::Write>(output_buffer: &mut W, input: &str) -> WriteResult {
    output_buffer.write_char('"')?;
    write_part_of_string_impl(output_buffer, input)?;
    output_buffer.write_char('"')?;
    Ok(())
}

///
/// Escapes `input` and appends result to `output_buffer` without adding quotes.
///
/// <p style="background:rgba(255,181,77,0.16);padding:0.75em;">
/// <strong>Warning:</strong>
/// If you use this function in conjunction with the rest of this library, you have to make
/// sure to adhere to the JSON format yourself.
/// </p>
///
/// Call [`write_value`] with a [`&str`] argument to serialize a complete JSON string value
/// including the quotes enclosing it.
///
#[inline(never)]
pub fn write_part_of_string<W: std::fmt::Write>(output_buffer: &mut W, input: &str) -> WriteResult {
    write_part_of_string_impl(output_buffer, input)
}

const fn get_replacements() -> [u8; 256] {
    // NOTE: only characters smaller than 128 are allowed here
    // see https://www.json.org/json-en.html
    let mut result = [0u8; 256];
    result[b'"' as usize] = b'"';
    result[b'\\' as usize] = b'\\';
    result[b'/' as usize] = b'/';

    let mut c: u8 = 0x00;
    while c < 0x20 {
        // mark all control characters 0x00 <= c < 0x20 as being replaced by a unicode escape
        result[c as usize] = b'u';
        c += 1;
    }

    // overwrite characters that have shorter escapes
    result[0x08] = b'b';
    result[0x0c] = b'f';
    result[b'\n' as usize] = b'n';
    result[b'\r' as usize] = b'r';
    result[b'\t' as usize] = b't';

    let mut c: u8 = 0x80;
    loop {
        if result[c as usize] != 0 {
            panic!("bytes from 0x80 to 0xFF are parts of UTF-8 multi-byte characters and must not be modified");
        }
        c = match c.checked_add(1) {
            Some(c) => c,
            None => break,
        };
    }

    result
}
static REPLACEMENTS: [u8; 256] = get_replacements();
static HEX: [u8; 16] = *b"0123456789ABCDEF";

///
/// Escapes and append part of string
///
#[inline(always)]
fn write_part_of_string_impl<W: std::fmt::Write>(
    output_buffer: &mut W,
    input: &str,
) -> WriteResult {
    // All of the relevant characters are in the ansi range (<128).
    // This means we can safely ignore any utf-8 characters and iterate over the bytes directly
    let mut num_bytes_written: usize = 0;
    let mut index: usize = 0;
    let bytes = input.as_bytes();
    while index < bytes.len() {
        let cur_byte = bytes[index];
        let replacement = REPLACEMENTS[cur_byte as usize];
        if replacement != 0 {
            if num_bytes_written < index {
                // Checks can be ommitted here:
                // We know that index is smaller than the output_buffer length.
                // We also know that num_bytes_written is smaller than index
                // We also know that the boundaries are not in the middle of an utf-8 multi byte sequence, because those characters are not escaped
                output_buffer
                    .write_str(unsafe { input.get_unchecked(num_bytes_written..index) })?;
            }
            if replacement == b'u' {
                let bytes: [u8; 6] = [
                    b'\\',
                    b'u',
                    b'0',
                    b'0',
                    HEX[(cur_byte >> 4) as usize],
                    HEX[(cur_byte & 0xF) as usize],
                ];
                // Checks can be ommitted here: We know bytes is a valid utf-8 string (see above)
                output_buffer.write_str(unsafe { std::str::from_utf8_unchecked(&bytes) })?;
            } else {
                let bytes: [u8; 2] = [b'\\', replacement];
                // Checks can be ommitted here: We know bytes is a valid utf-8 string, because the replacement table only contains characters smaller than 128
                output_buffer.write_str(unsafe { std::str::from_utf8_unchecked(&bytes) })?;
            }
            num_bytes_written = index + 1;
        }
        index += 1;
    }
    if num_bytes_written < bytes.len() {
        // Checks can be ommitted here:
        // We know that num_bytes_written is smaller than index
        // We also know that num_bytes_written not in the middle of an utf-8 multi byte sequence, because those are not escaped
        output_buffer.write_str(unsafe { input.get_unchecked(num_bytes_written..bytes.len()) })?;
    }
    Ok(())
}

///
/// If value is finite then value is converted to string and appended to buffer.
/// If value is NaN or infinity, then the string "null" is appended to buffer (without the quotes)
///
#[inline(never)]
fn write_float<W: std::fmt::Write>(output_buffer: &mut W, value: f64) -> WriteResult {
    if !value.is_finite() {
        // JSON does not allow infinite or nan values. In browsers JSON.stringify(Number.NaN) = "null"
        output_buffer.write_str("null")?;
        return Ok(());
    }

    // let mut buf = dtoa::Buffer::new();
    // let mut result = buf.format_finite(v);

    let mut buf = ryu::Buffer::new();
    let mut result = buf.format_finite(value);
    if result.ends_with(".0") {
        result = unsafe { result.get_unchecked(..result.len() - 2) };
    }
    // workaround for dtoa
    // if v < 0.0 && result != "0" {
    //     buffer.write_char('-');
    // }
    output_buffer.write_str(result)
}

// #[inline(never)]
// const fn needs_escaping(string: &str) -> usize {
//     let mut is_open = false;
//     usize mut i = 0;
//     for let b in string.bytes() {
//         match b {
//             b'\r' | b'\n' | b'\\' | b'"' => return i;
//             b'<' => is_open = true;
//             b'/' => if is_open return i; else is_open = false;
//             _ => is_open = false;
//         }
//         i += 1;
//     }
//     return usize::MAX;
// }

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_array() -> Result<(), std::fmt::Error> {
        let mut buffer = String::new();
        let mut array = write_array(&mut buffer)?;
        array.value(0u8)?;
        array.value(1i32)?;
        array.value("2")?;
        array.value("\"<script>1/2</script>\"")?;
        let mut nested_arr = array.array()?;
        nested_arr.value("nested")?;
        nested_arr.end()?;
        let mut nested_obj = array.object()?;
        nested_obj.member("ä\töü", "ä\töü")?;
        nested_obj.end()?;
        let nested_obj2 = array.object()?;
        nested_obj2.end()?;
        array.end()?;

        assert_eq!(
        buffer,
        "[0,1,\"2\",\"\\\"<script>1\\/2<\\/script>\\\"\",[\"nested\"],{\"ä\\töü\":\"ä\\töü\"},{}]"
    );

        Ok(())
    }

    #[test]
    fn test_array_range() -> Result<(), std::fmt::Error> {
        let bytes = b"ABC";
        assert_eq!(to_json_string(&bytes[..]), "[65,66,67]");

        let mut v = Vec::<u8>::new();
        v.extend_from_slice(bytes);
        assert_eq!(to_json_string(&v), "[65,66,67]");

        Ok(())
    }

    #[test]
    fn test_object() -> Result<(), std::fmt::Error> {
        let mut map = std::collections::HashMap::<String, String>::new();
        map.insert("a".to_owned(), "a".to_owned());
        assert_eq!(to_json_string(&map), "{\"a\":\"a\"}");

        Ok(())
    }

    #[test]
    #[allow(clippy::approx_constant)] // clippy detects PI
    fn test_numbers() -> Result<(), std::fmt::Error> {
        // unsigned
        assert_eq!(to_json_string(1u8), "1");
        assert_eq!(to_json_string(1u16), "1");
        assert_eq!(to_json_string(1u32), "1");
        assert_eq!(to_json_string(u8::MAX), "255");
        assert_eq!(to_json_string(u16::MAX), "65535");
        assert_eq!(to_json_string(u32::MAX), "4294967295");

        // signed
        assert_eq!(to_json_string(-1i8), "-1");
        assert_eq!(to_json_string(-1i16), "-1");
        assert_eq!(to_json_string(-1i32), "-1");

        // float
        assert_eq!(to_json_string(0f32), "0");
        assert_eq!(to_json_string(2f32), "2");
        assert_eq!(to_json_string(-2f32), "-2");

        assert_eq!(to_json_string(0f64), "0");
        assert_eq!(to_json_string(2f64), "2");
        assert_eq!(to_json_string(-2f64), "-2");
        assert_eq!(to_json_string(3.141592653589793), "3.141592653589793");
        assert_eq!(to_json_string(0.1f64), "0.1");
        assert_eq!(to_json_string(-0.1f64), "-0.1");
        //assert_eq!(to_json_string(-5.0/3.0), "-1.6666666666666667");
        assert_eq!(to_json_string(1.5e30f64), "1.5e30");
        assert_eq!(
            to_json_string(-2.220446049250313e-16f64),
            "-2.220446049250313e-16"
        );

        assert_eq!(to_json_string(1.0 / 0.0), "null");
        assert_eq!(to_json_string(std::f64::INFINITY), "null");
        assert_eq!(to_json_string(std::f64::NEG_INFINITY), "null");

        Ok(())
    }

    #[test]
    fn test_dtoa() -> Result<(), std::fmt::Error> {
        assert_dtoa(0.0)?;
        assert_dtoa(1.0)?;
        assert_dtoa(-1.0)?;
        assert_dtoa(2.0)?;
        //assert_dtoa(-5.0/3.0);

        Ok(())
    }

    #[cfg(test)]
    fn assert_dtoa(v: f64) -> Result<(), std::fmt::Error> {
        let a = v.to_string();
        let mut b = String::new();
        write_float(&mut b, v)?;
        assert_eq!(b, a);

        Ok(())
    }

    #[test]
    fn test_strings() -> Result<(), std::fmt::Error> {
        assert_eq!(
            to_json_string("中文\0\x08\x09\"\\\n\r\t</script>"),
            "\"中文\\u0000\\b\\t\\\"\\\\\\n\\r\\t<\\/script>\""
        );

        Ok(())
    }

    #[test]
    fn test_basic_example() -> Result<(), std::fmt::Error> {
        let mut object_str = String::new();

        let mut object_writer = write_object(&mut object_str)?;
        object_writer.member("number", 42i32)?;
        object_writer.end()?;

        assert_eq!(&object_str, "{\"number\":42}");

        Ok(())
    }

    #[test]
    #[allow(clippy::approx_constant)] // clippy detects PI
    fn test_misc_examples() -> Result<(), std::fmt::Error> {
        // Values
        assert_eq!(to_json_string("Hello World\n"), "\"Hello World\\n\"");
        assert_eq!(to_json_string(3.141592653589793f64), "3.141592653589793");
        assert_eq!(to_json_string(true), "true");
        assert_eq!(to_json_string(false), "false");
        assert_eq!(to_json_string(Null), "null");

        // Options of values
        assert_eq!(to_json_string(Option::<u8>::Some(42)), "42");
        assert_eq!(to_json_string(Option::<u8>::None), "null");

        // Slices and vectors
        let numbers: [u8; 4] = [1, 2, 3, 4];
        assert_eq!(to_json_string(&numbers[..]), "[1,2,3,4]");
        let numbers_vec: Vec<u8> = vec![1u8, 2u8, 3u8, 4u8];
        assert_eq!(to_json_string(&numbers_vec), "[1,2,3,4]");
        let strings: [&str; 4] = ["a", "b", "c", "d"];
        assert_eq!(to_json_string(&strings[..]), "[\"a\",\"b\",\"c\",\"d\"]");

        // Hash-maps:
        let mut map = std::collections::HashMap::<String, String>::new();
        map.insert("Hello".to_owned(), "World".to_owned());
        assert_eq!(to_json_string(&map), "{\"Hello\":\"World\"}");

        // Objects:
        let mut object_str = String::new();
        let mut object_writer = write_object(&mut object_str)?;

        // Values
        object_writer.member("number", 42i32)?;
        object_writer.member("slice", &numbers[..])?;

        // Nested arrays
        let mut nested_array = object_writer.array("array")?;
        nested_array.value(42u32)?;
        nested_array.value("?")?;
        nested_array.end()?;

        // Nested objects
        let nested_object = object_writer.object("object")?;
        nested_object.end()?;

        object_writer.end()?;
        assert_eq!(
            &object_str,
            "{\"number\":42,\"slice\":[1,2,3,4],\"array\":[42,\"?\"],\"object\":{}}"
        );

        Ok(())
    }

    #[test]
    fn test_duplicate_keys() -> Result<(), std::fmt::Error> {
        let mut object_str = String::new();

        let mut object_writer = write_object(&mut object_str)?;
        object_writer.member("number", 42i32)?;
        object_writer.member("number", 43i32)?;
        object_writer.end()?;

        // Duplicates are not checked, this is by design!
        assert_eq!(&object_str, "{\"number\":42,\"number\":43}");

        Ok(())
    }

    #[test]
    fn test_flush() -> std::io::Result<()> {
        // this could also be a file writer.
        let mut writer = Vec::<u8>::new();

        let mut buffer = String::new();
        let mut array = write_array(&mut buffer).unwrap();
        for i in 1i32..=1000000i32 {
            array.value(i).unwrap();
            let buffer = array.writer_mut();
            if buffer.len() > 2000 {
                // Manual flush
                let written = writer.write(buffer.as_bytes())?;
                drop(buffer.drain(0..written));
            }
        }
        array.end().unwrap();
        std::io::Write::write_all(&mut writer, buffer.as_bytes())?;

        if buffer.len() > 4000 {
            panic!("Buffer too long");
        }
        assert_eq!(
            &writer[writer.len() - b",999999,1000000]".len()..],
            b",999999,1000000]"
        );

        Ok(())
    }

    #[cfg(test)]
    #[allow(dead_code)]
    fn test_write_numbers(file: &mut std::fs::File) -> std::io::Result<()> {
        let mut buffer = String::new();
        let mut array = write_array(&mut buffer).unwrap();
        for i in 1i32..=1000000i32 {
            array.value(i).unwrap();
            let buffer = array.writer_mut();
            if buffer.len() > 2000 {
                // Manual flush
                let written = file.write(buffer.as_bytes())?;
                drop(buffer.drain(0..written));
            }
        }
        array.end().unwrap();
        std::io::Write::write_all(file, buffer.as_bytes())?;

        Ok(())
    }

    #[test]
    fn test_control_characters() -> Result<(), std::fmt::Error> {
        // all ascii characters 0x00 <= c < 0x20 must be escaped
        // see https://www.json.org/json-en.html

        for c in 0x00u8..0x20u8 {
            let c = char::from(c);
            let json = to_json_string(c.to_string().as_str());
            assert!(&json[0..2] == "\"\\");
        }

        Ok(())
    }
}
