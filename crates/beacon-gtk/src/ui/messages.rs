// SPDX-License-Identifier: GPL-3.0-or-later

//! How views talk: each takes typed input through `send` and reports typed
//! output to whoever connected to it. Input sent while the view is still
//! handling an earlier one (a widget it changes firing a signal back into
//! it, say) waits its turn rather than running inside the first, so a view
//! never handles two at once.

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::rc::Rc;

type Receiver<T> = Rc<dyn Fn(T)>;

/// Where a view's output goes: one receiver, connected by its owner.
pub struct Output<T>(Rc<RefCell<Option<Receiver<T>>>>);

impl<T> Default for Output<T> {
    fn default() -> Self {
        Self(Rc::default())
    }
}

impl<T> Clone for Output<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<T> Output<T> {
    /// Send output to `receive` from now on.
    pub fn connect(&self, receive: impl Fn(T) + 'static) {
        self.0.replace(Some(Rc::new(receive)));
    }

    /// Report `value`; nothing happens before anything is connected.
    pub fn emit(&self, value: T) {
        let receive = self.0.borrow().clone();
        if let Some(receive) = receive {
            receive(value);
        }
    }
}

/// What a view's widgets report through: input back to the view, or
/// output to its owner.
pub struct ViewSender<I, O> {
    input: Rc<dyn Fn(I)>,
    output: Output<O>,
}

impl<I, O> Clone for ViewSender<I, O> {
    fn clone(&self) -> Self {
        Self {
            input: self.input.clone(),
            output: self.output.clone(),
        }
    }
}

impl<I, O> ViewSender<I, O> {
    pub fn new(input: impl Fn(I) + 'static, output: Output<O>) -> Self {
        Self {
            input: Rc::new(input),
            output,
        }
    }

    pub fn input(&self, message: I) {
        (self.input)(message);
    }

    pub fn output(&self, message: O) {
        self.output.emit(message);
    }
}

/// A view's input, handled one message at a time, in order.
pub struct Inbox<T> {
    queue: RefCell<VecDeque<T>>,
    busy: Cell<bool>,
}

impl<T> Default for Inbox<T> {
    fn default() -> Self {
        Self {
            queue: RefCell::default(),
            busy: Cell::new(false),
        }
    }
}

impl<T> Inbox<T> {
    /// Handle `message` with `handle`, after any the view is handling now.
    pub fn deliver(&self, message: T, mut handle: impl FnMut(T)) {
        self.queue.borrow_mut().push_back(message);
        if self.busy.replace(true) {
            return;
        }
        loop {
            let next = self.queue.borrow_mut().pop_front();
            match next {
                Some(message) => handle(message),
                None => break,
            }
        }
        self.busy.set(false);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn input_sent_while_handling_waits_its_turn() {
        let inbox = Rc::new(Inbox::default());
        let seen = Rc::new(RefCell::new(Vec::new()));
        fn handle(inbox: &Rc<Inbox<u32>>, seen: &Rc<RefCell<Vec<u32>>>, n: u32) {
            let mut log = seen.borrow_mut();
            log.push(n);
            drop(log);
            if n == 1 {
                let (i, s) = (inbox.clone(), seen.clone());
                inbox.deliver(2, move |m| handle(&i, &s, m));
                // Not handled yet: it runs after this one.
                assert_eq!(*seen.borrow(), [1]);
            }
        }
        let (i, s) = (inbox.clone(), seen.clone());
        inbox.deliver(1, move |m| handle(&i, &s, m));
        assert_eq!(*seen.borrow(), [1, 2]);
    }

    #[test]
    fn output_goes_to_what_connected() {
        let output = Output::default();
        output.emit(1);
        let seen = Rc::new(Cell::new(0));
        let s = seen.clone();
        output.connect(move |n| s.set(n));
        output.emit(7);
        assert_eq!(seen.get(), 7);
    }
}
