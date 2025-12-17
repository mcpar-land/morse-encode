use std::{
	collections::VecDeque,
	fmt::Display,
	io::{ErrorKind, Read, stdin, stdout},
	iter::Peekable,
};

use bitstream_io::{BigEndian, BitRead, BitReader, BitWrite, BitWriter};

fn main() -> std::io::Result<()> {
	let mut s = String::new();
	stdin().read_to_string(&mut s)?;

	CharToSignalIterator::new(s.chars()).write(stdout())?;

	Ok(())

	// let test_str = "12345 The hungry purple dinosaur ate the kind zingy fox, the jabbering crab, and the mad whale and started vending and quacking 67890";

	// println!("{}", test_str);

	// let signals_from_str =
	// 	CharToSignalIterator::new(test_str.chars()).collect::<Vec<Signal>>();
	// println!(
	// 	"{} ({})",
	// 	signals_to_string(signals_from_str.iter()),
	// 	signals_from_str.len()
	// );

	// let mut signals_bytes = Vec::<u8>::new();

	// CharToSignalIterator::new(test_str.chars()).write(&mut signals_bytes)?;

	// println!("{:x?}", signals_bytes);

	// let mut signals_from_bytes = Vec::<Signal>::new();
	// for signal in ByteSignalReader::new(Cursor::new(&signals_bytes)) {
	// 	match signal {
	// 		Ok(signal) => signals_from_bytes.push(signal),
	// 		Err(err) => {
	// 			println!("{}", err);
	// 			break;
	// 		}
	// 	}
	// }

	// println!(
	// 	"{} ({})",
	// 	signals_to_string(signals_from_bytes.iter()),
	// 	signals_from_bytes.len()
	// );

	// let str_from_signals =
	// 	SignalsToCharIterator::new(signals_from_bytes.into_iter())
	// 		.collect::<std::io::Result<String>>()?;

	// println!("{}", str_from_signals);
	// println!("{:x?}", str_from_signals.as_bytes());

	// Ok(())
}

const DOT_LENGTH: usize = 1;
const DASH_LENGTH: usize = 2;
const GAP_LENGTH: usize = 1;
const LONG_GAP_LENGTH: usize = 2;
const WORD_GAP_LENGTH: usize = 3;

pub struct SignalsToCharIterator<I: Iterator<Item = Signal>> {
	inner: I,
	on_space: bool,
}

impl<I: Iterator<Item = Signal>> SignalsToCharIterator<I> {
	pub fn new(inner: I) -> Self {
		Self {
			inner,
			on_space: false,
		}
	}
}

impl<I: Iterator<Item = Signal>> Iterator for SignalsToCharIterator<I> {
	type Item = std::io::Result<char>;

	fn next(&mut self) -> Option<Self::Item> {
		if self.on_space {
			self.on_space = false;
			return Some(Ok(' '));
		}
		let mut current_code = Vec::<Signal>::new();
		loop {
			let signal = match self.inner.next() {
				Some(signal) => signal,
				None => break,
			};
			match signal {
				Signal::Dot => current_code.push(Signal::Dot),
				Signal::Dash => current_code.push(Signal::Dash),
				Signal::Gap => continue,
				Signal::LongGap => break,
				Signal::WordGap => {
					self.on_space = true;
					break;
				}
			}
		}
		if current_code.len() == 0 {
			return None;
		}
		Some(signals_to_char(&current_code))
	}
}

pub struct ByteSignalReader<R: std::io::Read> {
	inner: Peekable<BitIterator<R>>,
}

impl<R: std::io::Read> ByteSignalReader<R> {
	pub fn new(inner: R) -> Self {
		Self {
			inner: BitIterator(BitReader::endian(inner, BigEndian)).peekable(),
		}
	}
}

impl<R: std::io::Read> Iterator for ByteSignalReader<R> {
	type Item = std::io::Result<Signal>;

	fn next(&mut self) -> Option<Self::Item> {
		let current_bit = match self.inner.next()? {
			Ok(current) => current,
			Err(err) => return Some(Err(err)),
		};
		let mut counter = 1;
		let hit_end = loop {
			let peek_bit = self.inner.peek();
			match peek_bit {
				Some(inner_res) => match inner_res {
					Ok(inner_bit) => {
						if *inner_bit == current_bit {
							counter += 1;
							self.inner.next();
						} else {
							break false;
						}
					}
					Err(_) => {
						return self.inner.next().map(|v| v.map(|_| unreachable!()));
					}
				},
				None => {
					break true;
				}
			}
		};
		if hit_end {
			return None;
		}
		Some(match (current_bit, counter) {
			(true, DASH_LENGTH) => Ok(Signal::Dash),
			(true, DOT_LENGTH) => Ok(Signal::Dot),
			(false, WORD_GAP_LENGTH) => Ok(Signal::WordGap),
			(false, LONG_GAP_LENGTH) => Ok(Signal::LongGap),
			(false, GAP_LENGTH) => Ok(Signal::Gap),
			(current_bit, counter) => Err(std::io::Error::new(
				ErrorKind::InvalidData,
				format!("unrecognized bit len {:?} {}", current_bit, counter),
			)),
		})
	}
}

struct BitIterator<R: std::io::Read>(BitReader<R, BigEndian>);

impl<R: std::io::Read> Iterator for BitIterator<R> {
	type Item = std::io::Result<bool>;

	fn next(&mut self) -> Option<Self::Item> {
		match self.0.read_bit() {
			Ok(bit) => Some(Ok(bit)),
			Err(err) => match err.kind() {
				std::io::ErrorKind::UnexpectedEof => None,
				_ => Some(Err(err)),
			},
		}
	}
}

pub struct CharToSignalIterator<I: Iterator<Item = char>> {
	inner: I,
	has_sent_letter: bool,
	buf: VecDeque<Signal>,
}

impl<I: Iterator<Item = char>> CharToSignalIterator<I> {
	pub fn new(inner: I) -> Self {
		CharToSignalIterator {
			inner,
			has_sent_letter: false,
			buf: VecDeque::new(),
		}
	}
}

impl<I: Iterator<Item = char>> CharToSignalIterator<I> {
	fn write<W: std::io::Write>(self, w: W) -> std::io::Result<()> {
		let mut w = BitWriter::endian(w, BigEndian);
		for signal in self {
			signal.write(&mut w)?;
		}
		w.byte_align()?;
		w.flush()?;
		Ok(())
	}
}

impl<I: Iterator<Item = char>> Iterator for CharToSignalIterator<I> {
	type Item = Signal;

	fn next(&mut self) -> Option<Self::Item> {
		if self.buf.len() == 0 {
			if let Some(c) = self.inner.next() {
				match c {
					' ' => {
						self.buf.push_back(Signal::WordGap);
						self.has_sent_letter = false;
					}
					c => {
						if self.has_sent_letter {
							self.buf.push_back(Signal::LongGap);
						}
						for s in
							itertools::intersperse(char_to_signals(c).iter(), &Signal::Gap)
						{
							self.buf.push_back(*s);
						}
						self.has_sent_letter = true;
					}
				}
			} else {
				return None;
			}
		}
		let item = self.buf.pop_front().expect("buf should not be empty here");
		Some(item)
	}
}

pub fn char_to_signals(c: char) -> &'static [Signal] {
	use Signal::{Dash, Dot};
	match c {
		'A' | 'a' => &[Dot, Dash],
		'B' | 'b' => &[Dash, Dot, Dot, Dot],
		'C' | 'c' => &[Dash, Dot, Dash, Dot],
		'D' | 'd' => &[Dash, Dot, Dot],
		'E' | 'e' => &[Dot],
		'F' | 'f' => &[Dot, Dot, Dash, Dot],
		'G' | 'g' => &[Dash, Dash, Dot],
		'H' | 'h' => &[Dot, Dot, Dot, Dot],
		'I' | 'i' => &[Dot, Dot],
		'J' | 'j' => &[Dot, Dash, Dash, Dash],
		'K' | 'k' => &[Dash, Dot, Dash],
		'L' | 'l' => &[Dot, Dash, Dot, Dot],
		'M' | 'm' => &[Dash, Dash],
		'N' | 'n' => &[Dash, Dot],
		'O' | 'o' => &[Dash, Dash, Dash],
		'P' | 'p' => &[Dot, Dash, Dash, Dot],
		'Q' | 'q' => &[Dash, Dash, Dot, Dash],
		'R' | 'r' => &[Dot, Dash, Dot],
		'S' | 's' => &[Dot, Dot, Dot],
		'T' | 't' => &[Dash],
		'U' | 'u' => &[Dot, Dot, Dash],
		'V' | 'v' => &[Dot, Dot, Dot, Dash],
		'W' | 'w' => &[Dot, Dash, Dash],
		'X' | 'x' => &[Dash, Dot, Dot, Dash],
		'Y' | 'y' => &[Dash, Dot, Dash, Dash],
		'Z' | 'z' => &[Dash, Dash, Dot, Dot],
		'1' => &[Dot, Dash, Dash, Dash, Dash],
		'2' => &[Dot, Dot, Dash, Dash, Dash],
		'3' => &[Dot, Dot, Dot, Dash, Dash],
		'4' => &[Dot, Dot, Dot, Dot, Dash],
		'5' => &[Dot, Dot, Dot, Dot, Dot],
		'6' => &[Dash, Dot, Dot, Dot, Dot],
		'7' => &[Dash, Dash, Dot, Dot, Dot],
		'8' => &[Dash, Dash, Dash, Dot, Dot],
		'9' => &[Dash, Dash, Dash, Dash, Dot],
		'0' => &[Dash, Dash, Dash, Dash, Dash],
		_ => &[Dash, Dash, Dot, Dash, Dash],
	}
}

pub fn signals_to_char(signals: &[Signal]) -> std::io::Result<char> {
	use Signal::{Dash, Dot};
	match signals {
		[Dot, Dash] => Ok('A'),
		[Dash, Dot, Dot, Dot] => Ok('B'),
		[Dash, Dot, Dash, Dot] => Ok('C'),
		[Dash, Dot, Dot] => Ok('D'),
		[Dot] => Ok('E'),
		[Dot, Dot, Dash, Dot] => Ok('F'),
		[Dash, Dash, Dot] => Ok('G'),
		[Dot, Dot, Dot, Dot] => Ok('H'),
		[Dot, Dot] => Ok('I'),
		[Dot, Dash, Dash, Dash] => Ok('J'),
		[Dash, Dot, Dash] => Ok('K'),
		[Dot, Dash, Dot, Dot] => Ok('L'),
		[Dash, Dash] => Ok('M'),
		[Dash, Dot] => Ok('N'),
		[Dash, Dash, Dash] => Ok('O'),
		[Dot, Dash, Dash, Dot] => Ok('P'),
		[Dash, Dash, Dot, Dash] => Ok('Q'),
		[Dot, Dash, Dot] => Ok('R'),
		[Dot, Dot, Dot] => Ok('S'),
		[Dash] => Ok('T'),
		[Dot, Dot, Dash] => Ok('U'),
		[Dot, Dot, Dot, Dash] => Ok('V'),
		[Dot, Dash, Dash] => Ok('W'),
		[Dash, Dot, Dot, Dash] => Ok('X'),
		[Dash, Dot, Dash, Dash] => Ok('Y'),
		[Dash, Dash, Dot, Dot] => Ok('Z'),
		[Dot, Dash, Dash, Dash, Dash] => Ok('1'),
		[Dot, Dot, Dash, Dash, Dash] => Ok('2'),
		[Dot, Dot, Dot, Dash, Dash] => Ok('3'),
		[Dot, Dot, Dot, Dot, Dash] => Ok('4'),
		[Dot, Dot, Dot, Dot, Dot] => Ok('5'),
		[Dash, Dot, Dot, Dot, Dot] => Ok('6'),
		[Dash, Dash, Dot, Dot, Dot] => Ok('7'),
		[Dash, Dash, Dash, Dot, Dot] => Ok('8'),
		[Dash, Dash, Dash, Dash, Dot] => Ok('9'),
		[Dash, Dash, Dash, Dash, Dash] => Ok('0'),
		[Dash, Dash, Dot, Dash, Dash] => Ok('?'),
		signals => Err(std::io::Error::new(
			ErrorKind::InvalidData,
			format!("not recognized as a character {:?}", signals),
		)),
	}
}

#[derive(Clone, Copy, Debug)]
pub enum Signal {
	Dot,
	Dash,
	Gap,
	LongGap,
	WordGap,
}

impl Signal {
	pub fn write<W: BitWrite>(&self, mut writer: W) -> std::io::Result<()> {
		let (bit, length) = match self {
			Signal::Dot => (true, DOT_LENGTH),
			Signal::Dash => (true, DASH_LENGTH),
			Signal::Gap => (false, GAP_LENGTH),
			Signal::LongGap => (false, LONG_GAP_LENGTH),
			Signal::WordGap => (false, WORD_GAP_LENGTH),
		};
		for _ in 0..length {
			writer.write_bit(bit)?;
		}

		Ok(())
	}
}

impl Display for Signal {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		let (bit, length) = match self {
			Signal::Dot => ('▄', DOT_LENGTH),
			Signal::Dash => ('▄', DASH_LENGTH),
			Signal::Gap => (' ', GAP_LENGTH),
			Signal::LongGap => (' ', LONG_GAP_LENGTH),
			Signal::WordGap => (' ', WORD_GAP_LENGTH),
		};
		for _ in 0..length {
			write!(f, "{}", bit)?;
		}
		Ok(())
	}
}

fn signals_to_string<'a, I: Iterator<Item = &'a Signal>>(iter: I) -> String {
	let mut s = String::new();
	for signal in iter {
		s.push_str(&format!("{}", signal));
	}
	s
}
