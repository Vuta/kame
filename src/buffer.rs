pub const DEFAULT_GAP_LEN: usize = 1024;
const NULL: u8 = b'\0';

#[derive(Debug)]
pub struct Buffer {
    // insertion pointer (iptr)
    // [...|                     |.............]
    //     <-      gap len      ->
    // iptr -> points to the first slot of the gap
    // iptr - 1 -> points to the last byte before the gap
    // iptr + gap_len -> points to the first byte after the gap
    pub iptr: usize,
    gap_len: usize,
    // since `char` has a fixed size of 4-byte,
    // using `u8` is more memory efficient, but it has to parse the bytes to char manually
    bytes: Vec<u8>,
}

#[derive(Debug)]
pub struct BufferIter<'a> {
    buf: &'a Buffer,
    current: usize,
}

impl<'a> BufferIter<'a> {
    pub fn seek(&mut self, i: usize) {
        let mut new_iter = self.buf.iter();

        for _ in 0..i {
            new_iter.next();
        }

        *self = new_iter;
    }
}

impl<'a> Iterator for BufferIter<'a> {
    type Item = &'a u8;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current == self.buf.bytes.len() {
            return None;
        }

        let gap_end = self.buf.iptr + self.buf.gap_len;

        if self.current > self.buf.iptr && self.current < gap_end {
            panic!("corrupted iterator");
        }

        if self.current == self.buf.iptr {
            self.current += self.buf.gap_len;

            if self.current >= self.buf.bytes.len() {
                return None;
            }
        }

        let res = Some(&self.buf.bytes[self.current]);
        self.current += 1;

        res
    }
}

impl Buffer {
    pub fn init(mut s: String) -> Self {
        let mut buffer = Vec::with_capacity(DEFAULT_GAP_LEN + s.len());
        buffer.append(&mut vec![0u8; DEFAULT_GAP_LEN]);

        unsafe {
            buffer.append(&mut s.as_mut_vec());
        }

        Self {
            bytes: buffer,
            iptr: 0,
            gap_len: DEFAULT_GAP_LEN,
        }
    }

    pub fn iter(&self) -> BufferIter<'_> {
        BufferIter {
            buf: self,
            current: 0,
        }
    }

    pub fn jump(&mut self, n: usize) {
        if n < self.iptr {
            for _ in 0..self.iptr - n {
                self.move_ptr_backward();
            }
        } else {
            for _ in 0..n - self.iptr {
                self.move_ptr_forward();
            }
        }
    }

    pub fn before_insertion_point(&self) -> &[u8] {
        &self.bytes[..self.iptr]
    }

    pub fn after_insertion_point(&self) -> &[u8] {
        &self.bytes[self.iptr + self.gap_len..]
    }

    // the first "char" after the gap is moved to the slot(s) at the beginning of the gap
    // iptr points to the next slot in the gap
    pub fn move_ptr_forward(&mut self) {
        if self.iptr + self.gap_len == self.bytes.len() {
            return;
        }

        let i = self.iptr + self.gap_len;
        for j in 0..size_of::<char>() {
            self.bytes[self.iptr + j] = self.bytes[i + j];

            if let Ok(_) = str::from_utf8(&self.bytes[i..=i + j]) {
                self.iptr += j + 1;
                return;
            }
        }

        panic!("corrupted utf8");
    }

    // the last "char" before the gap is moved to the slot(s) after the gap
    pub fn move_ptr_backward(&mut self) {
        if self.iptr == 0 {
            return;
        }

        let i = self.iptr - 1;
        for j in 0..size_of::<char>() {
            self.bytes[i - j + self.gap_len] = self.bytes[i - j];
            if let Ok(_) = str::from_utf8(&self.bytes[i - j..=i]) {
                self.iptr -= j + 1;

                return;
            }
        }

        panic!("corrupted utf8");
    }

    pub fn insert(&mut self, c: char) {
        // when the gap is less than 25% of the current buffer, double the buffer size
        // this increases the gap size to = current gap size + previous buffer size
        if self.gap_len < self.bytes.len() / 4 {
            let old_len = self.bytes.len();
            self.bytes.resize(old_len * 2, NULL);
            let new_len = self.bytes.len();

            for i in (self.iptr + self.gap_len)..old_len {
                self.bytes[i + new_len - old_len] = self.bytes[i];
                self.bytes[i] = NULL;
            }

            self.gap_len += old_len;
        }

        assert!(
            self.gap_len >= size_of::<char>(),
            "not enough space for insertion"
        );

        let s = String::from(c);
        let s_bytes = s.as_bytes();
        let c_len = s_bytes.len();

        for i in 0..c_len {
            self.bytes[self.iptr + i] = s_bytes[i];
        }

        self.iptr += c_len;
        self.gap_len -= c_len;
    }

    pub fn revert_insert(&mut self, prev_iptr: usize, n: usize) {
        self.jump(prev_iptr);
        self.gap_len = (self.gap_len + n).min(self.bytes.len());
    }

    pub fn revert_delete_before_ptr(&mut self, prev_iptr: usize, deleted: &Vec<u8>) {
        self.jump(prev_iptr);
        self.gap_len = self.gap_len.saturating_sub(deleted.len());

        let new_iptr = self.iptr + deleted.len();
        for i in self.iptr..new_iptr {
            self.bytes[i] = deleted[i - self.iptr];
        }

        self.iptr = new_iptr;
    }

    pub fn delete_before_ptr(&mut self) -> Option<Vec<u8>> {
        if self.iptr == 0 {
            return None;
        }

        let mut i = self.iptr.saturating_sub(1);
        let mut res = Vec::new();
        for _ in 0..size_of::<char>() {
            res.push(self.bytes[i]);

            if let Ok(_) = str::from_utf8(&self.bytes[i..self.iptr]) {
                let n = self.iptr - i;
                self.gap_len += n;
                self.iptr = i;

                return Some(res);
            }

            i = i.saturating_sub(1);
        }

        panic!("corrupted utf8");
    }

    pub fn revert_delete_after_ptr(&mut self, prev_iptr: usize, deleted: &Vec<u8>) {
        self.jump(prev_iptr);
        self.gap_len = self.gap_len.saturating_sub(deleted.len());

        for i in self.iptr..self.iptr + deleted.len() {
            self.bytes[i + self.gap_len] = deleted[i - self.iptr];
        }
    }

    pub fn delete_after_ptr(&mut self) -> Option<Vec<u8>> {
        let i = self.iptr + self.gap_len;
        if i == self.bytes.len() {
            return None;
        }

        let mut res = Vec::new();
        for j in 0..size_of::<char>() {
            res.push(self.bytes[i]);

            if let Ok(_) = str::from_utf8(&self.bytes[i..=i + j]) {
                self.gap_len = (self.gap_len + j + 1).min(self.bytes.len());
                return Some(res);
            }
        }

        panic!("corrupted utf8");
    }

    #[cfg(test)]
    pub fn to_string(&self) -> String {
        let end = self.iptr + self.gap_len;
        let mut b = self.bytes[..self.iptr].to_vec();
        b.append(&mut self.bytes[end..].to_vec());

        String::from_utf8(b).expect("BUG!!!")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_buffer_init_1() {
        let buf = Buffer::init(String::from("a bc"));
        assert_eq!(buf.to_string(), "a bc");
        assert_eq!(&buf.bytes[..DEFAULT_GAP_LEN], [0; DEFAULT_GAP_LEN]);
        assert_eq!(buf.iptr, 0);
        assert_eq!(buf.gap_len, DEFAULT_GAP_LEN);
    }

    #[test]
    fn test_buffer_init_2() {
        let buf = Buffer::init(String::from(""));
        assert_eq!(buf.to_string(), "");
        assert_eq!(&buf.bytes[..DEFAULT_GAP_LEN], [0; DEFAULT_GAP_LEN]);
        assert_eq!(buf.iptr, 0);
        assert_eq!(buf.gap_len, DEFAULT_GAP_LEN);
    }

    #[test]
    fn test_buffer_insert_1() {
        let mut buf = Buffer::init(String::from("a"));
        buf.insert('b');
        assert_eq!(buf.to_string(), "ba");
        assert_eq!(buf.iptr, 1);
        assert_eq!(buf.gap_len, 1023);

        buf.insert('🧑');
        assert_eq!(buf.to_string(), "b🧑a");
        assert_eq!(buf.iptr, 5);
        assert_eq!(buf.gap_len, 1019);
    }

    #[test]
    fn test_buffer_insert_2() {
        let mut buf = Buffer::init(String::from("s"));
        for _ in 0..819 {
            buf.insert('a');
        }

        assert_eq!(buf.bytes.len(), 2050); // (1 + DEFAULT_GAP_LEN) * 2
        assert_eq!(buf.gap_len, 1230); // (1 + DEFAULT_GAP_LEN) + (DEFAULT_GAP_LEN - 819)
        assert_eq!(buf.iptr, 819);
        assert_eq!(buf.bytes[buf.iptr + buf.gap_len], b's');
    }

    #[test]
    fn test_buffer_delete_before_ptr_1() {
        let mut buf = Buffer::init(String::from(""));
        let gap_len = buf.gap_len;
        buf.insert('🧑'); // 4-byte char

        buf.delete_before_ptr();
        assert_eq!(buf.iptr, 0);
        assert_eq!(buf.to_string(), "");
        assert_eq!(buf.gap_len, gap_len);
    }

    #[test]
    fn test_buf_delete_before_ptr_2() {
        let mut buf = Buffer::init(String::from(""));
        let gap_len = buf.gap_len;
        buf.insert('🧑'); // 4-byte char
        buf.insert('a');

        buf.delete_before_ptr();
        assert_eq!(buf.iptr, 4);
        assert_eq!(buf.to_string(), "🧑");
        assert_eq!(buf.gap_len, gap_len - 4);
    }

    #[test]
    fn test_buf_delete_before_ptr_3() {
        let mut buf = Buffer::init(String::from(""));
        let gap_len = buf.gap_len;
        buf.insert('a');
        buf.insert('🧑'); // 4-byte char

        buf.delete_before_ptr();
        assert_eq!(buf.iptr, 1);
        assert_eq!(buf.to_string(), "a");
        assert_eq!(buf.gap_len, gap_len - 1);
    }

    #[test]
    fn test_buf_delete_before_ptr_4() {
        let mut buf = Buffer::init(String::from("a"));
        let gap_len = buf.gap_len;

        buf.delete_before_ptr();
        assert_eq!(buf.iptr, 0);
        assert_eq!(buf.to_string(), "a");
        assert_eq!(buf.gap_len, gap_len);
    }

    #[test]
    fn test_buf_revert_delete_1() {
        let mut buf = Buffer::init(String::from(""));
        buf.insert('a');
        buf.insert('b');
        buf.insert('c');
        let prev_iptr = buf.iptr;
        let n = buf.delete_before_ptr().unwrap();

        buf.move_ptr_backward();
        buf.move_ptr_backward();
        assert_eq!(buf.to_string(), "ab");

        buf.revert_delete_before_ptr(prev_iptr, &n);
        assert_eq!(buf.to_string(), "abc");
    }

    #[test]
    fn test_buf_revert_delete_4() {
        let mut buf = Buffer::init(String::from("hello"));
        for _ in 0..4 {
            buf.move_ptr_forward();
        }

        let n = buf.delete_after_ptr().unwrap();
        let prev_iptr = buf.iptr;
        assert_eq!(buf.to_string(), "hell");

        for _ in 0..5 {
            buf.move_ptr_backward();
        }

        buf.revert_delete_after_ptr(prev_iptr, &n);
        assert_eq!(buf.to_string(), "hello");
    }

    #[test]
    fn test_buf_revert_delete_2() {
        let mut buf = Buffer::init(String::from("hello"));

        for _ in 0..5 {
            buf.move_ptr_forward();
        }

        let n = buf.delete_before_ptr().unwrap();
        let prev = buf.iptr;

        for _ in 0..5 {
            buf.move_ptr_backward();
        }

        buf.revert_delete_before_ptr(prev, &n);
        assert_eq!(buf.to_string(), "hello");
    }

    #[test]
    fn test_buf_revert_delete_3() {
        let mut buf = Buffer::init(String::from("ab"));
        buf.move_ptr_forward();

        let n = buf.delete_before_ptr().unwrap();
        let prev_iptr = buf.iptr;

        buf.move_ptr_forward();
        assert_eq!(buf.to_string(), "b");

        buf.revert_delete_before_ptr(prev_iptr, &n);
        assert_eq!(buf.to_string(), "ab");
    }

    #[test]
    fn test_buf_delete_before_ptr_5() {
        let mut buf = Buffer::init(String::from(""));
        buf.insert('a');
        buf.insert('\u{1F9D1}');
        buf.insert('\u{200D}');
        buf.insert('\u{1F33E}');
        buf.insert('b');

        buf.delete_before_ptr();
        assert_eq!(buf.to_string(), "a🧑‍🌾");

        buf.delete_before_ptr();
        assert_eq!(buf.to_string(), "a🧑‍");

        buf.delete_before_ptr();
        assert_eq!(buf.to_string(), "a🧑");

        buf.delete_before_ptr();
        assert_eq!(buf.to_string(), "a");

        buf.delete_before_ptr();
        assert_eq!(buf.to_string(), "");

        assert_eq!(buf.iptr, 0);
    }

    #[test]
    fn test_buf_delete_after_ptr_1() {
        let mut buf = Buffer::init(String::from("🧑"));
        let gap_len = buf.gap_len;
        assert_eq!(buf.iptr, 0);

        buf.delete_after_ptr();
        assert_eq!(buf.iptr, 0);
        assert_eq!(buf.to_string(), "");
        assert_eq!(buf.gap_len, gap_len + 4);

        buf.insert('a');
        buf.delete_after_ptr();
    }

    #[test]
    fn test_buf_delete_after_ptr_2() {
        let mut buf = Buffer::init(String::from("a"));
        let gap_len = buf.gap_len;
        assert_eq!(buf.iptr, 0);

        buf.delete_after_ptr();
        assert_eq!(buf.iptr, 0);
        assert_eq!(buf.to_string(), "");
        assert_eq!(buf.gap_len, gap_len + 1);
    }

    #[test]
    fn test_buf_delete_after_ptr_3() {
        let mut buf = Buffer::init(String::from(""));
        let gap_len = buf.gap_len;
        assert_eq!(buf.iptr, 0);

        buf.delete_after_ptr();
        assert_eq!(buf.iptr, 0);
        assert_eq!(buf.to_string(), "");
        assert_eq!(buf.gap_len, gap_len);
    }

    #[test]
    fn test_buf_delete_after_ptr_4() {
        let mut buf = Buffer::init(String::from("hel🧑‍🌾"));
        assert_eq!(buf.iptr, 0);

        buf.delete_after_ptr();
        assert_eq!(buf.to_string(), "el🧑‍🌾");

        buf.delete_after_ptr();
        assert_eq!(buf.to_string(), "l🧑‍🌾");

        buf.delete_after_ptr();
        assert_eq!(buf.to_string(), "🧑‍🌾");

        buf.delete_after_ptr();
        // \u{200D} is a zero-width joiner codepoint
        assert_eq!(buf.to_string(), "\u{200D}🌾");

        buf.delete_after_ptr();
        assert_eq!(buf.to_string(), "🌾");

        buf.delete_after_ptr();
        assert_eq!(buf.to_string(), "");
    }

    #[test]
    fn test_buf_move_ptr_forward_1() {
        let mut buf = Buffer::init(String::from(""));
        buf.move_ptr_forward();
        assert_eq!(buf.iptr, 0);
    }

    #[test]
    fn test_buf_move_ptr_forward_2() {
        let mut buf = Buffer::init(String::from("a"));
        buf.move_ptr_forward();
        assert_eq!(buf.iptr, 1);
        assert_eq!(buf.to_string(), "a");
    }

    #[test]
    fn test_buf_move_ptr_forward_3() {
        let mut buf = Buffer::init(String::from("a"));
        buf.insert('b');
        buf.move_ptr_forward();
        assert_eq!(buf.iptr, 2);
        assert_eq!(buf.to_string(), "ba");
        buf.insert('c');
        assert_eq!(buf.to_string(), "bac");
    }

    #[test]
    fn test_buf_move_ptr_backward_1() {
        let mut buf = Buffer::init(String::from(""));
        buf.move_ptr_backward();
        assert_eq!(buf.iptr, 0);
    }

    #[test]
    fn test_buf_move_ptr_backward_2() {
        let mut buf = Buffer::init(String::from("a"));
        buf.move_ptr_backward();
        assert_eq!(buf.iptr, 0);
    }

    #[test]
    fn test_buf_move_ptr_backward_3() {
        let mut buf = Buffer::init(String::from("a"));
        buf.move_ptr_forward();
        buf.move_ptr_backward();
        assert_eq!(buf.iptr, 0);
    }

    #[test]
    fn test_buf_iter_1() {
        let mut buf = Buffer::init(String::from(""));
        buf.insert('a');
        buf.move_ptr_backward();
        buf.move_ptr_forward();

        for b in buf.iter() {
            assert_eq!(*b, b'a');
        }
    }

    #[test]
    fn test_buf_iter_2() {
        let mut buf = Buffer::init(String::from("h🌾el"));
        buf.move_ptr_forward();
        buf.move_ptr_forward();

        for (i, b) in buf.iter().enumerate() {
            dbg!(i, *b);
        }
    }
}
