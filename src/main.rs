mod generate;

use clap::Parser;
use generate::{generate_wordlist, ReplacePair};
use itertools::Itertools;
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

    let regex = args.pattern.expect("Testing regex");

    // Create AST
    let ast = parse(&regex).expect("regex is invalid");
    println!("Regex: {}", regex);

    // Walk AST recursively
    println!("{:#?}", ast);
    let result = walk_regex(&ast, &gen_info);

    println!("RESULT:");
    for word in &result {
        println!("{}", word);
    }

    println!("{} words generated!", result.len());

    // generate 'em
    // println!("Generating...");
    // generate_wordlist(&fout, dictionary, terms, transforms, min_length, max_length)?;

    // count resulting lines
    // fout.seek(std::io::SeekFrom::Start(0))?;
    // let line_count = BufReader::new(&fout).lines().count();
    // println!("{} words generated!", line_count);

    Ok(())
}

// "(%t){3}(199[0-9]|20(0[0-9]|1[0-9]|2[0-3]))"
// 2 terms + 1990-99 | 2000-2023
fn walk_regex(ast: &Hir, gen: &Generator) -> Vec<String> {
    let mut result: Vec<String> = Vec::new();

    match ast.kind() {
        HirKind::Look(c) => match c {
            hir::Look::End => {
                println!("END OF LINE.");
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
                for t in gen.terms {
                    result.push(t.clone());
                }
            }
            b"%w" => {
                for w in gen.dictionary {
                    result.push(w.clone());
                }
            }
            _ => {
                result.push(std::str::from_utf8(&c.0).unwrap().to_string());
            }
        },

        // this contains literal values
        HirKind::Class(hir::Class::Unicode(s)) => {
            if s.is_ascii() {
                let all_chars = s.iter().map(|x| x.start()..=x.end()).flatten();
                // let char_count: usize = s.iter().map(|x| x.len()).sum();
                // println!("Added {} chars", char_count);
                for ch in all_chars {
                    // let mut word = candidate.clone();
                    // word.push(ch);
                    result.push(ch.to_string());
                }
            } else {
                let all_len: usize = s.ranges().iter().map(|x| x.len()).sum();
                print!("Non-ascii values ({})", all_len);
            }
        }
        HirKind::Class(hir::Class::Bytes(s)) => {
            println!("CLASS IS BYTES??? {:#?}", s);
        }
        HirKind::Repetition(c) => {
            let max = c.max.unwrap_or(gen.max_length as u32);

            if c.min == 0 {
                result.push(String::new());
            }

            let words = walk_regex(&c.sub, gen);
            // println!("Permutating {} words {} times", words.len(), (max - c.min));
            for i in c.min..=max {
                for word in (1..=i).map(|_| &words).multi_cartesian_product() {
                    result.push(word.into_iter().join(""));
                }
            }
        }
        HirKind::Capture(c) => {
            // capture's are just groups...we don't really care
            for word in walk_regex(&c.sub, gen) {
                result.push(word);
            }
        }
        HirKind::Concat(coll) => {
            let mut groups: Vec<Vec<String>> = Vec::new();
            let mut count = 0;
            for x in coll {
                let group = walk_regex(x, gen);
                count += group.len();
                groups.push(group.clone());
            }

            println!(
                "Concating {} groups with {} total words",
                groups.len(),
                count
            );
            let all_perms: std::collections::HashSet<String> = groups
                .iter()
                .map(|sublist| sublist.iter())
                .multi_cartesian_product()
                .filter(|combination| {
                    let total_length = combination.iter().map(|s| s.len()).sum::<usize>();
                    total_length >= gen.min_length && total_length <= gen.max_length
                })
                .map(|combo| combo.into_iter().join(""))
                .collect();

            // println!("{} total permutations", all_perms.into_iter().count());
            println!("Done concating {}", all_perms.len());

            // let unique_words: std::collections::HashSet<String> = all_perms.into_iter().collect();
            // for word in unique_words {
            for word in all_perms {
                // if !result.contains(&word) {
                result.push(word);
                // }
            }

            println!("Done again");
        }
        HirKind::Alternation(coll) => {
            for x in coll {
                for word in walk_regex(x, gen) {
                    result.push(word);
                }
            }
        } // _ => println!("NO MATCH: {}", ast),
    }

    result
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
