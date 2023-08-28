mod generate;

use clap::Parser;
use generate::{generate_wordlist, ReplacePair};
use regex_syntax::{hir, hir::Hir, hir::HirKind, parse};
use std::error::Error;
use std::fmt;
use std::fs::{File, OpenOptions};
use std::io::{self, BufRead, BufReader, Seek};
use std::path::PathBuf;

/// Simple program to generate a wordlist
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Output file to write to
    #[arg(short, long, value_hint = clap::ValueHint::DirPath)]
    output: PathBuf,

    /// Input wordlist to permutate
    #[arg(short, long, value_hint = clap::ValueHint::DirPath)]
    dictionary: Option<PathBuf>,

    /// Custom terms to add to dictionary words
    #[arg(short, long, value_delimiter = ',')]
    terms: Vec<String>,

    /// Characters to replace (i.e. e=3 to replace e's with 3's)
    #[arg(short, long, value_parser=parse_key_val, default_value="o=0,e=3,l=1,i=!,a=@,s=$,t=7", value_delimiter=',')]
    replacements: Vec<ReplacePair>,

    /// Min/max password length
    #[arg(short, long, num_args = 2, default_values_t = vec![3, 6])]
    length: Vec<usize>,

    /// Regex syntax for word generation
    #[arg(short, long)]
    pattern: Option<String>,
}

struct Generator<'a> {
    min_length: usize,
    max_length: usize,
    transforms: &'a [ReplacePair],
    dictionary: &'a [String],
    terms: &'a [String],
}

impl<'a> fmt::Display for Generator<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Generator {{\n")?;
        // Print info
        writeln!(f, "\tDictionary length: {}", self.dictionary.len())?;
        writeln!(f, "\tTerms length: {}", self.terms.len())?;
        writeln!(f, "\tReplacements: {}", self.transforms.len())?;
        writeln!(
            f,
            "\tWord min/max: {} - {}",
            self.min_length, self.max_length
        )?;
        write!(f, "}}\n")
    }
}

fn main() -> io::Result<()> {
    let args = Args::parse();
    // println!("{:#?}", args);

    // parse args
    let min_length = args.length[0];
    let max_length = args.length[1];
    let transforms = args.replacements;
    let terms: Vec<String> = args
        .terms
        .into_iter()
        .filter(|word| word.len() <= max_length)
        .collect();
    let dictionary: Vec<String> = if let Some(dict_fname) = args.dictionary {
        read_dictionary(dict_fname, max_length)?
    } else {
        Vec::new()
    };

    // open for read/write/create
    let mut fout: File = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(true)
        .open(args.output.clone())?;

    let gen_info = Generator {
        min_length,
        max_length,
        transforms: &transforms,
        terms: &terms,
        dictionary: &dictionary,
    };
    println!("{}", gen_info);

    let mut regex = args.pattern.expect("Testing regex");

    // Ensure $ to signify end of string
    if regex.chars().last() != Some('$') {
        regex.push('$');
    }

    // Create AST
    let ast = parse(&regex).expect("regex is invalid");
    println!("Regex: {}", regex);

    // Walk AST recursively
    println!("{:#?}", ast);
    let mut string = String::new();
    walk_regex(&ast, &mut string, &gen_info, 0);

    // generate 'em
    // println!("Generating...");
    // generate_wordlist(&fout, dictionary, terms, transforms, min_length, max_length)?;

    // count resulting lines
    fout.seek(std::io::SeekFrom::Start(0))?;
    let line_count = BufReader::new(&fout).lines().count();
    println!("{} words generated!", line_count);

    Ok(())
}

// "(%t){3}(199[0-9]|20(0[0-9]|1[0-9]|2[0-3]))"
// 2 terms + 1990-99 | 2000-2023
fn walk_regex(ast: &Hir, candidate: &mut String, gen: &Generator, depth: usize) {
    if candidate.len() >= gen.max_length {
        return;
    }

    for _ in 0..depth {
        print!("- ");
    }
    match ast.kind() {
        HirKind::Look(c) => match c {
            hir::Look::End => {
                println!("END OF LINE. TODO: add '{}'", candidate);
            }
            _ => {
                println!("LOOK: {:#?}", c);
            }
        },
        HirKind::Empty => {
            println!("EMPTY: {}", ast);
        }
        HirKind::Literal(c) => match &*c.0 {
            b"%t" => {
                println!("FOUND TERM");
            }
            b"%w" => {
                println!("FOUND WORD");
            }
            _ => {
                println!("LITERAL: {:#?}", c);
            }
        },

        // end recursion here?
        HirKind::Class(c) => match c {
            hir::Class::Unicode(s) => {
                if s.is_ascii() {
                    let all_chars = s.ranges().iter().map(|x| x.start()..=x.end()).flatten();
                    println!("EXTENDING WORD: '{}'", candidate);
                    for ch in all_chars {
                        let mut word = candidate.clone();
                        word.push(ch);
                        println!("ADD WORD: '{}'", word);
                        walk_regex(ast, &mut word, gen, depth);
                    }
                } else {
                    let all_len: usize = s.ranges().iter().map(|x| x.len()).sum();
                    print!("Non-ascii values ({})", all_len);
                }
                println!("");
                // println!("CLASS: {:#?}", s.ranges());
            }
            _ => {
                println!("CLASS IS BYTES???");
            }
        },
        HirKind::Repetition(c) => {
            // TODO: max default should be max_length?
            println!(
                "REPETITION({}, {}):",
                c.min,
                c.max.unwrap_or(std::f32::INFINITY as u32)
            );
            // walk_regex(&c.sub, candidate, gen, depth + 1);

            // while candidate.len() < max_length
            // -> add to candidate
            for _ in 0..c.min {
                walk_regex(&c.sub, candidate, gen, depth + 1);
            }

            // continue walk

            // for i in min..max
            // -> break if candidate.len() > max_length
            // -> continue walk
        }
        HirKind::Capture(c) => {
            // capture's are just separators for this usage
            // println!("CAPTURE: {:#?}", c);
            print!("\r");
            walk_regex(&c.sub, candidate, gen, depth);
        }
        HirKind::Concat(coll) => {
            // println!("CONCAT: {:#?}", coll);
            println!("CONCAT:");
            for x in coll {
                walk_regex(x, candidate, gen, depth + 1);
            }
        }
        HirKind::Alternation(coll) => {
            println!("ALTERNATION:");
            for x in coll {
                walk_regex(x, candidate, gen, depth + 1);
            }
        } // _ => println!("NO MATCH: {}", ast),
    }
}

fn read_dictionary(filename: PathBuf, max_length: usize) -> io::Result<Vec<String>> {
    let file = File::open(filename)?;
    let lines = io::BufReader::new(file).lines();

    let dictionary: Vec<String> = lines
        .filter_map(|line| line.ok())
        .map(|word| word.trim().to_string())
        .filter(|word| word.len() > 0 && word.len() <= max_length)
        .collect();

    Ok(dictionary)
}

/// Parse a single key-value pair
fn parse_key_val(s: &str) -> Result<ReplacePair, Box<dyn Error + Send + Sync + 'static>> {
    let pos = s
        .find('=')
        .ok_or_else(|| format!("invalid KEY=value: no `=` found in `{s}`"))?;
    Ok(ReplacePair(s[..pos].parse()?, s[pos + 1..].parse()?))
}
