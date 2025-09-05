use crate::buffer::Buffer;

#[derive(Debug)]
pub struct UndoManager {
    undos: Vec<Command>,
    redos: Vec<Command>,
}

#[derive(Debug)]
pub enum Command {
    Insert((usize, String)),        // (insertion pointer, num of bytes inserted)
    DeleteBefore((usize, Vec<u8>)), // (insertion pointer, deleted bytes)
    DeleteAfter((usize, Vec<u8>)),  // (insertion pointer, deleted bytes)
    Checkpoint,                     // special checkpoint (i.e when saved)
}

// TODO: fix this frequently small string allocation
impl UndoManager {
    pub fn new() -> Self {
        Self {
            undos: Vec::with_capacity(8),
            redos: Vec::with_capacity(8),
        }
    }

    pub fn push(&mut self, cmd: Command) {
        let mut accumulate = false;

        let final_cmd = if let Some(last_cmd) = self.undos.last() {
            match last_cmd {
                Command::DeleteBefore((last_i, last_val)) => match cmd {
                    Command::DeleteBefore((i, mut val)) if i + val.len() == *last_i => {
                        accumulate = true;

                        val.extend(last_val);
                        Command::DeleteBefore((i, val))
                    }
                    _ => cmd,
                },
                Command::DeleteAfter((last_i, last_val)) => match cmd {
                    Command::DeleteAfter((i, val)) if i == *last_i => {
                        accumulate = true;

                        let mut deleted = last_val.clone();
                        deleted.extend(val);
                        Command::DeleteAfter((i, deleted))
                    }
                    _ => cmd,
                },
                Command::Insert((last_i, last_s)) => match cmd {
                    Command::Insert((i, s)) if last_i + last_s.len() == i => {
                        accumulate = true;

                        let mut val = last_s.clone();
                        val.push_str(&s);
                        Command::Insert((*last_i, val))
                    }
                    _ => cmd,
                },
                Command::Checkpoint => {
                    accumulate = true;

                    cmd
                }
            }
        } else {
            cmd
        };

        if accumulate {
            self.undos.pop();
        }

        self.undos.push(final_cmd);
        self.redos.clear();
    }

    pub fn undo(&mut self, buf: &mut Buffer) {
        if let Some(cmd) = self.undos.pop() {
            match cmd {
                Command::Insert((prev, ref inserted)) => buf.revert_insert(prev, inserted.len()),
                Command::DeleteBefore((prev, ref deleted)) => {
                    buf.revert_delete_before_ptr(prev, deleted)
                }
                Command::DeleteAfter((prev, ref deleted)) => {
                    buf.revert_delete_after_ptr(prev, deleted)
                }
                Command::Checkpoint => {}
            }

            self.redos.push(cmd);
        }
    }

    pub fn redo(&mut self, buf: &mut Buffer) {
        if let Some(cmd) = self.redos.pop() {
            match cmd {
                Command::Insert((prev, ref inserted)) => {
                    buf.jump(prev);
                    for c in inserted.chars() {
                        buf.insert(c);
                    }
                }
                Command::DeleteBefore((prev, ref deleted)) => {
                    buf.jump(prev + deleted.len());
                    for _ in 0..deleted.len() {
                        buf.delete_before_ptr();
                    }
                }
                Command::DeleteAfter((prev, ref deleted)) => {
                    buf.jump(prev);
                    for _ in 0..deleted.len() {
                        buf.delete_after_ptr();
                    }
                }
                Command::Checkpoint => {}
            }

            self.undos.push(cmd);
        }
    }
}
