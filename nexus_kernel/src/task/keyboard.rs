use conquer_once::spin::OnceCell;
use core::{
    pin::Pin,
    task::{Context, Poll},
};
use crossbeam_queue::ArrayQueue;
use futures::stream::Stream;
use futures::task::AtomicWaker;

static SCANCODE_QUEUE: OnceCell<ArrayQueue<u8>> = OnceCell::uninit();
pub(crate) static WAKER: AtomicWaker = AtomicWaker::new();

/// Called by the keyboard interrupt handler
/// Must not block or allocate.
pub(crate) fn add_scancode(scancode: u8) {
    if let Ok(queue) = SCANCODE_QUEUE.try_get() {
        if let Err(_) = queue.push(scancode) {
            // queue full; discard scancode
            // outputting a warning is difficult in interrupt context
        } else {
            WAKER.wake();
        }
    } else {
        // queue not initialized
    }
}

pub struct ScancodeStream {
    _private: (),
}

impl ScancodeStream {
    pub fn new() -> Self {
        SCANCODE_QUEUE
            .try_init_once(|| ArrayQueue::new(100))
            .expect("ScancodeStream::new should only be called once");
        ScancodeStream { _private: () }
    }
}

impl Stream for ScancodeStream {
    type Item = u8;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<u8>> {
        let queue = SCANCODE_QUEUE
            .try_get()
            .expect("scancode queue not initialized");

        // fast path
        if let Some(scancode) = queue.pop() {
            return Poll::Ready(Some(scancode));
        }

        WAKER.register(&cx.waker());
        match queue.pop() {
            Some(scancode) => {
                WAKER.take();
                Poll::Ready(Some(scancode))
            }
            None => Poll::Pending,
        }
    }
}

pub async fn print_keypresses() {
    let mut scancodes = ScancodeStream::new();
    use futures::stream::StreamExt;

    while let Some(scancode) = scancodes.next().await {
        if let Ok(char) = decode_scancode(scancode) {
            crate::print!("{}", char);
        } else {
            crate::print!("{}", scancode);
        }
    }
}

fn decode_scancode(scancode: u8) -> Result<char, ()> {
    // minimalist set 1 (QWERTY)
    let c = match scancode {
        0x02 => '1',
        0x03 => '2',
        0x04 => '3',
        0x05 => '4',
        0x06 => '5',
        0x07 => '6',
        0x08 => '7',
        0x09 => '8',
        0x0A => '9',
        0x0B => '0',
        0x10 => 'q',
        0x11 => 'w',
        0x12 => 'e',
        0x13 => 'r',
        0x14 => 't',
        0x15 => 'y',
        0x16 => 'u',
        0x17 => 'i',
        0x18 => 'o',
        0x19 => 'p',
        0x1E => 'a',
        0x1F => 's',
        0x20 => 'd',
        0x21 => 'f',
        0x22 => 'g',
        0x23 => 'h',
        0x24 => 'j',
        0x25 => 'k',
        0x26 => 'l',
        0x2C => 'z',
        0x2D => 'x',
        0x2E => 'c',
        0x2F => 'v',
        0x30 => 'b',
        0x31 => 'n',
        0x32 => 'm',
        0x39 => ' ',
        0x1C => '\n',
        _ => return Err(()),
    };
    Ok(c)
}
