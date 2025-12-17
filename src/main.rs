use std::{
	collections::VecDeque,
	fmt::Display,
	io::{ErrorKind, Read, stdin, stdout},
	iter::Peekable,
};

use bitstream_io::{BigEndian, BitRead, BitReader, BitWrite, BitWriter};
use clap::{Args, Parser};

#[derive(Parser)]
struct Cli {
	#[command(flatten)]
	from_to: FromTo,
	/// When encountering an unknown character, encode it as "- - . - - "
	#[arg(long, short)]
	unknown: bool,
	/// insert STOP in place of periods and newlines.
	#[arg(long, short)]
	stop: bool,
}

#[derive(Args)]
#[group(required = false, multiple = false)]
struct FromTo {
	/// convert to utf-8 from morse
	#[arg(long, short, default_value_t = false)]
	from: bool,

	/// convert from utf-8 to morse (default)
	#[arg(long, short, default_value_t = false)]
	to: bool,
}

fn main() -> std::io::Result<()> {
	let cli = Cli::parse();

	match (cli.from_to.from, cli.from_to.to) {
		// convert from morse to utf-8
		(true, false) => {
			let signals = ByteSignalReader::new(stdin())
				.collect::<std::io::Result<Vec<Signal>>>()?;
			let str_from_signals = SignalsToCharIterator::new(signals.into_iter())
				.collect::<std::io::Result<String>>()?;
			println!("{}", str_from_signals);
			Ok(())
		}
		// convert to morse from utf-8
		(false, true) | (false, false) => {
			let mut s = String::new();
			stdin().read_to_string(&mut s)?;

			if cli.stop {
				let stop_re =
					regex::Regex::new(r"([^\s])(\. |\n+)").expect("invalid regex");
				s = stop_re.replace_all(&s, "$1 STOP ").to_string();
			}

			CharToSignalIterator::new(s.chars(), !cli.unknown).write(stdout())?;
			Ok(())
		}
		(_, _) => {
			unreachable!();
		}
	}
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
	skip_unrecognized: bool,
	has_sent_letter: bool,
	buf: VecDeque<Signal>,
}

impl<I: Iterator<Item = char>> CharToSignalIterator<I> {
	pub fn new(inner: I, skip_unrecognized: bool) -> Self {
		CharToSignalIterator {
			inner,
			skip_unrecognized,
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
			loop {
				if let Some(c) = self.inner.next() {
					match c {
						' ' => {
							if self.has_sent_letter {
								self.buf.push_back(Signal::WordGap);
								self.has_sent_letter = false;
								break;
							}
						}
						c => {
							let (char_signals, recognized) = char_to_signals(c);
							let should_skip = self.skip_unrecognized && !recognized;
							if should_skip {
								continue;
							}
							if self.has_sent_letter {
								self.buf.push_back(Signal::LongGap);
							}
							for s in itertools::intersperse(char_signals.iter(), &Signal::Gap)
							{
								self.buf.push_back(*s);
							}
							self.has_sent_letter = true;
							break;
						}
					}
				} else {
					return None;
				}
			}
		}
		let item = self.buf.pop_front().expect("buf should not be empty here");
		Some(item)
	}
}

fn char_to_signals(c: char) -> (&'static [Signal], bool) {
	use Signal::{Dash, Dot};
	match c {
		'A' | 'a' => (&[Dot, Dash], true),
		'B' | 'b' => (&[Dash, Dot, Dot, Dot], true),
		'C' | 'c' => (&[Dash, Dot, Dash, Dot], true),
		'D' | 'd' => (&[Dash, Dot, Dot], true),
		'E' | 'e' => (&[Dot], true),
		'F' | 'f' => (&[Dot, Dot, Dash, Dot], true),
		'G' | 'g' => (&[Dash, Dash, Dot], true),
		'H' | 'h' => (&[Dot, Dot, Dot, Dot], true),
		'I' | 'i' => (&[Dot, Dot], true),
		'J' | 'j' => (&[Dot, Dash, Dash, Dash], true),
		'K' | 'k' => (&[Dash, Dot, Dash], true),
		'L' | 'l' => (&[Dot, Dash, Dot, Dot], true),
		'M' | 'm' => (&[Dash, Dash], true),
		'N' | 'n' => (&[Dash, Dot], true),
		'O' | 'o' => (&[Dash, Dash, Dash], true),
		'P' | 'p' => (&[Dot, Dash, Dash, Dot], true),
		'Q' | 'q' => (&[Dash, Dash, Dot, Dash], true),
		'R' | 'r' => (&[Dot, Dash, Dot], true),
		'S' | 's' => (&[Dot, Dot, Dot], true),
		'T' | 't' => (&[Dash], true),
		'U' | 'u' => (&[Dot, Dot, Dash], true),
		'V' | 'v' => (&[Dot, Dot, Dot, Dash], true),
		'W' | 'w' => (&[Dot, Dash, Dash], true),
		'X' | 'x' => (&[Dash, Dot, Dot, Dash], true),
		'Y' | 'y' => (&[Dash, Dot, Dash, Dash], true),
		'Z' | 'z' => (&[Dash, Dash, Dot, Dot], true),
		'1' => (&[Dot, Dash, Dash, Dash, Dash], true),
		'2' => (&[Dot, Dot, Dash, Dash, Dash], true),
		'3' => (&[Dot, Dot, Dot, Dash, Dash], true),
		'4' => (&[Dot, Dot, Dot, Dot, Dash], true),
		'5' => (&[Dot, Dot, Dot, Dot, Dot], true),
		'6' => (&[Dash, Dot, Dot, Dot, Dot], true),
		'7' => (&[Dash, Dash, Dot, Dot, Dot], true),
		'8' => (&[Dash, Dash, Dash, Dot, Dot], true),
		'9' => (&[Dash, Dash, Dash, Dash, Dot], true),
		'0' => (&[Dash, Dash, Dash, Dash, Dash], true),
		_ => (&[Dash, Dash, Dot, Dash, Dash], false),
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
