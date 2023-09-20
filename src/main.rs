mod generate;

use clap::Parser;
use clap::builder::PossibleValue;
use generate::{generate_wordlist, ReplacePair};
use itertools::{Itertools, iproduct};
use regex_syntax::{hir, hir::Hir, hir::HirKind, parse};
use std::error::Error;
use std::fmt;
use std::fs::{File, OpenOptions};
use std::io::{self, BufRead, BufReader, Seek};
use std::path::PathBuf;
use rayon::prelude::*;

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

struct Generator {
    min_length: usize,
    max_length: usize,
    transforms: Vec<ReplacePair>,
    dictionary: Vec<String>,
    terms: Vec<String>,
}

impl<'a> fmt::Display for Generator {
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
        transforms: transforms,
        terms: terms,
        dictionary: dictionary,
    };
    println!("{}", gen_info);

    let regex = args.pattern.expect("Testing regex");
    let result = regex_words(&regex, &gen_info);

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

fn regex_words(input: &str, gen: &Generator) -> Vec<String> {
    // Create AST
    let ast = parse(input).expect("regex is invalid");
    println!("Regex: {}", input);

    // Walk AST recursively
    println!("{:#?}", ast);
    let possible = walk_regex(&ast, &gen);

    println!("Before filter: {}", possible.len());
    let result: Vec<String> = possible.into_iter()
    .filter(|word| {
        // max_length is already checked at concat and repeat...where else could it happen?
        return word.len() >= gen.min_length;
    })
    .collect();
    println!("After filter: {}", result.len());

    return result;
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
        }
        HirKind::Empty => {
            println!("EMPTY: {}", ast);
        }
        HirKind::Literal(c) => match &*c.0 {
            b"%t" => {
                for t in &gen.terms {
                    result.push(t.clone());
                }
            }
            b"%w" => {
                for w in &gen.dictionary {
                    result.push(w.clone());
                }
            }
            _ => {
                result.push(std::str::from_utf8(&c.0).unwrap().to_string());
            }
        }
        HirKind::Class(hir::Class::Unicode(s)) => {
            if s.is_ascii() {
                let all_chars = s.iter().map(|x| x.start()..=x.end()).flatten();
                // let char_count: usize = s.iter().map(|x| x.len()).sum();
                // println!("Added {} chars", char_count);
                for ch in all_chars {
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

            // determine max iterations that wouldn't exceed max_length
            let shortest = if let Some(shortest_word) = words.iter().min_by_key(|s| s.len()) {
                shortest_word.len()
            } else {
                0
            } as u32;
            let last = (max + shortest - 1) / shortest;
            let range = c.min..=last;
            let words: Vec<Vec<String>> = range.into_par_iter().map(|i| {
                // println!("Repeat {}", i);
                let x = (1..=i).map(|_| &words).multi_cartesian_product().filter_map(|word| {
                    let text = word.into_iter().join("");
                    // only check max isn't exceeded
                    if text.len() <= gen.max_length {
                        return Some(text);
                    }
                    None
                }).collect();
                println!("Repeat {} done. Max = {}", i, gen.max_length);
                x
            }).collect();
            result.extend(words.iter().cloned().flatten());
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

            let cartesian = groups.iter().multi_cartesian_product();
            println!("Cartesian concat done");

            for x in cartesian {
                if x.iter().map(|y| y.len()).sum::<usize>() <= gen.max_length {
                    result.push(x.iter().join(""));
                }
            }

            println!("Done joining concat results");
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


#[cfg(test)]
mod tests {
    use super::*;

    fn empty_gen(min: usize, max: usize) -> Generator {
        Generator {
            min_length: min,
            max_length: max,
            transforms: vec![],
            terms: vec![],
            dictionary: vec![],
        }
    }

    #[test]
    fn play() {

    }

    #[test]
    fn simple_concat() {
        let gen = empty_gen(2, 2);
        let pattern = "[a-c][1-3]";
        let expected = vec!["a1", "b2", "c3"];

        expect(&gen, pattern, expected, 9);
    }
    #[test]
    fn repeat_1() {
        let gen1 = empty_gen(1, 1);
        let pattern1 = "[a-z]+";
        let expected1 = vec!["a", "m", "z"];
        expect(&gen1, pattern1, expected1, 26);
    }

    #[test]
    fn repeat_2() {
        let gen = empty_gen(2, 2);
        let pattern = "[a-c]+";
        let expected = vec!["aa", "bc", "cc"];
        expect(&gen, pattern, expected, 9);
    }

    #[test]
    fn repeat_1_2() {
        let gen2 = empty_gen(1, 2);
        let pattern2 = "[a-z]+";
        let expected2 = vec!["a", "mt", "zz"];
        expect(&gen2, pattern2, expected2, 26*26 + 26);
    }

    #[test]
    fn concat_repeat_2_5() {
        let min = 2;
        let max = 5;
        let gen = empty_gen(min, max);
        let pattern = "[a-d]+[1-6]+";
        let expected = vec!["a3", "abcb1", "b2312", "cca33", "cc2", "abc3"];

        let count = calculate_total_creations(4, 6, min..=max);
        let _res = expect(&gen, pattern, expected, count);
        // println!("{}", res.join("\n"));
        // assert!(false);
    }

    fn expect(gen: &Generator, pattern: &str, expected: Vec<&str>, count: usize) -> Vec<String> {
        let result = regex_words(pattern, &gen);

        assert_eq!(result.len(), count, "RESULT: {:?}", &result[0..50]);
        for s in expected {
            assert!(result.contains(&s.to_string()), "doesn't contain {}", s);
        }
        return result;
    }


    // [a-d]+[1-3]+
    // 3 => 84 == 4^2 * 3^1 + 4^1 * 3^2
    // 4 => 444 == 4^3 * 3^1 + 4^2 * 3^2 + 4^1*3^3
    // 5 => 2100 == 4^4 * 3^1 + 4^3 * 3^2 + 4^2 * 3^3 + 4^1 * 3^4
    fn calculate_total_creations(pool1_size: usize, pool2_size: usize, length_range: std::ops::RangeInclusive<usize>) -> usize {
        length_range
            .map(|string_length| {
                let mut total = 0;
                
                for i in 1..string_length {
                    let pool1_contrib = pool1_size.pow((string_length - i) as u32);
                    let pool2_contrib = pool2_size.pow(i as u32);
                    total += pool1_contrib * pool2_contrib;
                }
                
                total
            })
            .sum()
    }
}