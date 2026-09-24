//! Reading a raw commit object.
//!
//! `clean` rebuilds commits rather than letting git rewrite them, so it has to
//! understand the object format: header lines, then a blank line, then the
//! message. Headers can span lines — a signature continues on lines starting
//! with a space — and everything after the first blank line is message, even if
//! it looks like a header.

use std::fmt;

/// A commit object's author or committer line, split for reuse as env vars.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Ident {
    /// Display name, e.g. `Andrew McCall`.
    pub name: String,
    /// Email without the angle brackets.
    pub email: String,
    /// Unix timestamp and timezone exactly as stored, e.g. `1758100000 +0100`.
    pub date: String,
}

/// A parsed commit object.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Commit {
    /// The tree this commit points at.
    pub tree: String,
    /// Parent object ids, in order. Two or more means a merge.
    pub parents: Vec<String>,
    pub author: Ident,
    pub committer: Ident,
    /// Whether the object carried a signature header.
    pub signed: bool,
    /// The `encoding` header, when the message is not UTF-8.
    pub encoding: Option<String>,
    /// Everything after the header block's blank line.
    pub message: String,
}

/// A commit object that could not be understood.
#[derive(Debug, PartialEq, Eq)]
pub enum ParseError {
    /// A required header was absent.
    MissingHeader(&'static str),
    /// An author or committer line was not `Name <email> timestamp zone`.
    MalformedIdent { header: &'static str, line: String },
}

impl fmt::Display for ParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingHeader(name) => {
                write!(formatter, "commit object has no {name} header")
            }
            Self::MalformedIdent { header, line } => {
                write!(
                    formatter,
                    "commit object has an unreadable {header} line: {line}"
                )
            }
        }
    }
}

impl Commit {
    /// Parses the bytes `git cat-file commit <id>` prints.
    pub fn parse(raw: &str) -> Result<Self, ParseError> {
        // The first blank line ends the header block. Signature continuation
        // lines start with a space, so they never look blank.
        let (headers, message) = match raw.find("\n\n") {
            Some(blank) => (&raw[..blank + 1], &raw[blank + 2..]),
            None => (raw, ""),
        };

        let mut tree = None;
        let mut parents = Vec::new();
        let mut author = None;
        let mut committer = None;
        let mut signed = false;
        let mut encoding = None;

        for line in headers.lines() {
            if line.starts_with(' ') {
                continue; // continuation of the header above
            }
            let (key, value) = line.split_once(' ').unwrap_or((line, ""));
            match key {
                "tree" => tree = Some(value.to_string()),
                "parent" => parents.push(value.to_string()),
                "author" => author = Some(parse_ident("author", value, line)?),
                "committer" => committer = Some(parse_ident("committer", value, line)?),
                "gpgsig" | "gpgsig-sha256" => signed = true,
                "encoding" => encoding = Some(value.to_string()),
                _ => {}
            }
        }

        Ok(Self {
            tree: tree.ok_or(ParseError::MissingHeader("tree"))?,
            parents,
            author: author.ok_or(ParseError::MissingHeader("author"))?,
            committer: committer.ok_or(ParseError::MissingHeader("committer"))?,
            signed,
            encoding,
            message: message.to_string(),
        })
    }
}

fn parse_ident(header: &'static str, value: &str, line: &str) -> Result<Ident, ParseError> {
    Ident::parse(value).ok_or_else(|| ParseError::MalformedIdent {
        header,
        line: line.to_string(),
    })
}

impl Ident {
    /// Parses `Name <email> 1758100000 +0100`.
    pub fn parse(value: &str) -> Option<Self> {
        let open = value.find('<')?;
        let close = open + value[open..].find('>')?;
        let date = value[close + 1..].trim();
        if date.is_empty() {
            return None;
        }
        Some(Self {
            name: value[..open].trim().to_string(),
            email: value[open + 1..close].to_string(),
            date: date.to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SIMPLE: &str = concat!(
        "tree 4b825dc642cb6eb9a060e54bf8d69288fbee4904\n",
        "parent a3f19c2f0c1d4e5b6a7c8d9e0f1a2b3c4d5e6f70\n",
        "author Andrew McCall <andrew@example.com> 1758100000 +0100\n",
        "committer Andrew McCall <andrew@example.com> 1758100060 +0100\n",
        "\n",
        "Fix pagination\n",
        "\n",
        "Co-Authored-By: Claude <noreply@anthropic.com>\n",
    );

    #[test]
    fn parses_the_header_block() {
        let commit = Commit::parse(SIMPLE).unwrap();
        assert_eq!(commit.tree, "4b825dc642cb6eb9a060e54bf8d69288fbee4904");
        assert_eq!(
            commit.parents,
            vec!["a3f19c2f0c1d4e5b6a7c8d9e0f1a2b3c4d5e6f70"]
        );
        assert_eq!(commit.author.name, "Andrew McCall");
        assert_eq!(commit.author.email, "andrew@example.com");
        assert_eq!(commit.author.date, "1758100000 +0100");
        assert_eq!(commit.committer.date, "1758100060 +0100");
        assert!(!commit.signed);
        assert_eq!(commit.encoding, None);
    }

    #[test]
    fn parses_the_message_after_the_blank_line() {
        let commit = Commit::parse(SIMPLE).unwrap();
        assert_eq!(
            commit.message,
            "Fix pagination\n\nCo-Authored-By: Claude <noreply@anthropic.com>\n"
        );
    }

    #[test]
    fn parses_a_root_commit_with_no_parents() {
        let raw = SIMPLE.replace("parent a3f19c2f0c1d4e5b6a7c8d9e0f1a2b3c4d5e6f70\n", "");
        assert!(Commit::parse(&raw).unwrap().parents.is_empty());
    }

    #[test]
    fn parses_every_parent_of_a_merge() {
        let raw = SIMPLE.replace(
            "parent a3f19c2f0c1d4e5b6a7c8d9e0f1a2b3c4d5e6f70\n",
            "parent 1111111111111111111111111111111111111111\nparent 2222222222222222222222222222222222222222\n",
        );
        let commit = Commit::parse(&raw).unwrap();
        assert_eq!(
            commit.parents,
            vec![
                "1111111111111111111111111111111111111111",
                "2222222222222222222222222222222222222222"
            ]
        );
    }

    #[test]
    fn detects_a_signature_without_leaking_it_into_the_message() {
        let raw = concat!(
            "tree 4b825dc642cb6eb9a060e54bf8d69288fbee4904\n",
            "author A <a@example.com> 1758100000 +0100\n",
            "committer A <a@example.com> 1758100000 +0100\n",
            "gpgsig -----BEGIN PGP SIGNATURE-----\n",
            " \n",
            " iQIzBAABCgAdFiEE\n",
            " -----END PGP SIGNATURE-----\n",
            "\n",
            "Signed work\n",
        );
        let commit = Commit::parse(raw).unwrap();
        assert!(commit.signed);
        assert_eq!(commit.message, "Signed work\n");
    }

    #[test]
    fn detects_an_sha256_signature() {
        let raw = SIMPLE.replace(
            "committer Andrew",
            "gpgsig-sha256 -----BEGIN PGP SIGNATURE-----\n -----END PGP SIGNATURE-----\ncommitter Andrew",
        );
        assert!(Commit::parse(&raw).unwrap().signed);
    }

    #[test]
    fn parses_the_encoding_header() {
        let raw = SIMPLE.replace("\n\nFix", "\nencoding ISO-8859-1\n\nFix");
        assert_eq!(
            Commit::parse(&raw).unwrap().encoding.as_deref(),
            Some("ISO-8859-1")
        );
    }

    #[test]
    fn keeps_header_lookalikes_in_the_message() {
        let raw = SIMPLE.replace(
            "Fix pagination\n",
            "Fix pagination\n\ntree of life\nauthor unknown\n",
        );
        let commit = Commit::parse(&raw).unwrap();
        assert!(commit.message.contains("tree of life"));
        assert_eq!(commit.tree, "4b825dc642cb6eb9a060e54bf8d69288fbee4904");
    }

    #[test]
    fn parses_a_commit_with_an_empty_message() {
        let raw = concat!(
            "tree 4b825dc642cb6eb9a060e54bf8d69288fbee4904\n",
            "author A <a@example.com> 1758100000 +0100\n",
            "committer A <a@example.com> 1758100000 +0100\n",
            "\n",
        );
        assert_eq!(Commit::parse(raw).unwrap().message, "");
    }

    #[test]
    fn rejects_an_object_with_no_tree() {
        let raw = SIMPLE.replace("tree 4b825dc642cb6eb9a060e54bf8d69288fbee4904\n", "");
        assert_eq!(
            Commit::parse(&raw).unwrap_err(),
            ParseError::MissingHeader("tree")
        );
    }

    #[test]
    fn rejects_an_object_with_no_committer() {
        let raw = SIMPLE.replace(
            "committer Andrew McCall <andrew@example.com> 1758100060 +0100\n",
            "",
        );
        assert_eq!(
            Commit::parse(&raw).unwrap_err(),
            ParseError::MissingHeader("committer")
        );
    }

    #[test]
    fn rejects_a_malformed_author_line() {
        let raw = SIMPLE.replace(
            "author Andrew McCall <andrew@example.com> 1758100000 +0100",
            "author nonsense",
        );
        assert!(matches!(
            Commit::parse(&raw).unwrap_err(),
            ParseError::MalformedIdent {
                header: "author",
                ..
            }
        ));
    }

    #[test]
    fn ident_parses_a_name_containing_spaces() {
        let ident = Ident::parse("Ada Byron Lovelace <ada@example.com> 1758100000 -0500").unwrap();
        assert_eq!(ident.name, "Ada Byron Lovelace");
        assert_eq!(ident.email, "ada@example.com");
        assert_eq!(ident.date, "1758100000 -0500");
    }

    #[test]
    fn ident_parses_an_empty_name() {
        let ident = Ident::parse("<ada@example.com> 1758100000 +0000").unwrap();
        assert_eq!(ident.name, "");
        assert_eq!(ident.email, "ada@example.com");
    }

    #[test]
    fn ident_rejects_a_line_without_an_email() {
        assert_eq!(Ident::parse("Ada Lovelace 1758100000 +0000"), None);
    }

    #[test]
    fn ident_rejects_a_line_without_a_date() {
        assert_eq!(Ident::parse("Ada <ada@example.com>"), None);
    }
}
