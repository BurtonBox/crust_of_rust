#![allow(missing_debug_implementations, rust_2018_idioms, missing_docs)]

#[derive(Debug, PartialEq)]
pub struct StrSplit<'haystack, D> {
	pub remainder: Option<&'haystack str>,
	pub delimiter: D,
	empty_leading_pending: bool,
}

impl<'haystack, D> StrSplit<'haystack, D> {
	pub fn new(haystack: &'haystack str, delimiter: D) -> Self {
		Self {
			remainder: Some(haystack),
			delimiter,
			empty_leading_pending: true,
		}
	}
}

pub trait Delimiter {
	fn find_next(&self, s: &str) -> Option<(usize, usize)>;
}

impl<'haystack, D> Iterator for StrSplit<'haystack, D>
where
	D: Delimiter,
{
	type Item = &'haystack str;

	fn next(&mut self) -> Option<Self::Item> {
		let s = self.remainder.take()?;
		if let Some((start, end)) = self.delimiter.find_next(s) {
			if start == end {
				if self.empty_leading_pending {
					self.empty_leading_pending = false;
					self.remainder = Some(s);
					return Some(&s[..0]);
				}

				if s.is_empty() {
					return Some(&s[..0]);
				}

				let mut it = s.char_indices();
				let (_, ch) = it.next().unwrap();
				let k = ch.len_utf8();
				let piece = &s[..k];
				self.remainder = Some(&s[k..]);
				return Some(piece);
			}

			let head = &s[..start];
			let tail = &s[end..];
			self.empty_leading_pending = true; // reset for next boundary
			self.remainder = Some(tail);
			Some(head)
		} else {
			self.empty_leading_pending = true;
			Some(s)
		}
	}
}


impl Delimiter for &str {
	fn find_next(&self, s: &str) -> Option<(usize, usize)> {
		if self.is_empty() {
			Some((0, 0))
		} else {
			s.find(*self).map(|start| (start, start + self.len()))
		}
	}
}

impl Delimiter for char {
	fn find_next(&self, s: &str) -> Option<(usize, usize)> {
		s.find(*self).map(|start| (start, start + self.len_utf8()))
	}
}

impl Delimiter for &[char] {
	fn find_next(&self, s: &str) -> Option<(usize, usize)> {
		s.char_indices().find_map(|(i, c)| {
			if self.contains(&c) {
				Some((i, i + c.len_utf8()))
			} else {
				None
			}
		})
	}
}

impl<F> Delimiter for F
where
	F: Fn(char) -> bool,
{
	fn find_next(&self, s: &str) -> Option<(usize, usize)> {
		s.char_indices()
			.find(|&(_, ch)| self(ch))
			.map(|(i, ch)| (i, i + ch.len_utf8()))
	}
}

#[cfg(test)]
mod tests {
	use crate::StrSplit;

	pub fn until_char(s: &str, c: char) -> &'_ str {
		let delim = format!("{}", c);
		StrSplit::new(s, &*delim)
			.next()
			.expect("StrSplit always gives at least one result")
	}

	#[test]
	fn until_char_test() {
		assert_eq!(until_char("hello world", 'o'), "hell");
	}

	#[test]
	fn it_works() {
		let haystack = "a b c d e";
		let letters: Vec<_> = StrSplit::new(haystack, " ").collect();
		assert_eq!(letters, vec!["a", "b", "c", "d", "e"]);
	}

	#[test]
	fn tail() {
		let haystack = "a b c d ";
		let letters: Vec<_> = StrSplit::new(haystack, " ").collect();
		assert_eq!(letters, vec!["a", "b", "c", "d", ""]);
	}

	#[test]
	fn split_test_01() {
		let haystack = "Mary had a little lamb";
		let splits: Vec<_> = StrSplit::new(haystack, " ").collect();
		assert_eq!(splits, ["Mary", "had", "a", "little", "lamb"]);
	}

	#[test]
	fn split_test_02() {
		let haystack = "";
		let splits: Vec<_> = StrSplit::new(haystack, "X").collect();
		assert_eq!(splits, [""]);
	}

	#[test]
	fn split_test_03() {
		let haystack = "lion::tiger::leopard";
		let splits: Vec<_> = StrSplit::new(haystack, "::").collect();
		assert_eq!(splits, ["lion", "tiger", "leopard"]);
	}

	#[test]
	fn split_test_04() {
		let haystack = "abc1def2ghi";
		let splits: Vec<_> = StrSplit::new(haystack, char::is_numeric).collect();
		assert_eq!(splits, ["abc", "def", "ghi"]);
	}

	#[test]
	fn split_test_05() {
		let haystack = "lionXtigerXleopard";
		let splits: Vec<_> = StrSplit::new(haystack, char::is_uppercase).collect();
		assert_eq!(splits, ["lion", "tiger", "leopard"]);
	}

	#[test]
	fn split_test_06() {
		let haystack = "2020-11-03 23:59";
		let splits: Vec<_> = StrSplit::new(haystack, &['-', ' ', ':', '@'][..]).collect();
		assert_eq!(splits, ["2020", "11", "03", "23", "59"]);
	}

	#[test]
	fn split_test_07() {
		let haystack = "abc1defXghi";
		let splits: Vec<_> = StrSplit::new(haystack, |c| c == '1' || c == 'X').collect();
		assert_eq!(splits, ["abc", "def", "ghi"]);
	}

	#[test]
	fn split_test_08() {
		let haystack = "||||a||b|c";
		let splits: Vec<_> = StrSplit::new(haystack, '|').collect();
		assert_eq!(splits, &["", "", "", "", "a", "", "b", "c"]);
	}

	#[test]
	fn split_test_09() {
		let haystack ="(///)".to_string();
		let splits: Vec<_> = StrSplit::new(&haystack, '/').collect();
		assert_eq!(splits, &["(", "", "", ")"]);
	}

	#[test]
	fn split_test_10() {
		let haystack ="010".to_string();
		let splits: Vec<_> = StrSplit::new(&haystack, "0").collect();
		assert_eq!(splits, &["", "1", ""]);
	}

	#[test]
	fn split_test_11() {
		let haystack = "    a  b c".to_string();
		let splits: Vec<_> = StrSplit::new(&haystack, ' ').collect();
		assert_eq!(splits, &["", "", "", "", "a", "", "b", "c"]);
	}

	#[test]
	fn split_test_12() {
		let haystack ="rust".to_string();
		let splits: Vec<_> = StrSplit::new(&haystack, "").collect();
		assert_eq!(splits, &["", "r", "u", "s", "t", ""]);
	}
}