#![feature(env, fs, io, path, slicing_syntax, plugin)]
extern crate jit;
use jit::*;
use std::cell::RefCell;
use std::old_io;
use std::io::Read;
use std::fs::File;
use std::iter::Peekable;
use std::mem;
use std::env;
use std::old_path::Path;
use std::rc::Rc;

static PROMPT:&'static str = "> ";
type WrappedLoop<'a> = Rc<RefCell<Loop<'a>>>;
struct Loop<'a> {
    start: Label<'a>,
    end: Label<'a>,
    parent: Option<WrappedLoop<'a>>
}

impl<'a> Loop<'a> {
    fn new(func: &UncompiledFunction<'a>, current_loop: Option<WrappedLoop<'a>>) -> Loop<'a> {
        let mut new_loop = Loop {
            start: Label::new(func),
            end: Label::new(func),
            parent: current_loop
        };
        func.insn_label(&mut new_loop.start);
        new_loop
    }
    fn end(&mut self, func: &UncompiledFunction<'a>) -> Option<WrappedLoop<'a>> {
        func.insn_branch(&mut self.start);
        func.insn_label(&mut self.end);
        let mut parent = None;
        mem::swap(&mut parent, &mut self.parent);
        parent
    }
}

fn count<'a, I>(func: &UncompiledFunction<'a>, code: &mut Peekable<I>, curr:char) -> Value<'a> where I:Iterator<Item=char> {
    let mut amount = 1us;
    while code.peek() == Some(&curr) {
        amount += 1;
        code.next();
    }
    func.insn_of(&amount)
}

fn compile<'a>(func: &UncompiledFunction<'a>, code: &str) {
    let ubyte = typecs::get_ubyte();;
    let putchar_sig = get::<fn(u8)>();
    let readchar_sig = get::<fn() -> u8>();
    let data = func[0];
    let mut current_loop = None;
    let mut code = code.chars().peekable();
    while let Some(c) = code.next() {
        match c {
            '>' => {
                let amount = count(func, &mut code, c);
                let new_value = data + amount;
                func.insn_store(data, new_value);
            },
            '<' => {
                let amount = count(func, &mut code, c);
                let new_value = data - amount;
                func.insn_store(data, new_value);
            },
            '+' => {
                let amount = count(func, &mut code, c);
                let mut value = func.insn_load_relative(data, 0, ubyte);
                value = value + amount;
                value = func.insn_convert(value, ubyte, false);
                func.insn_store_relative(data, 0, value)
            },
            '-' => {
                let amount = count(func, &mut code, c);
                let mut value = func.insn_load_relative(data, 0, ubyte);
                value = value - amount;
                value = func.insn_convert(value, ubyte, false);
                func.insn_store_relative(data, 0, value)
            },
            '.' => {
                extern fn putchar(c: u8) {
                    old_io::stdout().write_u8(c).unwrap();
                }
                let value = func.insn_load_relative(data, 0, ubyte);
                func.insn_call_native1(Some("putchar"), putchar, putchar_sig.get(), [value], flags::NO_THROW);
            },
            ',' => {
                extern fn readchar() -> u8 {
                    old_io::stdin().read_byte().unwrap()
                }
                let value = func.insn_call_native0(Some("readchar"), readchar, readchar_sig.get(), flags::NO_THROW);
                func.insn_store_relative(data, 0, value);
            },
            '[' => {
                let wrapped_loop = Rc::new(RefCell::new(Loop::new(func, current_loop)));
                let tmp = func.insn_load_relative(data, 0, ubyte);
                {
                    let mut borrow = wrapped_loop.borrow_mut();
                    func.insn_branch_if_not(tmp, &mut borrow.end);
                }
                current_loop = Some(wrapped_loop);
            },
            ']' => {
                current_loop = if let Some(ref inner_loop) = current_loop {
                    let mut borrow = inner_loop.borrow_mut();
                    borrow.end(func)
                } else {
                    None
                }
            },
            _ => ()
        }
    };
    func.insn_default_return();
}
fn run(ctx: &mut Context, code: &str) {
    let sig = get::<fn(*mut u8)>();
    let func = ctx.build_func(sig.get(), |func| compile(func, code));
    func.with(|func:extern fn(*mut u8)| {
        let mut data: [u8; 3000] = unsafe { mem::zeroed() };
        func(data.as_mut_ptr());
    });
}
fn main() {
    let mut ctx = Context::new();
    if let Some(ref script) = env::args().skip(1).next() {
        let ref script = Path::new(&*script);
        let mut text = String::new();
        File::open(script).unwrap().read_to_string(&mut text).unwrap();
        run(&mut ctx, &*text);
    } else {
        old_io::print(PROMPT);
        let mut input = old_io::stdin();
        for line in input.lock().lines() {
            run(&mut ctx, &*line.unwrap());
            old_io::print(PROMPT);
        }
    }
}