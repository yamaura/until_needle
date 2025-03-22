# until_needle

`until_needle` is a Rust crate that extends the `BufRead` trait to allow reading from a buffer until a specified "needle" (search pattern) is found.
This is useful for reading a stream or buffer until a certain pattern is encountered, and is similar in concept to `read_until` but more flexible with custom patterns.

## Features

- Provides the `UntilNeedleRead` trait to extend `BufRead` functionality.
- Reads data from a buffer until a specified "needle" is found or the end of the stream is reached.
- Stores data before the needle and the needle itself separately for further processing.

## Example

```rust
use until_needle::io::UntilNeedleRead;
use std::io::{BufRead, Cursor};

fn main() {
    let data = b"hello world!!";
    let mut cursor = Cursor::new(data);

    let captures = cursor.read_until_needle(b"world").unwrap();
    assert_eq!(captures.total_bytes_read(), b"hello world".len());
    assert_eq!(captures.before(), b"hello ");
    assert_eq!(captures.matched(), b"world");
}
```

This code reads the data until the pattern "world" is found, storing the data before the needle in the before buffer and the needle itself in the matched buffer.
The `until_needle` crate also supports regular expressions (`regex`) as the needle.

