//! POSIX extended regular expressions provided by the system C library.
use std::{ffi::CString, mem::MaybeUninit};
pub struct Regex {
    inner: Box<libc::regex_t>,
}
impl Regex {
    pub fn new(pattern: &str, case: bool) -> Result<Self, String> {
        let pattern = CString::new(pattern).map_err(|_| "A search pattern cannot contain NUL")?;
        let mut inner = Box::new(unsafe { MaybeUninit::<libc::regex_t>::zeroed().assume_init() });
        let code = unsafe {
            libc::regcomp(
                &mut *inner,
                pattern.as_ptr(),
                libc::REG_EXTENDED | if case { 0 } else { libc::REG_ICASE },
            )
        };
        if code != 0 {
            let n = unsafe { libc::regerror(code, &*inner, std::ptr::null_mut(), 0) };
            let mut text = vec![0u8; n];
            unsafe {
                libc::regerror(code, &*inner, text.as_mut_ptr().cast(), n);
            }
            if text.last() == Some(&0) {
                text.pop();
            }
            return Err(String::from_utf8_lossy(&text).into());
        }
        Ok(Self { inner })
    }
    pub fn captures(&self, text: &str) -> Vec<Vec<Option<(usize, usize)>>> {
        self.captures_from(text, 0)
    }
    pub fn captures_from(&self, text: &str, at: usize) -> Vec<Vec<Option<(usize, usize)>>> {
        self.collect(text, at, false)
    }
    pub fn first_match(&self, text: &str, at: usize) -> Option<(usize, usize)> {
        self.collect(text, at, true).first().and_then(|c| c[0])
    }
    fn collect(&self, text: &str, mut at: usize, first: bool) -> Vec<Vec<Option<(usize, usize)>>> {
        let mut bytes = text.as_bytes().to_vec();
        bytes.push(0);
        let mut out = Vec::new();
        while at <= text.len() {
            let mut groups = [libc::regmatch_t {
                rm_so: -1,
                rm_eo: -1,
            }; 10];
            if text.len() > libc::regoff_t::MAX as usize {
                break;
            }
            groups[0].rm_so = at as libc::regoff_t;
            groups[0].rm_eo = text.len() as libc::regoff_t;
            let result = unsafe {
                libc::regexec(
                    &*self.inner,
                    bytes.as_ptr().cast(),
                    groups.len(),
                    groups.as_mut_ptr(),
                    libc::REG_STARTEND | if at > 0 { libc::REG_NOTBOL } else { 0 },
                )
            };
            if result != 0 {
                break;
            }
            let groups = groups
                .iter()
                .map(|m| {
                    if m.rm_so < 0 || m.rm_eo < 0 {
                        None
                    } else {
                        Some((m.rm_so as usize, m.rm_eo as usize))
                    }
                })
                .collect::<Vec<_>>();
            let Some((a, b)) = groups[0] else {
                break;
            };
            if !text.is_char_boundary(a) || !text.is_char_boundary(b) {
                // In a non-UTF-8 locale regexec can report byte boundaries.
                // Never slice UTF-8 at such a boundary.
                at = b.max(at + 1);
                while at < text.len() && !text.is_char_boundary(at) {
                    at += 1;
                }
                continue;
            }
            out.push(groups);
            if first {
                break;
            }
            if b > a {
                at = b;
            } else if b < text.len() {
                at = b + text[b..].chars().next().unwrap().len_utf8();
            } else {
                break;
            }
        }
        out
    }
}
impl Drop for Regex {
    fn drop(&mut self) {
        unsafe {
            libc::regfree(&mut *self.inner);
        }
    }
}
pub fn escape(text: &str) -> String {
    let mut out = String::new();
    for c in text.chars() {
        if ".[^$()|*+?{\\".contains(c) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

impl std::fmt::Debug for Regex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PosixRegex").finish_non_exhaustive()
    }
}
