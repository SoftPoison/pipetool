use std::{
    collections::VecDeque,
    fmt::Display,
    io::{Read, Stdin, Stdout, Write},
    num::NonZeroUsize,
};

const BUFFER_SIZE: usize = 32;

pub type Byte = u8;
pub type Char = char;
pub type Num = f64;

pub type StreamPipeline = fn(PipeType) -> Option<PipeType>;

pub trait Stream {
    fn size_hint(&self) -> Option<usize>;

    fn type_hint(&self) -> PipeTypeHint;

    fn next(&mut self) -> Option<PipeType>;
}

#[derive(Debug, Clone)]
pub enum PipeType {
    Byte(Byte),
    Char(Char),
    Num(Num),
    Multi(Vec<Option<PipeType>>),
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum PipeTypeHint {
    Byte,
    Char,
    Num,
    // Parallel(Box<PipeTypeHint>), // todo: how do we support this deep type hinting?
    Multi,
    Nothing,
}

impl PipeTypeHint {
    fn matches_hint(&self, target: &PipeType) -> bool {
        match self {
            PipeTypeHint::Byte => matches!(target, PipeType::Byte(_)),
            PipeTypeHint::Char => matches!(target, PipeType::Char(_)),
            PipeTypeHint::Num => matches!(target, PipeType::Num(_)),
            PipeTypeHint::Multi => matches!(target, PipeType::Multi(_)),
            PipeTypeHint::Nothing => false,
        }
    }
}

impl Display for PipeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PipeType::Byte(_) => write!(f, "byte"),
            PipeType::Char(_) => write!(f, "char"),
            PipeType::Num(_) => write!(f, "num"),
            PipeType::Multi(_) => write!(f, "multi"),
        }
    }
}

impl PipeType {
    #[inline]
    pub fn byte(self) -> Byte {
        match self {
            PipeType::Byte(b) => b,
            _ => panic!("expected a byte stream but got a {self} stream instead"),
        }
    }

    #[inline]
    pub fn char(self) -> Char {
        match self {
            PipeType::Char(c) => c,
            _ => panic!("expected a char stream but got a {self} stream instead"),
        }
    }

    #[inline]
    pub fn num(self) -> Num {
        match self {
            PipeType::Num(n) => n,
            _ => panic!("expected a num stream but got a {self} stream instead"),
        }
    }

    #[inline]
    pub fn parallel(self) -> Vec<Option<PipeType>> {
        match self {
            PipeType::Multi(p) => p,
            _ => panic!("expected a parallel stream but got a {self} stream instead"),
        }
    }

    pub fn same_type(&self, other: &Self) -> bool {
        match self {
            PipeType::Byte(_) => {
                if let PipeType::Byte(_) = other {
                    return true;
                }
            }
            PipeType::Char(_) => {
                if let PipeType::Char(_) = other {
                    return true;
                }
            }
            PipeType::Num(_) => {
                if let PipeType::Num(_) = other {
                    return true;
                }
            }
            PipeType::Multi(p) => {
                return !p
                    .iter()
                    .flatten()
                    .map(|x| Self::same_type(x, other))
                    .any(|x| !x);
            }
        }

        false
    }
}

struct Discard {
    input: Box<dyn Stream>,
}

impl Discard {
    fn uncoerced(input: Box<dyn Stream>) -> Box<Self> {
        Box::new(Self { input })
    }
}

impl Stream for Discard {
    fn size_hint(&self) -> Option<usize> {
        Some(0)
    }

    fn type_hint(&self) -> PipeTypeHint {
        PipeTypeHint::Nothing
    }

    fn next(&mut self) -> Option<PipeType> {
        while self.input.next().is_some() {}
        None
    }
}

pub fn coerce(input: Box<dyn Stream>, to: PipeTypeHint) -> Box<dyn Stream> {
    let from = input.type_hint();

    if from == to {
        return input;
    }

    if to == PipeTypeHint::Nothing {
        return Discard::uncoerced(input);
    }

    match from {
        PipeTypeHint::Byte => match to {
            PipeTypeHint::Char => BytesToChars::uncoerced(input),
            PipeTypeHint::Num => unimplemented!("BytesToNum does not yet exist"),
            PipeTypeHint::Multi => unimplemented!("Parallelisation does not yet exist"),
            _ => unreachable!(),
        },
        PipeTypeHint::Char => match to {
            PipeTypeHint::Byte => CharsToBytes::uncoerced(input),
            PipeTypeHint::Num => unimplemented!("CharsToNum does not yet exist"),
            PipeTypeHint::Multi => unimplemented!("Parallelisation does not yet exist"),
            _ => unreachable!(),
        },
        PipeTypeHint::Num => unimplemented!("Number methods do not yet exist"),
        PipeTypeHint::Multi => {
            unimplemented!("Indirectly flattening multiple items is not yet implemented")
        }
        PipeTypeHint::Nothing => {
            panic!("Attempting to turn a stream of nothing into a stream of {to:?}")
        }
    }
}

#[derive(Debug)]
pub struct In {
    buf: [Byte; 32],
    size: usize,
    offset: usize,
    exhausted: bool,
    stdin: Stdin,
}

impl In {
    pub fn new() -> Box<Self> {
        Box::new(Self {
            buf: [0; BUFFER_SIZE],
            size: 0,
            offset: 0,
            exhausted: false,
            stdin: std::io::stdin(),
        })
    }
}

impl Stream for In {
    fn size_hint(&self) -> Option<usize> {
        None
    }

    fn next(&mut self) -> Option<PipeType> {
        if self.exhausted && self.offset >= self.size {
            return None;
        }

        // we need to buffer some more bytes
        if self.offset >= self.size {
            let read = match self.stdin.read(&mut self.buf) {
                Ok(n) => n,
                Err(e) => {
                    eprintln!("Could not read stdin: {e:?}");
                    self.exhausted = true;
                    self.size = 0;
                    self.offset = BUFFER_SIZE;
                    return None;
                }
            };

            self.offset = 0;
            self.size = read;
            self.exhausted = read != BUFFER_SIZE;
        }

        // we have buffered bytes
        let b = self.buf[self.offset];
        self.offset += 1;

        Some(PipeType::Byte(b))
    }

    fn type_hint(&self) -> PipeTypeHint {
        PipeTypeHint::Byte
    }
}

pub struct Out {
    input: Box<dyn Stream>,
    stdout: Stdout,
}

impl Out {
    pub fn new(input: Box<dyn Stream>) -> Box<Self> {
        Box::new(Self {
            input, // does not need coercion since we accept any incoming type
            stdout: std::io::stdout(),
        })
    }

    fn output(stdout: &mut Stdout, item: PipeType) -> std::io::Result<()> {
        match item {
            PipeType::Byte(b) => stdout.write_all(&[b])?,
            PipeType::Char(c) => write!(stdout, "{c}")?,
            PipeType::Num(n) => writeln!(stdout, "{n}")?,
            PipeType::Multi(p) => {
                // todo: I think this is what we want? idk yet lol
                for x in p.into_iter().flatten() {
                    Self::output(stdout, x)?;
                }

                writeln!(stdout)?;
            }
        }

        Ok(())
    }
}

impl Stream for Out {
    fn size_hint(&self) -> Option<usize> {
        self.input.size_hint()
    }

    fn next(&mut self) -> Option<PipeType> {
        while let Some(x) = self.input.next() {
            Self::output(&mut self.stdout, x).ok()?;
        }
        None
    }

    fn type_hint(&self) -> PipeTypeHint {
        PipeTypeHint::Nothing
    }
}

pub struct Pass {
    input: Box<dyn Stream>,
}

impl Pass {
    pub fn new(input: Box<dyn Stream>) -> Box<Self> {
        Box::new(Self { input })
    }
}

impl Stream for Pass {
    fn size_hint(&self) -> Option<usize> {
        self.input.size_hint()
    }

    fn type_hint(&self) -> PipeTypeHint {
        self.input.type_hint()
    }

    fn next(&mut self) -> Option<PipeType> {
        self.input.next()
    }
}

pub struct GenerateByte {
    target: Byte,
}

impl GenerateByte {
    pub fn new(target: Byte) -> Box<Self> {
        Box::new(Self { target })
    }
}

impl Stream for GenerateByte {
    fn size_hint(&self) -> Option<usize> {
        None // we have no idea what the size of this output will be
    }

    fn type_hint(&self) -> PipeTypeHint {
        PipeTypeHint::Byte
    }

    fn next(&mut self) -> Option<PipeType> {
        Some(PipeType::Byte(self.target))
    }
}

pub struct GenerateChar {
    target: Char,
}

impl GenerateChar {
    pub fn new(target: Char) -> Box<Self> {
        Box::new(Self { target })
    }
}

impl Stream for GenerateChar {
    fn size_hint(&self) -> Option<usize> {
        None // we have no idea what the size of this output will be
    }

    fn type_hint(&self) -> PipeTypeHint {
        PipeTypeHint::Char
    }

    fn next(&mut self) -> Option<PipeType> {
        Some(PipeType::Char(self.target))
    }
}

pub struct GenerateNum {
    target: Num,
}

impl GenerateNum {
    pub fn new(target: Num) -> Box<Self> {
        Box::new(Self { target })
    }
}

impl Stream for GenerateNum {
    fn size_hint(&self) -> Option<usize> {
        None // we have no idea what the size of this output will be
    }

    fn type_hint(&self) -> PipeTypeHint {
        PipeTypeHint::Num
    }

    fn next(&mut self) -> Option<PipeType> {
        Some(PipeType::Num(self.target))
    }
}

pub struct ToHex {
    input: Box<dyn Stream>,
    leftover: Option<char>,
}

impl ToHex {
    pub fn new(input: Box<dyn Stream>) -> Box<Self> {
        Box::new(Self {
            input: coerce(input, PipeTypeHint::Byte),
            leftover: None,
        })
    }

    fn to_hex(b: Byte) -> (Char, Char) {
        const CHARSET: [Char; 16] = [
            '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', 'A', 'B', 'C', 'D', 'E', 'F',
        ];

        (
            CHARSET[((b & 0xf0) >> 4) as usize],
            CHARSET[(b & 0xf) as usize],
        )
    }
}

impl Stream for ToHex {
    fn size_hint(&self) -> Option<usize> {
        self.input.size_hint().map(|n| 2 * n)
    }

    fn next(&mut self) -> Option<PipeType> {
        if let Some(c) = self.leftover {
            self.leftover = None;
            return Some(PipeType::Char(c));
        }

        let b = self.input.next()?.byte();
        let chars = Self::to_hex(b);
        self.leftover = Some(chars.1);
        Some(PipeType::Char(chars.0))
    }

    fn type_hint(&self) -> PipeTypeHint {
        PipeTypeHint::Char
    }
}

// todo: FromHex

pub struct ToLower {
    input: Box<dyn Stream>,
}

impl ToLower {
    pub fn new(input: Box<dyn Stream>) -> Box<Self> {
        Box::new(Self {
            input: coerce(input, PipeTypeHint::Char),
        })
    }
}

impl Stream for ToLower {
    fn size_hint(&self) -> Option<usize> {
        self.input.size_hint()
    }

    fn next(&mut self) -> Option<PipeType> {
        let c = self.input.next()?.char();
        Some(PipeType::Char(c.to_ascii_lowercase()))
    }

    fn type_hint(&self) -> PipeTypeHint {
        PipeTypeHint::Char
    }
}

pub struct ToUpper {
    input: Box<dyn Stream>,
}

impl ToUpper {
    pub fn new(input: Box<dyn Stream>) -> Box<Self> {
        Box::new(Self {
            input: coerce(input, PipeTypeHint::Char),
        })
    }
}

impl Stream for ToUpper {
    fn size_hint(&self) -> Option<usize> {
        self.input.size_hint()
    }

    fn next(&mut self) -> Option<PipeType> {
        let c = self.input.next()?.char();
        Some(PipeType::Char(c.to_ascii_lowercase()))
    }

    fn type_hint(&self) -> PipeTypeHint {
        PipeTypeHint::Char
    }
}

const BASE64_CHARSET: [char; 64] = [
    'A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J', 'K', 'L', 'M', 'N', 'O', 'P', 'Q', 'R', 'S',
    'T', 'U', 'V', 'W', 'X', 'Y', 'Z', 'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j', 'k', 'l',
    'm', 'n', 'o', 'p', 'q', 'r', 's', 't', 'u', 'v', 'w', 'x', 'y', 'z', '0', '1', '2', '3', '4',
    '5', '6', '7', '8', '9', '+', '/',
];

pub struct ToBase64 {
    input: Box<dyn Stream>,
    leftover: [char; 3],
    offset: usize,
}

impl ToBase64 {
    pub fn new(input: Box<dyn Stream>) -> Box<Self> {
        Box::new(Self {
            input: coerce(input, PipeTypeHint::Byte),
            leftover: ['\0'; 3],
            offset: 3,
        })
    }
}

impl Stream for ToBase64 {
    fn size_hint(&self) -> Option<usize> {
        // todo: is this right?
        self.input.size_hint().map(|n| n * 4 / 3)
    }

    fn next(&mut self) -> Option<PipeType> {
        if self.offset < 3 {
            let c = self.leftover[self.offset];
            self.offset += 1;
            return Some(PipeType::Char(c));
        }

        let b0 = match self.input.next() {
            Some(b) => b.byte(),
            None => return None,
        };
        let b1 = self.input.next().map(PipeType::byte);
        let b2 = self.input.next().map(PipeType::byte);

        let i = (b0 as usize) << 16 | (b1.unwrap_or(0) as usize) << 8 | (b2.unwrap_or(0) as usize);

        let c = BASE64_CHARSET[i >> 18 & 0x3f];
        self.leftover[0] = BASE64_CHARSET[i >> 12 & 0x3f];
        self.leftover[1] = if b1.is_some() {
            BASE64_CHARSET[i >> 6 & 0x3f]
        } else {
            '='
        };
        self.leftover[2] = if b2.is_some() {
            BASE64_CHARSET[i & 0x3f]
        } else {
            '='
        };

        self.offset = 0;

        Some(PipeType::Char(c))
    }

    fn type_hint(&self) -> PipeTypeHint {
        PipeTypeHint::Char
    }
}

// todo: FromBase64

pub struct CharsToBytes {
    input: Box<dyn Stream>,
    leftover: [Byte; 4],
    offset: usize,
    len: usize,
}

impl CharsToBytes {
    pub fn new(input: Box<dyn Stream>) -> Box<Self> {
        Self::uncoerced(coerce(input, PipeTypeHint::Char))
    }

    pub(crate) fn uncoerced(input: Box<dyn Stream>) -> Box<Self> {
        Box::new(Self {
            input,
            leftover: [0; 4],
            offset: 0,
            len: 0,
        })
    }
}

impl Stream for CharsToBytes {
    fn size_hint(&self) -> Option<usize> {
        self.input.size_hint() // it's not exact, but it's a hint!
    }

    fn next(&mut self) -> Option<PipeType> {
        if self.offset < self.len {
            let b = self.leftover[self.offset];
            self.offset += 1;
            return Some(PipeType::Byte(b));
        }

        let c = self.input.next()?.char();
        let ret = c.encode_utf8(&mut self.leftover);
        self.len = ret.len();
        self.offset = 1;

        Some(PipeType::Byte(self.leftover[0]))
    }

    fn type_hint(&self) -> PipeTypeHint {
        PipeTypeHint::Byte
    }
}

pub struct BytesToChars {
    input: Box<dyn Stream>,
}

impl BytesToChars {
    pub fn new(input: Box<dyn Stream>) -> Box<Self> {
        Self::uncoerced(coerce(input, PipeTypeHint::Byte))
    }

    pub(crate) fn uncoerced(input: Box<dyn Stream>) -> Box<Self> {
        Box::new(BytesToChars { input })
    }
}

impl Stream for BytesToChars {
    fn size_hint(&self) -> Option<usize> {
        self.input.size_hint() // it's not exact, but it's a hint!
    }

    fn type_hint(&self) -> PipeTypeHint {
        PipeTypeHint::Char
    }

    fn next(&mut self) -> Option<PipeType> {
        let mut buf = [0u8; 4];
        for i in 0..4 {
            buf[i] = self.input.next()?.byte();
            if let Ok(s) = std::str::from_utf8(&buf[0..=i]) {
                return Some(PipeType::Char(s.chars().next().unwrap()));
            }
        }

        panic!("Encountered invalid UTF-8 bytes which couldn't be turned into a char: {buf:?}")
    }
}

pub struct Fork {
    input: Box<dyn Stream>,
    num_outputs: NonZeroUsize,
    buffers: Box<[VecDeque<PipeType>]>,
}

pub struct ForkOutput {
    parent: Box<Fork>,
}

impl Fork {
    pub fn blah(input: Box<dyn Stream>, num_outputs: NonZeroUsize) -> Box<[ForkOutput]> {
        let this = Box::new(Fork {
            input,
            num_outputs,
            buffers: Box::new([VecDeque::new(); num_outputs]), // how tf do I do this
        });

        todo!()
    }
}

// pub struct Fork {
//     input: Box<dyn Stream>,
//     functions: Vec<StreamPipeline>,
// }

// impl Fork {
//     pub fn new(input: Box<dyn Stream>, functions: Vec<StreamPipeline>) -> Box<Self> {
//         Box::new(Self { input, functions })
//     }
// }

// impl Stream for Fork {
//     fn size_hint(&self) -> Option<usize> {
//         self.input.size_hint()
//     }

//     fn type_hint(&self) -> PipeTypeHint {
//         PipeTypeHint::Multi
//     }

//     fn next(&mut self) -> Option<PipeType> {
//         let x = self.input.next()?;
//         let results: Vec<_> = self.functions.iter().map(|f| f(x.clone())).collect();

//         Some(PipeType::Multi(results))
//     }
// }

// groups items in bunches and spits out the bunches one at a time
pub struct Group {
    input: Box<dyn Stream>,
    size: NonZeroUsize,
}

impl Group {
    pub fn new(input: Box<dyn Stream>, size: NonZeroUsize) -> Box<Self> {
        Box::new(Self { input, size })
    }
}

impl Stream for Group {
    fn size_hint(&self) -> Option<usize> {
        self.input.size_hint().map(|x| x / self.size)
    }

    fn type_hint(&self) -> PipeTypeHint {
        PipeTypeHint::Multi
    }

    fn next(&mut self) -> Option<PipeType> {
        let first = self.input.next()?;
        let mut ret = Vec::with_capacity(self.size.into());

        ret.push(Some(first));

        for _ in 1..self.size.into() {
            ret.push(self.input.next());
        }

        Some(PipeType::Multi(ret))
    }
}

pub struct AlternatingMerge {
    input: Box<dyn Stream>,
    hint: PipeTypeHint,
    buffer: VecDeque<PipeType>,
}

impl AlternatingMerge {
    pub fn new(input: Box<dyn Stream>, input_hint: PipeTypeHint) -> Box<Self> {
        Box::new(Self {
            input,
            hint: input_hint,
            buffer: VecDeque::new(),
        })
    }
}

impl Stream for AlternatingMerge {
    fn size_hint(&self) -> Option<usize> {
        self.input.size_hint()
    }

    fn type_hint(&self) -> PipeTypeHint {
        self.hint
    }

    fn next(&mut self) -> Option<PipeType> {
        let mut items = match self.input.next() {
            Some(x) => x,
            None => {
                while !self.buffer.is_empty() {
                    if let Some(x) = self.buffer.pop_front() {
                        return Some(x);
                    }
                }

                return None;
            }
        }
        .parallel();

        while !self.buffer.is_empty() {
            if let Some(x) = self.buffer.pop_front() {
                return Some(x);
            }
        }

        items.retain(Option::is_some);
        let mut items = items.into_iter();
        let ret = items.next().flatten();

        for i in items {
            self.buffer.push_back(i.unwrap()); // unwrap is okay, because of the retain
        }

        ret
    }
}

pub struct SequentialMerge {
    input: Box<dyn Stream>,
    hint: PipeTypeHint,
    buffers: Vec<VecDeque<PipeType>>,
}

impl SequentialMerge {
    pub fn new(input: Box<dyn Stream>, input_hint: PipeTypeHint) -> Box<Self> {
        Box::new(Self {
            input,
            hint: input_hint,
            buffers: Vec::new(),
        })
    }
}

impl Stream for SequentialMerge {
    fn size_hint(&self) -> Option<usize> {
        self.input.size_hint() // this is so not right, but oh well
    }

    fn type_hint(&self) -> PipeTypeHint {
        self.hint
    }

    fn next(&mut self) -> Option<PipeType> {
        let mut items = match self.input.next() {
            Some(x) => x,
            None => {
                for buff in &mut self.buffers {
                    while !buff.is_empty() {
                        if let Some(x) = buff.pop_front() {
                            return Some(x);
                        }
                    }
                }

                return None;
            }
        }
        .parallel();

        if items
            .iter()
            .flatten()
            .map(|x| self.hint.matches_hint(x))
            .any(|b| !b)
        {
            panic!(
                "Forked streams are not of the expected type ({:?})",
                self.hint
            );
        }

        // resize if we need extra buffer space
        // we should never encounter the case where it's negative since fork does not resize
        for _ in 0..(items.len() - self.buffers.len() - 1) {
            self.buffers.push(VecDeque::new());
        }

        let first = items.remove(0)?;

        todo!()
    }
}

pub struct Skip {
    input: Box<dyn Stream>,
    num: usize,
    curr: usize,
}

impl Skip {
    pub fn new(input: Box<dyn Stream>, num: usize) -> Box<Self> {
        Box::new(Self {
            input,
            num,
            curr: 0,
        })
    }
}

impl Stream for Skip {
    fn size_hint(&self) -> Option<usize> {
        self.input.size_hint().map(|n| n.saturating_sub(self.num))
    }

    fn type_hint(&self) -> PipeTypeHint {
        self.input.type_hint()
    }

    fn next(&mut self) -> Option<PipeType> {
        while self.curr < self.num {
            self.curr += 1;
            self.input.next()?;
        }

        self.input.next()
    }
}

pub struct Take {
    input: Box<dyn Stream>,
    num: usize,
    curr: usize,
}

impl Take {
    pub fn new(input: Box<dyn Stream>, num: usize) -> Box<Self> {
        Box::new(Self {
            input,
            num,
            curr: 0,
        })
    }
}

impl Stream for Take {
    fn size_hint(&self) -> Option<usize> {
        self.input.size_hint().map(|n| n.min(self.num))
    }

    fn type_hint(&self) -> PipeTypeHint {
        self.input.type_hint()
    }

    fn next(&mut self) -> Option<PipeType> {
        if self.curr >= self.num {
            return None;
        }

        self.curr += 1;
        self.input.next()
    }
}

fn main() {
    let step = In::new();

    // let step = Fork::new(step, vec![
    //     |x| {},
    //     |x| {},
    // ]);
    // let step = Fork::new(step, vec![])
    // let step = ToBase64::new(step);
    let mut output = Out::new(step);

    while output.next().is_some() {}

    println!()
}
